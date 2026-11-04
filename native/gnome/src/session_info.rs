//! Bounded session metadata/history reads. Generation itself uses the ordinary
//! conversation send path; this module only observes its identified turn.
use crate::{conversation_pages::Source, gateway::Api};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub const QUESTION_BYTES: usize = 16 * 1024;
pub const ROLE_CHARS: usize = 4096;
#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Opening {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub responsibility: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub captured_at: String,
    #[serde(default)]
    pub truncated: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub session_key: String,
    pub revision: String,
    pub message: Option<Opening>,
    pub responsibility: String,
}
impl Stored {
    fn validate(self, key: &str) -> Result<Self, String> {
        if self.session_key != key
            || self.revision.len() != 16
            || !self.revision.bytes().all(|b| b.is_ascii_hexdigit())
            || self.responsibility.chars().count() > ROLE_CHARS
            || self.message.as_ref().is_some_and(|m| {
                m.text.len() > QUESTION_BYTES
                    || m.responsibility.chars().count() > ROLE_CHARS
                    || m.source.len() > 64
                    || m.captured_at.len() > 128
            })
        {
            return Err("Invalid session information response".into());
        }
        Ok(self)
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub session_key: String,
    pub expected_revision: String,
    pub question: Option<String>,
    pub responsibility: Option<String>,
    pub captured_at: String,
    #[serde(default)]
    pub capture_only: bool,
    #[serde(default)]
    pub truncated: bool,
}
#[derive(Clone, Debug)]
pub struct Field {
    pub label: &'static str,
    pub value: String,
}
#[derive(Debug)]
pub struct Loaded {
    pub stored: Stored,
    pub fields: Vec<Field>,
    pub warning: Option<String>,
}
#[derive(Debug)]
pub enum Request {
    Read {
        source: Source,
        capture: bool,
        cancel: Arc<AtomicBool>,
    },
    Save(Edit),
    Observe {
        source: Source,
        turn: String,
        cancel: Arc<AtomicBool>,
    },
}
#[derive(Debug)]
pub enum Reply {
    Loaded(Loaded),
    Saved(Stored),
    Generated(String),
}
#[derive(Debug)]
pub struct Failure {
    pub message: String,
    pub reload_required: bool,
    pub http_status: Option<reqwest::StatusCode>,
}
fn failure(message: String) -> Failure {
    Failure {
        message,
        reload_required: false,
        http_status: None,
    }
}
fn save_rejection(status: reqwest::StatusCode, value: &Value) -> Failure {
    Failure {
        message: crate::gateway::response_error(status, value),
        reload_required: status == reqwest::StatusCode::CONFLICT || status.is_server_error(),
        http_status: Some(status),
    }
}
fn uncertain_save(message: String) -> Failure {
    Failure {
        message,
        reload_required: true,
        http_status: None,
    }
}
pub fn key(source: &Source) -> String {
    format!("{}:{}", source.context.backend, source.context.thread)
}
pub fn can_stop(source: &Source, generation: &str, active: Option<&str>) -> bool {
    active.is_some_and(|active| {
        active == generation
            || (source.backend_kind == "opencode"
                && active == format!("opencode-{}", source.context.thread))
    })
}
fn path(key: &str) -> String {
    format!(
        "/studio/native/session-info?sessionKey={}",
        percent_encoding::utf8_percent_encode(key, percent_encoding::NON_ALPHANUMERIC)
    )
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Session information request cancelled".into())
    } else {
        Ok(())
    }
}
async fn read_store(api: &Api, key: &str) -> Result<Stored, String> {
    let value = api.json(Method::GET, &path(key), None).await?;
    let key = key.to_owned();
    crate::worker::compute(move || {
        serde_json::from_value::<Stored>(value)
            .map_err(|_| "Invalid session information snapshot".into())
            .and_then(|s| s.validate(&key))
    })
    .await
}
async fn save(api: &Api, edit: Edit) -> Result<Stored, Failure> {
    if edit
        .question
        .as_ref()
        .is_some_and(|q| q.len() > QUESTION_BYTES)
    {
        return Err(failure(
            "Opening questions support at most 16 KiB of UTF-8 text".into(),
        ));
    }
    if edit
        .responsibility
        .as_ref()
        .is_some_and(|r| r.chars().count() > ROLE_CHARS)
    {
        return Err(failure(
            "Responsibilities support at most 4,096 characters".into(),
        ));
    }
    let key = edit.session_key.clone();
    let body = serde_json::to_value(edit).map_err(|e| failure(e.to_string()))?;
    let (status, value) = api
        .json_status(Method::PATCH, "/studio/native/session-info", &body)
        .await
        .map_err(uncertain_save)?;
    if !status.is_success() {
        return Err(save_rejection(status, &value));
    }
    crate::worker::compute(move || {
        serde_json::from_value::<Stored>(value)
            .map_err(|_| "Invalid saved session information".into())
            .and_then(|s| s.validate(&key))
    })
    .await
    .map_err(uncertain_save)
}
fn bounded(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn push_text(output: &mut String, text: &str, truncated: &mut bool) {
    if !output.is_empty() && output.len() < QUESTION_BYTES {
        output.push('\n');
    }
    let remaining = QUESTION_BYTES.saturating_sub(output.len());
    let prefix = bounded(text, remaining);
    output.push_str(prefix);
    *truncated |= prefix.len() < text.len();
}
pub fn opening_in_turns(turns: &[Value]) -> Option<(String, bool)> {
    for turn in turns {
        for item in turn["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i["type"] == "userMessage")
        {
            let mut output = String::new();
            let mut truncated = false;
            if let Some(text) = item["content"].as_str() {
                push_text(&mut output, text, &mut truncated);
            } else {
                for part in item["content"].as_array().into_iter().flatten() {
                    if let Some(text) = part.as_str().or_else(|| part["text"].as_str()) {
                        push_text(&mut output, text, &mut truncated);
                    }
                }
            }
            let output = output.trim().to_owned();
            if !output.is_empty() {
                return Some((output, truncated));
            }
        }
    }
    None
}
fn next_cursor(value: &Value, seen: &mut HashSet<String>) -> Result<Option<String>, String> {
    let Some(cursor) = crate::conversation_pages::cursor(value.get("nextCursor"))? else {
        return Ok(None);
    };
    if !seen.insert(cursor.clone()) {
        return Err("The backend repeated its opening history cursor".into());
    }
    Ok(Some(cursor))
}
async fn find_opening(
    api: &Api,
    source: &Source,
    cancel: &AtomicBool,
) -> Result<Option<(String, bool)>, String> {
    if source.backend_kind == "opencode" {
        return opencode_opening(api, source, cancel).await;
    }
    let mut cursor = None;
    let mut seen = HashSet::new();
    for _ in 0..100 {
        cancelled(cancel)?;
        let value = match crate::conversation_focus::read(
            api,
            &source.context,
            "opening",
            cursor.clone().map(|v| ("cursor", Value::String(v))),
        )
        .await
        {
            Ok(v) => v,
            Err(error) if cursor.is_none() && crate::conversation_focus::unsupported(&error) => {
                let value =
                    crate::conversation_focus::read(api, &source.context, "read", None).await?;
                let thread = source.context.thread.clone();
                return crate::worker::compute(move || {
                    if value.pointer("/thread/id").and_then(Value::as_str) != Some(&thread) {
                        return Err("Opening history belongs to a different session".into());
                    }
                    Ok(value
                        .pointer("/thread/turns")
                        .and_then(Value::as_array)
                        .and_then(|t| opening_in_turns(t)))
                })
                .await;
            }
            Err(error) => return Err(error),
        };
        let parsed = crate::worker::compute(move || {
            let turns = value["data"]
                .as_array()
                .ok_or("The backend returned invalid opening history")?;
            if turns.len() > 4 {
                return Err("The backend exceeded the opening history page size".into());
            }
            Ok((
                opening_in_turns(turns),
                value.get("nextCursor").cloned().unwrap_or(Value::Null),
            ))
        })
        .await?;
        if parsed.0.is_some() {
            return Ok(parsed.0);
        }
        cursor = next_cursor(&json!({"nextCursor":parsed.1}), &mut seen)?;
        if cursor.is_none() {
            return Ok(None);
        }
    }
    Err(
        "Opening question identification reached its history limit; enter it manually or reload"
            .into(),
    )
}
async fn opencode_opening(
    api: &Api,
    source: &Source,
    cancel: &AtomicBool,
) -> Result<Option<(String, bool)>, String> {
    let mut cursor = None;
    let mut seen = HashSet::new();
    let mut earliest = None;
    for _ in 0..100 {
        cancelled(cancel)?;
        let (value, next) = api
            .history_page(&crate::opencode::message_page_path(
                &source.context.thread,
                &source.directory,
                cursor.as_deref(),
                80,
            ))
            .await?;
        let candidate = crate::worker::compute(move || {
            let items = crate::opencode::message_items(&value)?;
            Ok(opening_in_turns(&[json!({"items":items})]))
        })
        .await?;
        if candidate.is_some() {
            earliest = candidate;
        }
        cursor = match next {
            None => return Ok(earliest),
            Some(c) if c.len() <= 4096 && seen.insert(c.clone()) => Some(c),
            _ => return Err("OpenCode repeated or exceeded its opening history cursor".into()),
        };
    }
    // Never label the earliest item of a partial tail as the opening question.
    Err(
        "Opening question identification reached its history limit; enter it manually or reload"
            .into(),
    )
}
pub fn fields(value: &Value, source: &Source) -> Result<Vec<Field>, String> {
    let object = if source.backend_kind == "opencode" {
        value
    } else {
        &value["thread"]
    };
    if object["id"].as_str() != Some(&source.context.thread) {
        return Err("Session metadata belongs to a different session".into());
    }
    let mut fields = Vec::new();
    for (label, names) in [
        ("Status", &["status"][..]),
        ("Session tree", &["sessionId"][..]),
        ("Project directory", &["cwd", "directory"][..]),
        ("Source", &["source"][..]),
        ("CLI Version", &["cliVersion", "version"][..]),
        ("Fork source", &["forkedFromId"][..]),
        ("Parent session", &["parentThreadId", "parentID"][..]),
    ] {
        if let Some(v) = names
            .iter()
            .find_map(|k| object.get(k))
            .filter(|v| !v.is_null())
        {
            let value = v.as_str().map(str::to_owned).unwrap_or_else(|| {
                v.get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            });
            if !value.is_empty() && !(label == "Session tree" && value == source.context.thread) {
                fields.push(Field {
                    label,
                    value: bounded(&value, 4096).replace('\0', "�"),
                });
            }
        }
    }
    Ok(fields)
}
async fn load(
    api: &Api,
    source: Source,
    capture: bool,
    cancel: Arc<AtomicBool>,
) -> Result<Loaded, String> {
    let key = key(&source);
    let stored = read_store(api, &key).await?;
    cancelled(&cancel)?;
    let metadata = if source.backend_kind == "opencode" {
        api.json(
            Method::GET,
            &crate::opencode::session_metadata_path(&source.context.thread, &source.directory),
            None,
        )
        .await
    } else {
        crate::conversation_focus::read(api, &source.context, "summary", None).await
    };
    let mut warning = None;
    let mut valid_metadata = false;
    let fields = match metadata {
        Ok(value) => {
            let context = source.clone();
            match crate::worker::compute(move || fields(&value, &context)).await {
                Ok(fields) => {
                    valid_metadata = true;
                    fields
                }
                Err(error) => {
                    warning = Some(error);
                    Vec::new()
                }
            }
        }
        Err(error) => {
            warning = Some(error);
            Vec::new()
        }
    };
    let mut stored = stored;
    if capture && valid_metadata && stored.message.as_ref().is_none_or(|m| m.text.is_empty()) {
        match find_opening(api, &source, &cancel).await {
            Ok(Some((text, truncated))) => {
                cancelled(&cancel)?;
                let captured_at =
                    chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now())
                        .to_rfc3339();
                match save(
                    api,
                    Edit {
                        session_key: key.clone(),
                        expected_revision: stored.revision.clone(),
                        question: Some(text),
                        responsibility: None,
                        captured_at,
                        capture_only: true,
                        truncated,
                    },
                )
                .await
                {
                    Ok(updated) => stored = updated,
                    Err(error) if error.http_status == Some(reqwest::StatusCode::CONFLICT) => {
                        stored = read_store(api, &key).await?;
                        warning=Some("The opening question changed during identification; reload to try again".into());
                    }
                    Err(error) => warning = Some(error.message),
                }
            }
            Ok(None) => {}
            Err(error) => warning = Some(error),
        }
    }
    cancelled(&cancel)?;
    Ok(Loaded {
        stored,
        fields,
        warning,
    })
}
pub fn generation_prompt(opening: &str) -> String {
    format!("Generate a concise session summary that can be used as this session's responsibility for routing future user queries. Describe the domain this session owns and the kinds of requests it should receive. Use the session conversation as primary context.{}\nReturn only the responsibility summary, with no preface or formatting.",if opening.trim().is_empty(){String::new()}else{format!(" The opening question is:\n{}",bounded(opening.trim(),QUESTION_BYTES))})
}
pub fn turn_text(turn: &Value) -> Result<Option<String>, String> {
    match turn["status"].as_str() {
        Some("inProgress") => return Ok(None),
        Some("failed" | "interrupted") => {
            return Err(turn
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Session responsibility generation failed or was stopped")
                .into())
        }
        Some("completed") => {}
        _ => return Err("The backend returned an invalid generation status".into()),
    }
    let mut output = String::new();
    let mut count = 0;
    for text in turn["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| i["type"] == "agentMessage")
        .filter_map(|i| i["text"].as_str())
    {
        if !output.is_empty() && count < ROLE_CHARS {
            output.push('\n');
            count += 1;
        }
        for c in text.chars().take(ROLE_CHARS.saturating_sub(count)) {
            output.push(if c == '\0' { '�' } else { c });
            count += 1;
        }
    }
    let output = output.trim().to_owned();
    if output.is_empty() {
        Err("AI did not return a session responsibility summary".into())
    } else {
        Ok(Some(output))
    }
}
async fn observe(
    api: &Api,
    source: Source,
    turn: String,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    loop {
        cancelled(&cancel)?;
        let result = if source.backend_kind == "opencode" {
            observe_opencode(api, &source, &turn).await?
        } else {
            observe_codex(api, &source, &turn, &cancel).await?
        };
        if let Some(text) = result {
            cancelled(&cancel)?;
            return Ok(text);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("Session responsibility generation timed out; inspect the conversation before retrying".into());
        }
        tokio::time::sleep(Duration::from_millis(600)).await;
    }
}
async fn observe_codex(
    api: &Api,
    source: &Source,
    turn: &str,
    cancel: &AtomicBool,
) -> Result<Option<String>, String> {
    let mut cursor = None;
    let mut seen = HashSet::new();
    for _ in 0..100 {
        cancelled(cancel)?;
        let value = match crate::conversation_focus::read(
            api,
            &source.context,
            "turns",
            cursor.clone().map(|v| ("cursor", Value::String(v))),
        )
        .await
        {
            Ok(value) => value,
            Err(error) if cursor.is_none() && crate::conversation_focus::unsupported(&error) => {
                let value =
                    crate::conversation_focus::read(api, &source.context, "read", None).await?;
                let thread = source.context.thread.clone();
                let turn = turn.to_owned();
                return crate::worker::compute(move || {
                    if value.pointer("/thread/id").and_then(Value::as_str) != Some(&thread) {
                        return Err("Generation history belongs to a different session".into());
                    }
                    Ok(value
                        .pointer("/thread/turns")
                        .and_then(Value::as_array)
                        .and_then(|rows| rows.iter().find(|t| t["id"] == turn))
                        .map(turn_text)
                        .transpose()?
                        .flatten())
                })
                .await;
            }
            Err(error) => return Err(error),
        };
        let turn = turn.to_owned();
        let (found, next) = crate::worker::compute(move || {
            let rows = value["data"]
                .as_array()
                .ok_or("Invalid generation history")?;
            if rows.len() > 4 {
                return Err("Generation history exceeded its page limit".into());
            }
            Ok((
                rows.iter()
                    .find(|t| t["id"] == turn)
                    .map(turn_text)
                    .transpose()?,
                value.get("nextCursor").cloned().unwrap_or(Value::Null),
            ))
        })
        .await?;
        if let Some(found) = found {
            return Ok(found);
        }
        cursor = next_cursor(&json!({"nextCursor":next}), &mut seen)?;
        if cursor.is_none() {
            return Ok(None);
        }
    }
    Err(
        "Generation history reached its paging limit; inspect the conversation before retrying"
            .into(),
    )
}
async fn observe_opencode(
    api: &Api,
    source: &Source,
    turn: &str,
) -> Result<Option<String>, String> {
    let (value, _) = api
        .history_page(&crate::opencode::message_page_path(
            &source.context.thread,
            &source.directory,
            None,
            80,
        ))
        .await?;
    let statuses = api
        .json(
            Method::GET,
            &crate::opencode::status_path(&source.directory),
            None,
        )
        .await?;
    let thread = source.context.thread.clone();
    let turn = turn.to_owned();
    crate::worker::compute(move|| {
        if statuses.get(&thread).and_then(|s|s["type"].as_str()).is_some_and(|s|s!="idle"){return Ok(None);}
        let messages=value.as_array().ok_or("Invalid OpenCode generation history")?;
        let responses=messages.iter().filter(|m|m.pointer("/info/role").and_then(Value::as_str)==Some("assistant") && m.pointer("/info/parentID").and_then(Value::as_str)==Some(&turn)).collect::<Vec<_>>();
        if responses.is_empty() || responses.iter().any(|m|m.pointer("/info/time/completed").is_none_or(Value::is_null)){return Ok(None);}
        if let Some(error)=responses.iter().filter_map(|m|m.pointer("/info/error")).find(|e|!e.is_null()){return Err(error.to_string());}
        let items=responses.into_iter().map(|m|json!({"type":"agentMessage","text":m["parts"].as_array().into_iter().flatten().filter(|p|p["type"]=="text").filter_map(|p|p["text"].as_str()).collect::<Vec<_>>().join("\n")})).collect::<Vec<_>>();
        turn_text(&json!({"status":"completed","items":items}))
    }).await
}
pub async fn execute(api: &Api, request: Request) -> Result<Reply, Failure> {
    match request {
        Request::Read {
            source,
            capture,
            cancel,
        } => load(api, source, capture, cancel)
            .await
            .map(Reply::Loaded)
            .map_err(failure),
        Request::Save(edit) => save(api, edit).await.map(Reply::Saved),
        Request::Observe {
            source,
            turn,
            cancel,
        } => observe(api, source, turn, cancel)
            .await
            .map(Reply::Generated)
            .map_err(failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_rejections_use_http_status_and_uncertain_writes_require_reload() {
        let conflict = save_rejection(
            reqwest::StatusCode::CONFLICT,
            &json!({"error":"The opening message or responsibility changed; reload before saving"}),
        );
        assert!(conflict.reload_required);
        assert_eq!(conflict.http_status, Some(reqwest::StatusCode::CONFLICT));
        assert!(!conflict.message.contains("409"));
        let invalid = save_rejection(
            reqwest::StatusCode::BAD_REQUEST,
            &json!({"error":"Invalid field value 409"}),
        );
        assert!(!invalid.reload_required);
        assert!(
            save_rejection(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                &json!({"error":"Write acknowledgement unavailable"})
            )
            .reload_required
        );
        assert!(uncertain_save("Connection reset".into()).reload_required);
    }
    #[test]
    fn original_question_and_role_limits_use_utf8_bytes_and_unicode_characters() {
        let turns = json!([{"items":[{"type":"agentMessage","text":"later"},{"type":"userMessage","content":[{"text":"中".repeat(6000)}]}]}]);
        let (question, truncated) = opening_in_turns(turns.as_array().unwrap()).unwrap();
        assert!(truncated);
        assert!(question.len() <= QUESTION_BYTES);
        assert!(question.chars().all(|c| c == '中'));
        let turn = json!({"status":"completed","items":[{"type":"agentMessage","text":"😀".repeat(5000)}]});
        assert_eq!(turn_text(&turn).unwrap().unwrap().chars().count(), 4096);
        assert_eq!(turn_text(&json!({"status":"inProgress"})).unwrap(), None);
        assert!(turn_text(&json!({"status":"failed"})).is_err());
    }
    #[test]
    fn prompt_uses_opening_as_data_and_summary_keeps_identity_fields_literal() {
        let source = Source {
            context: crate::session_search::Context {
                backend: "deepseek".into(),
                thread: "session".into(),
                generation: 2,
            },
            backend_kind: "codex".into(),
            directory: "/project".into(),
        };
        assert!(can_stop(&source, "owned-turn", Some("owned-turn")));
        assert!(!can_stop(&source, "owned-turn", Some("foreign-turn")));
        let mut opencode_source = source.clone();
        opencode_source.backend_kind = "opencode".into();
        assert!(can_stop(
            &opencode_source,
            "owned-user",
            Some("opencode-session")
        ));
        assert!(!can_stop(
            &opencode_source,
            "owned-user",
            Some("opencode-other")
        ));
        assert!(!can_stop(&opencode_source, "owned-user", None));
        let fields=fields(&json!({"thread":{"id":"session","cwd":"/project/<b>literal</b>","cliVersion":"v-next","parentThreadId":"parent","forkedFromId":"fork","source":{"subAgent":"review"}}}),&source).unwrap();
        assert!(fields
            .iter()
            .any(|f| f.label == "Project directory" && f.value == "/project/<b>literal</b>"));
        assert!(fields
            .iter()
            .any(|f| f.label == "Parent session" && f.value == "parent"));
        assert!(generation_prompt("Original 中文 question").contains("Original 中文 question"));
        assert!(super::fields(&json!({"thread":{"id":"other"}}), &source).is_err());
    }
}
