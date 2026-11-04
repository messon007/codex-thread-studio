//! Provider activity snapshots and concise text projection, all on Rust workers.
use crate::{
    conversation_output::{self, Reference},
    model::text,
};
use serde_json::Value;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub translated: bool,
    pub failed: bool,
}
/// Display only. Tool completion/failure never determines session idle state.
pub fn status(value: &Value) -> Option<Status> {
    let raw = value["status"]
        .as_str()
        .or_else(|| value.pointer("/status/type").and_then(Value::as_str))?;
    if raw.trim().is_empty() {
        return None;
    }
    let (label, failed) = match raw {
        "inProgress" | "running" | "busy" | "retry" => (Some("In progress"), false),
        "completed" | "success" => (Some("Done"), false),
        "failed" | "error" => (Some("Failed"), true),
        "interrupted" | "cancelled" | "canceled" => (Some("Stopped"), false),
        "pending" => (Some("Pending"), false),
        _ => (None, false),
    };
    Some(Status {
        text: label
            .map(str::to_owned)
            .unwrap_or_else(|| raw.chars().filter(|c| !c.is_control()).take(64).collect()),
        translated: label.is_some(),
        failed,
    })
}
#[derive(serde::Serialize)]
struct Progress<'a> {
    id: &'a Value,
    #[serde(rename = "type")]
    kind: &'a Value,
    phase: &'a Value,
    text: &'a str,
}
#[derive(serde::Serialize)]
struct Reasoning<'a> {
    id: &'a Value,
    #[serde(rename = "type")]
    kind: &'a Value,
    status: &'a Value,
    summary: &'a Value,
    content: &'a Value,
}

pub fn snapshot(value: &Value, partial: bool) -> Option<Reference> {
    let kind = text(value, "type");
    if kind.is_empty()
        || kind == "userMessage"
        || (kind == "agentMessage" && value["phase"] != "commentary")
    {
        return None;
    }
    if partial {
        return Some(Reference{token:String::new(),bytes:0,unavailable:Some("Activity details are incomplete. Reload the conversation or history page to retrieve the complete provider item.".into())});
    }
    if kind == "agentMessage" {
        // Progress text has the same internal Map-marker filtering as its chat
        // presentation; do not expose hidden coordination text through details.
        return Some(conversation_output::retain_json(&Progress {
            id: &value["id"],
            kind: &value["type"],
            phase: &value["phase"],
            text: crate::session_map_text::visible(text(value, "text")),
        }));
    }
    if kind == "reasoning" {
        // Inspect the visible summary/content supplied by the provider. Opaque
        // encrypted reasoning envelopes are not useful activity presentation.
        return Some(conversation_output::retain_json(&Reasoning {
            id: &value["id"],
            kind: &value["type"],
            status: &value["status"],
            summary: &value["summary"],
            content: &value["content"],
        }));
    }
    Some(conversation_output::retain_json(value))
}
pub fn array_text(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().or_else(|| v["text"].as_str()))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn reasoning(value: &Value) -> String {
    let summary = array_text(&value["summary"]);
    if summary.is_empty() {
        array_text(&value["content"])
    } else {
        summary
    }
}
pub fn command(value: &Value) -> String {
    match &value["command"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}
pub fn plan(value: &Value) -> String {
    if let Some(text) = value["text"].as_str().filter(|s| !s.is_empty()) {
        return text.into();
    }
    let rows = value["plan"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|step| {
            format!(
                "{}  {}",
                step["status"].as_str().unwrap_or("pending"),
                text(step, "step")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    [text(value, "explanation"), &rows]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Item;
    use serde_json::json;
    fn json_snapshot(reference: Reference) -> Value {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("activity.json");
        conversation_output::export(reference, path.clone()).unwrap();
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    #[test]
    fn concise_tool_projection_preserves_arguments_errors_status_and_future_fields() {
        let value = json!({"id":"tool","type":"mcpToolCall","server":"literal <server>","tool":"lookup","arguments":{"query":"中文😀\0"},"status":"failed","error":{"message":"Provider error"},"future":{"metadata":[1,2,3]},"result":null});
        let item = Item::from_value(&value);
        assert!(!item.body.contains("query"));
        assert_eq!(item.status.as_ref().unwrap().text, "Failed");
        assert!(item.status.as_ref().unwrap().failed);
        assert_eq!(json_snapshot(item.details.clone().unwrap()), value);
        let mut changed = value.clone();
        changed["arguments"]["query"] = "updated".into();
        let next = Item::from_value(&changed);
        assert_eq!(item.body, next.body);
        assert_ne!(item, next);
        assert!(item.weight() >= item.body.capacity() + item.details.as_ref().unwrap().weight());
        assert!(Item::from_value(
            &json!({"type":"userMessage","content":[{"text":"private draft"}]})
        )
        .details
        .is_none());
        assert!(
            Item::from_value(&json!({"type":"agentMessage","text":"final"}))
                .details
                .is_none()
        );
        let mut partial = value;
        partial["_studioTruncated"] = true.into();
        assert!(Item::from_value(&partial)
            .details
            .unwrap()
            .unavailable
            .is_some());
    }
    #[test]
    fn reasoning_falls_back_to_visible_content_and_command_arrays_are_readable() {
        let value = json!({"id":"r","type":"reasoning","summary":[],"content":[{"text":"Visible 中文 reasoning"}],"encryptedContent":"opaque envelope"});
        let item = Item::from_value(&value);
        assert_eq!(item.body, "Visible 中文 reasoning");
        let raw = json_snapshot(item.details.unwrap());
        assert_eq!(raw["content"], value["content"]);
        assert!(raw.get("encryptedContent").is_none());
        let item = Item::from_value(
            &json!({"id":"c","type":"commandExecution","command":["cargo","test","--locked"],"aggregatedOutput":"done","exitCode":23,"status":"failed"}),
        );
        assert_eq!(item.body, "$ cargo test --locked\ndone");
        assert_eq!(json_snapshot(item.details.unwrap())["exitCode"], 23);
        let value = json!({"id":"f","type":"fileChange","changes":[{"path":"literal <file>","kind":{"type":"add"},"diff":"+ 中文"}]});
        assert_eq!(
            json_snapshot(Item::from_value(&value).details.unwrap()),
            value
        );
    }
    #[test]
    fn json_serialization_rejects_oversize_instead_of_retaining_a_partial_snapshot() {
        let value = json!({"payload":"x".repeat(conversation_output::MAX_TEXT+16)});
        let snapshot = conversation_output::retain_json(&value);
        assert!(snapshot.token.is_empty());
        assert!(snapshot.unavailable.unwrap().contains("16 MiB"));
        assert!(snapshot.bytes > conversation_output::MAX_TEXT);
    }
    #[test]
    fn unknown_activity_status_stays_literal_bounded_and_separate_from_turn_state() {
        let value = json!({"status":"<future>\0中文😀".repeat(100)});
        let status = status(&value).unwrap();
        assert!(!status.translated && !status.failed);
        assert!(status.text.starts_with("<future>"));
        assert!(status.text.chars().count() <= 64 && !status.text.contains('\0'));
        assert!(super::status(&json!({"status":""})).is_none());
    }
    #[test]
    fn progress_details_filter_private_map_suffix_and_deltas_keep_the_captured_revision() {
        let value = json!({"id":"progress","type":"agentMessage","phase":"commentary",
            "text":format!("Visible progress\n{}private coordination{}", crate::session_map_text::START, crate::session_map_text::END)});
        let mut item = Item::from_value(&value);
        let before = item.details.clone().unwrap();
        let raw = json_snapshot(before.clone());
        assert_eq!(raw["text"].as_str().unwrap().trim(), "Visible progress");
        assert!(!raw.to_string().contains("private coordination"));
        item.append(" more visible text");
        assert!(item.body.contains("more visible text"));
        assert_eq!(item.details, Some(before));
        assert_eq!(json_snapshot(item.details.unwrap()), raw);
    }
}
