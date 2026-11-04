//! Bounded, read-only source hydration for a selected browser-history page.
use super::{Record, *};
use crate::{
    native_router_backend::{self as backend, ResponseItem},
    opencode_server::RouterOperation as Oc,
};

fn valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn clip(value: &str, limit: usize) -> String {
    let mut end = limit.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn user(items: &Value) -> String {
    items
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| i["type"] == "userMessage")
        .flat_map(|i| i["content"].as_array().into_iter().flatten())
        .filter(|p| p["type"] == "text")
        .map(|p| text(&p["text"]))
        .collect::<Vec<_>>()
        .join("\n")
}
async fn metadata(state: &GatewayState, key: &str, cwd: &str) -> Result<Value, String> {
    let (backend, id) = key.split_once(':').ok_or("Invalid saved source identity")?;
    if !valid(id) || !crate::valid_router_session_key(key) || key.len() > 320 {
        return Err("Invalid saved source identity".into());
    }
    let raw = if let Some(instance) = state.codex_backends.get(backend) {
        let value = backend::read(
            &instance.server,
            "thread/read",
            json!({"threadId":id,"includeTurns":false}),
        )
        .await?;
        value["thread"].clone()
    } else if backend == "opencode" {
        let value = state.opencode.router_request(Oc::Read(id), cwd).await?;
        json!({"id":value["id"],"name":value["title"],"cwd":value["directory"],"parentID":value["parentID"],"ephemeral":value["ephemeral"]})
    } else {
        return Err("The saved source backend is no longer configured".into());
    };
    if raw["id"].as_str() != Some(id)
        || text(&raw["cwd"]).is_empty()
        || text(&raw["cwd"]).len() > 4096
        || text(&raw["cwd"]).contains('\0')
        || !cwd.is_empty() && raw["cwd"].as_str() != Some(cwd)
    {
        return Err("Saved source identity or directory changed".into());
    }
    if raw["ephemeral"] == true || state.utility_sessions.visibility()?.hidden(backend, &raw) {
        return Err("Temporary utility sessions are not readable Router history sources".into());
    }
    Ok(raw)
}
struct Turn {
    question: String,
    items: Vec<ResponseItem>,
}
async fn turn(state: &GatewayState, key: &str, cwd: &str, id: &str) -> Result<Turn, String> {
    if !valid(id) {
        return Err("Saved source turn identity is unavailable".into());
    }
    let (kind, thread) = key.split_once(':').ok_or("Invalid saved source identity")?;
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    let mut responses = Vec::new();
    for _ in 0..64 {
        if let Some(instance) = state.codex_backends.get(kind) {
            let mut params =
                json!({"threadId":thread,"limit":4,"sortDirection":"desc","itemsView":"full"});
            if let Some(c) = &cursor {
                params["cursor"] = c.clone().into();
            }
            let value = backend::read(&instance.server, "thread/turns/list", params).await?;
            let rows = value["data"]
                .as_array()
                .filter(|rows| rows.len() <= 4)
                .ok_or("Invalid saved source turn page")?;
            if let Some(turn) = rows.iter().find(|t| t["id"] == id) {
                if turn.get("threadId").is_some_and(|v| v != thread) {
                    return Err("Saved source turn belongs to another conversation".into());
                }
                for item in turn["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|i| i["type"] == "agentMessage" && i["phase"] != "commentary")
                {
                    backend::source(
                        &mut responses,
                        text(&item["id"]),
                        crate::session_map_protocol::visible(text(&item["text"])),
                    );
                }
                return Ok(Turn {
                    question: clip(&user(&turn["items"]), 64 * 1024),
                    items: responses,
                });
            }
            cursor = match value.get("nextCursor") {
                None | Some(Value::Null) => None,
                Some(Value::String(c)) => Some(c.clone()),
                _ => return Err("Invalid saved source cursor".into()),
            };
            if rows.is_empty() && cursor.is_some() {
                return Err("Saved source cursor did not advance".into());
            }
        } else if kind == "opencode" {
            let (value, next) = state
                .opencode
                .router_page(Oc::History(thread, 100, cursor.as_deref()), cwd)
                .await?;
            let rows = value
                .as_array()
                .filter(|r| r.len() <= 100)
                .ok_or("Invalid saved OpenCode message page")?;
            // Messages are chronological within each page. A reply can be on
            // a newer page than its user message, so retain only this turn's
            // bounded response parts while moving toward the original user.
            let mut page_responses = Vec::new();
            for message in rows.iter().filter(|m| m["info"]["parentID"] == id) {
                let info = &message["info"];
                if info.get("sessionID").is_some_and(|v| v != thread) {
                    return Err("Saved OpenCode reply belongs to another conversation".into());
                }
                for part in message["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["type"] == "text")
                {
                    if part.get("sessionID").is_some_and(|v| v != thread)
                        || part.get("messageID").is_some_and(|v| v != &info["id"])
                    {
                        return Err(
                            "Saved OpenCode part belongs to another reply or conversation".into(),
                        );
                    }
                    backend::source(&mut page_responses, text(&part["id"]), text(&part["text"]));
                }
                if let Some(structured) = info.get("structured").filter(|v| !v.is_null()) {
                    backend::source(
                        &mut page_responses,
                        &format!("{}-structured", text(&info["id"])),
                        &structured.to_string(),
                    );
                }
            }
            page_responses.extend(responses);
            responses = Vec::new();
            for part in page_responses {
                backend::source(&mut responses, &part.id, &part.text);
                if let Some(last) = responses.last_mut() {
                    last.excerpt |= part.excerpt;
                }
            }
            if let Some(original) = rows.iter().find(|m| m["info"]["id"] == id) {
                if original["info"]["role"] != "user"
                    || original["info"]
                        .get("sessionID")
                        .is_some_and(|v| v != thread)
                {
                    return Err("Saved OpenCode request identity changed".into());
                }
                let question = original["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["type"] == "text")
                    .map(|p| text(&p["text"]))
                    .collect::<Vec<_>>()
                    .join("\n");
                return Ok(Turn {
                    question: clip(&question, 64 * 1024),
                    items: responses,
                });
            }
            if rows.is_empty() && next.is_some() {
                return Err("Saved OpenCode cursor did not advance".into());
            }
            cursor = next;
        } else {
            return Err("The saved source backend is no longer configured".into());
        }
        let Some(c) = &cursor else {
            break;
        };
        if c.is_empty()
            || c.len() > 4096
            || c.chars().any(char::is_control)
            || !seen.insert(c.clone())
        {
            return Err("Saved source history repeated or returned an invalid cursor".into());
        }
    }
    Err("This request is outside the bounded preview window. Open its source conversation to browse further.".into())
}
pub(super) async fn view(state: &GatewayState, record: &Record) -> Value {
    let saved = &record.saved;
    let direct = user(&saved["directTurn"]["items"]);
    let mut value = base(record);
    let read = async {
        if direct.is_empty() {
            let (_, controller_turn) = record
                .turn_key
                .split_once(':')
                .ok_or("Invalid saved controller turn")?;
            let meta = metadata(state, &record.controller, "").await?;
            let original = turn(
                state,
                &record.controller,
                text(&meta["cwd"]),
                controller_turn,
            )
            .await?;
            if !original.question.is_empty() {
                value["prompt"] = original.question.into();
            }
        }
        Ok::<(), String>(())
    };
    match tokio::time::timeout(Duration::from_secs(15), read).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => append(
            &mut value,
            &format!("Original request preview unavailable: {e}"),
        ),
        Err(_) => append(
            &mut value,
            "Original request preview exceeded its time budget",
        ),
    }
    let read = async {
        let key = text(&saved["decision"]["targetSessionKey"]);
        if key.is_empty() || text(&saved["targetTurnId"]).is_empty() {
            return Ok::<(), String>(());
        }
        let meta = metadata(state, key, "").await?;
        let (kind, id) = key.split_once(':').ok_or("Invalid saved target")?;
        let cwd = text(&meta["cwd"]);
        let title = meta["name"]
            .as_str()
            .or_else(|| meta["title"].as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(id);
        value["target"] =
            json!({"sessionKey":key,"backend":kind,"id":id,"title":clip(title,1200),"cwd":cwd});
        let snapshot = turn(state, key, cwd, text(&saved["targetTurnId"])).await?;
        metadata(state, key, cwd).await?;
        if !snapshot.question.is_empty() {
            value["decision"]["forwardedPrompt"] = snapshot.question.into();
        }
        value["responseItems"] = serde_json::to_value(snapshot.items).map_err(|e| e.to_string())?;
        Ok(())
    };
    match tokio::time::timeout(Duration::from_secs(15), read).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => append(&mut value, &format!("Source preview unavailable: {e}")),
        Err(_) => append(&mut value, "Source preview exceeded its time budget"),
    }
    if value["target"].is_object() && value["responseItems"].as_array().is_some_and(Vec::is_empty) {
        value["response"] = "Open the source conversation to inspect this request.".into();
    }
    value
}
fn append(value: &mut Value, notice: &str) {
    let current = text(&value["dispatchNotice"]);
    value["dispatchNotice"] = clip(&format!("{current}\n{notice}"), 1024).into();
}

fn base(record: &Record) -> Value {
    let saved = &record.saved;
    let direct = user(&saved["directTurn"]["items"]);
    let decision=saved.get("decision").filter(|d|d.is_object()).map(|d|json!({
        "action":clip(text(&d["action"]),32),"targetSessionKey":clip(text(&d["targetSessionKey"]),320),
        "forwardedPrompt":clip(text(&d["forwardedPrompt"]),64*1024),"reason":clip(text(&d["reason"]),4096),"message":clip(text(&d["message"]),4096)}));
    let status = saved["status"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 32 && !s.chars().any(char::is_control))
        .unwrap_or("unknown");
    let source_turn = text(&saved["targetTurnId"]);
    let source_turn = if valid(source_turn) { source_turn } else { "" };
    json!({"id":record.id,"controller":record.controller,"browserHistory":true,
        "prompt":clip(if direct.is_empty(){text(&saved["decision"]["forwardedPrompt"])}else{&direct},64*1024),
        "status":status,"target":null,"targetTurnId":source_turn,"decision":decision,
        "response":"","responseItems":[],"error":clip(text(&saved["error"]),4096),"unread":false,
        "dispatchNotice":"Read-only browser history. Status is the saved delivery state; no delivery is resumed."})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unfamiliar_saved_fields_are_bounded_without_rewriting_the_record() {
        let record = Record {
            id: "browser-1-00000000000000000000000000000000".into(),
            controller: "codex:router".into(),
            turn_key: "codex:turn".into(),
            receipt: String::new(),
            saved: json!({"status":"x".repeat(4000),"decision":{"forwardedPrompt":"中文😀".repeat(20000),"reason":{"newer":"shape"},"futureField":[1,2,3]},"futureTopLevel":true}),
        };
        let original = record.saved.clone();
        let value = base(&record);
        assert_eq!(value["status"], "unknown");
        assert_eq!(value["decision"]["reason"], "");
        assert!(
            text(&value["prompt"]).len() <= 64 * 1024
                && text(&value["decision"]["forwardedPrompt"]).len() <= 64 * 1024
        );
        assert_eq!(record.saved, original);
    }
}
