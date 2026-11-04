//! Original question discovery independent of the visible transcript page.
use crate::{
    conversation_focus::{self, Request},
    gateway::Api,
    model::{text, Item},
};
use reqwest::{Method, StatusCode};
use serde_json::Value;
use std::time::Duration;

const MAX_PAGES: usize = 64;

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn source(value: &Value, limit: usize) -> Result<String, String> {
    if !value["content"].is_array() {
        return Err("Question history returned incomplete user content".into());
    }
    let (role, body) = Item::source_content(value);
    if role != "You" {
        return Err("Question history returned a non-user item".into());
    }
    if body.len() > limit {
        return Err("The question exceeds the shared Favorites limit.".into());
    }
    Ok(body)
}
fn next_cursor(value: &Value) -> Result<Option<String>, String> {
    match &value["nextCursor"] {
        Value::Null => Ok(None),
        Value::String(cursor) => Ok((!cursor.is_empty()).then(|| cursor.clone())),
        _ => Err("Question history returned an invalid cursor".into()),
    }
}
fn advance(cursors: &mut crate::opencode_history::Cursors, cursor: &str) -> Result<(), String> {
    if cursor.is_empty() || cursor.len() > 4096 {
        return Err("Question history returned an invalid cursor".into());
    }
    if !cursors
        .advance(cursor)
        .map_err(|_| "Question history repeated its cursor")?
    {
        return Err("Question history exceeded its cursor budget".into());
    }
    Ok(())
}
fn codex_page(turns: &Value, request: &Request, limit: usize) -> Result<Option<String>, String> {
    let turns = turns
        .as_array()
        .ok_or("Question history returned no turn array")?;
    let mut matching = turns
        .iter()
        .filter(|turn| turn["id"].as_str() == Some(&request.hit.turn_id));
    let Some(turn) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err("Question history returned a duplicate Turn".into());
    }
    let items = turn["items"]
        .as_array()
        .ok_or("Question history returned no turn items")?;
    if !items.iter().any(|item| {
        item["id"].as_str() == Some(&request.hit.item_id) && item["type"] == "agentMessage"
    }) {
        return Err("The saved response is no longer in its original Turn".into());
    }
    let question = items
        .iter()
        .find(|item| item["type"] == "userMessage")
        .ok_or("The original Turn contains no user question")?;
    source(question, limit).map(Some)
}
fn validate_session(message: &Value, thread: &str) -> Result<(), String> {
    let info = &message["info"];
    if info["role"] == "user" && !message["parts"].is_array() {
        return Err("OpenCode returned incomplete question parts".into());
    }
    if !info["sessionID"].is_null() && info["sessionID"].as_str() != Some(thread) {
        return Err("Question history belongs to another OpenCode session".into());
    }
    for part in message["parts"].as_array().into_iter().flatten() {
        if (!part["sessionID"].is_null() && part["sessionID"].as_str() != Some(thread))
            || (!part["messageID"].is_null()
                && (part["messageID"].as_str().is_none()
                    || part["messageID"].as_str() != info["id"].as_str()))
        {
            return Err("Question history returned a foreign OpenCode part".into());
        }
    }
    Ok(())
}
fn opencode_page(
    value: Value,
    request: &Request,
    limit: usize,
) -> Result<(bool, Option<String>), String> {
    for message in value
        .as_array()
        .ok_or("Question history returned no OpenCode message array")?
    {
        validate_session(message, &request.context.thread)?;
    }
    let items = crate::opencode::message_items(&value)?;
    let mut target = false;
    let mut question = None;
    for item in items {
        if text(&item, "id") == request.hit.item_id {
            if text(&item, "turnId") != request.hit.turn_id || item["type"] != "agentMessage" {
                return Err("The OpenCode response no longer has its original parent".into());
            }
            target = true;
        }
        if item["type"] == "userMessage" && text(&item, "id") == request.hit.turn_id {
            let body = source(&item, limit)?;
            if question.replace(body).is_some() {
                return Err("Question history returned a duplicate user message".into());
            }
        }
    }
    Ok((target, question))
}

pub async fn load(api: &Api, request: Request, limit: usize) -> Result<String, String> {
    if limit == 0
        || limit > crate::conversation_output::MAX_TEXT
        || !identifier(&request.context.thread)
        || !identifier(&request.hit.turn_id)
        || !identifier(&request.hit.item_id)
    {
        return Err("Invalid original-question source".into());
    }
    tokio::time::timeout(Duration::from_secs(20), load_inner(api, request, limit))
        .await
        .map_err(|_| "Original question lookup timed out".to_string())?
}
async fn load_inner(api: &Api, request: Request, limit: usize) -> Result<String, String> {
    let mut cursor: Option<String> = None;
    let mut visited = crate::opencode_history::Cursors::default();
    if request.backend_kind == "codex" {
        for _ in 0..MAX_PAGES {
            let value = match conversation_focus::read(
                api,
                &request.context,
                "turns",
                cursor.clone().map(|cursor| ("cursor", cursor.into())),
            )
            .await
            {
                Ok(value) => value,
                Err(error) if conversation_focus::unsupported(&error) => {
                    let value =
                        conversation_focus::read(api, &request.context, "read", None).await?;
                    return crate::worker::compute(move || {
                        if value
                            .pointer("/thread/id")
                            .and_then(Value::as_str)
                            .is_some_and(|id| id != request.context.thread)
                        {
                            return Err("Question history belongs to another Codex thread".into());
                        }
                        codex_page(
                            value
                                .pointer("/thread/turns")
                                .ok_or("Question history returned no thread turns")?,
                            &request,
                            limit,
                        )?
                        .ok_or_else(|| {
                            "The original question is no longer in this conversation".into()
                        })
                    })
                    .await;
                }
                Err(error) => return Err(error),
            };
            let target = request.clone();
            let (question, next) = crate::worker::compute(move || {
                Ok((
                    codex_page(&value["data"], &target, limit)?,
                    next_cursor(&value)?,
                ))
            })
            .await?;
            if let Some(question) = question {
                return Ok(question);
            }
            let Some(next) = next else {
                return Err("The original question is no longer in this conversation".into());
            };
            advance(&mut visited, &next)?;
            cursor = Some(next);
        }
    } else if request.backend_kind == "opencode" && request.context.backend == "opencode" {
        let path = crate::opencode::message_path(
            &request.context.thread,
            &request.directory,
            &request.hit.turn_id,
        );
        let (status, value) = api.json_status_optional(Method::GET, &path, None).await?;
        if status.is_success() {
            return crate::worker::compute(move || {
                validate_session(&value, &request.context.thread)?;
                if value["info"]["id"].as_str() != Some(&request.hit.turn_id)
                    || value["info"]["role"] != "user"
                {
                    return Err("OpenCode returned a different original question".into());
                }
                opencode_page(Value::Array(vec![value]), &request, limit)?
                    .1
                    .ok_or_else(|| "OpenCode returned no user question".into())
            })
            .await;
        }
        if !matches!(
            status,
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ) {
            return Err(crate::gateway::response_error(status, &value));
        }
        let mut target_seen = false;
        let mut question: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let (value, next) = crate::opencode_history::page(
                api,
                &request.context.thread,
                &request.directory,
                cursor.as_deref(),
            )
            .await?;
            let target = request.clone();
            let (seen, found) =
                crate::worker::compute(move || opencode_page(value, &target, limit)).await?;
            target_seen |= seen;
            if let Some(found) = found {
                if question.as_ref().is_some_and(|old| old != &found) {
                    return Err("The original question changed during retrieval".into());
                }
                question = Some(found);
            }
            if target_seen && question.is_some() {
                return Ok(question.unwrap());
            }
            let Some(next) = next else {
                return Err(
                    "The original OpenCode question is not in the retrieved history".into(),
                );
            };
            advance(&mut visited, &next)?;
            cursor = Some(next);
        }
    } else {
        return Err("Unsupported original-question backend".into());
    }
    Err("Original question lookup exceeded its 64-page budget".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        session_search::{Context, Filter, Occurrence},
        test_http::server,
    };
    use serde_json::json;
    fn request(kind: &str) -> Request {
        Request {
            backend_kind: kind.into(),
            directory: "/tmp/Question 中文".into(),
            serial: 0,
            context: Context {
                backend: if kind == "codex" {
                    "custom-codex"
                } else {
                    "opencode"
                }
                .into(),
                thread: "session".into(),
                generation: 9,
            },
            hit: Occurrence {
                turn_id: "user".into(),
                item_id: "answer".into(),
                kind: Filter::Assistant,
                snippet: String::new(),
                highlight: 0..0,
                source: None,
                block: None,
                table_row: None,
                paragraph_page: None,
                paragraph_range: None,
            },
        }
    }
    fn turn() -> Value {
        let mut items = vec![
            json!({"id":"q","type":"userMessage","content":[{"type":"text","text":"Original 中文😀\r\n\0question"}]}),
        ];
        items.extend((0..100).map(|i| json!({"id":format!("tool-{i}"),"type":"commandExecution","command":"read","aggregatedOutput":""})));
        items.push(json!({"id":"answer","type":"agentMessage","text":"Visible response"}));
        json!({"id":"user","items":items})
    }
    fn user() -> Value {
        json!({"info":{"id":"user","role":"user","sessionID":"session"},"parts":[{"id":"q-part","messageID":"user","sessionID":"session","type":"text","text":"Original 中文😀\r\n\0question"}]})
    }
    #[tokio::test]
    async fn codex_reads_original_question_beyond_context_window_without_resuming() {
        let (api, task) = server(vec![
            (200, json!({"data":[],"nextCursor":"opaque:中文/?&"})),
            (200, json!({"data":[turn()],"nextCursor":null})),
        ])
        .await;
        assert_eq!(
            load(&api, request("codex"), 1024).await.unwrap(),
            "Original 中文😀\r\n\0question"
        );
        for raw in task.await.unwrap() {
            assert!(raw
                .to_ascii_lowercase()
                .contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
            assert!(raw.starts_with("POST /studio/native/conversation/read "));
            let body: Value = serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["operation"], "turns");
            assert_eq!(body["backend"], "custom-codex");
            assert_eq!(body["threadId"], "session");
        }
    }
    #[tokio::test]
    async fn codex_legacy_reads_remain_private_and_reject_foreign_threads() {
        let (api, task) = server(vec![
            (501, json!({"error":"unsupported method"})),
            (200, json!({"thread":{"id":"session","turns":[turn()]}})),
        ])
        .await;
        assert_eq!(
            load(&api, request("codex"), 1024).await.unwrap(),
            "Original 中文😀\r\n\0question"
        );
        let rows = task.await.unwrap();
        assert!(rows[1].contains("\"operation\":\"read\""));
        let (api, task) = server(vec![
            (501, json!({"error":"unsupported method"})),
            (200, json!({"thread":{"id":"foreign","turns":[turn()]}})),
        ])
        .await;
        assert!(load(&api, request("codex"), 1024)
            .await
            .unwrap_err()
            .contains("another Codex"));
        task.await.unwrap();
    }
    #[tokio::test]
    async fn opencode_uses_direct_user_read_and_falls_back_to_cursor_pages() {
        let (api, task) = server(vec![(200, user())]).await;
        assert_eq!(
            load(&api, request("opencode"), 1024).await.unwrap(),
            "Original 中文😀\r\n\0question"
        );
        let rows = task.await.unwrap();
        assert!(rows[0].starts_with("GET /opencode/session/session/message/user?directory="));
        let answer = json!({"info":{"id":"message","role":"assistant","sessionID":"session","parentID":"user"},"parts":[{"id":"answer","type":"text","text":"Visible response"}]});
        // The reusable simple HTTP fixture has no paging headers. Check merging
        // the two independent pages through the same production projection.
        assert_eq!(
            opencode_page(json!([answer]), &request("opencode"), 1024).unwrap(),
            (true, None)
        );
        assert_eq!(
            opencode_page(json!([user()]), &request("opencode"), 1024).unwrap(),
            (false, Some("Original 中文😀\r\n\0question".into()))
        );
    }
    #[tokio::test]
    async fn denied_direct_reads_do_not_silently_scan_and_cursors_are_bounded() {
        let (api, task) = server(vec![(403, json!({"error":"denied"}))]).await;
        assert!(load(&api, request("opencode"), 1024)
            .await
            .unwrap_err()
            .contains("denied"));
        assert_eq!(task.await.unwrap().len(), 1);
        let (api, task) = server(vec![
            (200, json!({"data":[],"nextCursor":"same"})),
            (200, json!({"data":[],"nextCursor":"same"})),
        ])
        .await;
        assert!(load(&api, request("codex"), 1024)
            .await
            .unwrap_err()
            .contains("repeated"));
        assert_eq!(task.await.unwrap().len(), 2);
    }
    #[test]
    fn source_validation_rejects_wrong_parent_foreign_parts_and_size_overflow() {
        let mut page = turn();
        page["items"].as_array_mut().unwrap().pop();
        assert!(codex_page(&json!([page]), &request("codex"), 1024).is_err());
        assert!(codex_page(&json!([turn()]), &request("codex"), 4)
            .unwrap_err()
            .contains("limit"));
        let mut message = user();
        message["parts"][0]["sessionID"] = "foreign".into();
        assert!(opencode_page(json!([message]), &request("opencode"), 1024).is_err());
        assert!(next_cursor(&json!({"nextCursor":42})).is_err());
        assert!(next_cursor(&json!({"nextCursor":""})).unwrap().is_none());
        assert!(source(&json!({"type":"userMessage"}), 1024).is_err());
        let mut message = user();
        message["parts"] = Value::Null;
        assert!(opencode_page(json!([message]), &request("opencode"), 1024).is_err());
        let path = crate::opencode::message_path("s/中文", "/tmp/Q & A", "u/?#");
        assert!(path.starts_with("/opencode/session/s%2F%E4%B8%AD%E6%96%87/message/u%2F%3F%23?"));
        assert!(path.contains("directory=%2Ftmp%2FQ+%26+A"));
    }
}
