//! Session rules are compiled on workers; GTK only applies prepared rows.
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Values {
    pub hidden_session_directories: Vec<String>,
    pub session_directory_ignore: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub values: serde_json::Value,
    #[serde(default, rename = "validationError")]
    pub validation_error: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Editor {
    pub hidden: Option<String>,
    pub rules: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Reply {
    pub snapshot: Snapshot,
    pub filter: Result<Arc<crate::session_directory_policy::Filter>, String>,
    pub editor: Editor,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save { revision: String, values: Values },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Reply, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save { revision, values } => (
            reqwest::Method::PATCH,
            Some(serde_json::json!({"expectedRevision":revision,"values":values})),
        ),
    };
    let value = api
        .json(method, "/studio/native/session-visibility", body.as_ref())
        .await?;
    crate::worker::compute(move || decode(value)).await
}
// Even malformed arrays are bounded by the preferences document. Avoid sending
// huge lines, control characters, or enormous row counts to GTK's text layout.
fn editable(value: &serde_json::Value, hidden: bool) -> Option<String> {
    let values = value.as_array()?;
    if values.len() > 4096 {
        return None;
    }
    let mut bytes = 0usize;
    let mut lines = Vec::with_capacity(values.len());
    for value in values {
        let line = value.as_str()?;
        if line.len() > 4096
            || line.chars().any(char::is_control)
            || (hidden && line.trim().is_empty())
        {
            return None;
        }
        bytes = bytes.checked_add(line.len() + 1)?;
        if bytes > 2 * 1024 * 1024 {
            return None;
        }
        lines.push(line);
    }
    Some(lines.join("\n"))
}
pub fn decode(value: serde_json::Value) -> Result<Reply, String> {
    let snapshot: Snapshot =
        serde_json::from_value(value).map_err(|_| "Invalid session-visibility response")?;
    if snapshot.revision.len() != 64 || !snapshot.revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid session-visibility revision".into());
    }
    let editor = Editor {
        hidden: editable(&snapshot.values["hiddenSessionDirectories"], true),
        rules: editable(&snapshot.values["sessionDirectoryIgnore"], false),
    };
    let filter = if let Some(error) = snapshot.validation_error.as_ref() {
        Err(error.chars().take(512).collect())
    } else {
        let values = serde_json::from_value::<Values>(snapshot.values.clone())
            .map_err(|_| "Saved visibility values must be lists of strings".to_owned());
        values.and_then(|values| {
            crate::session_directory_policy::Filter::new(
                &values.hidden_session_directories,
                &values.session_directory_ignore,
            )
            .map(Arc::new)
        })
    };
    Ok(Reply {
        snapshot,
        filter,
        editor,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn recovery_projection_is_bounded_and_keeps_safe_fields_editable() {
        let reply=decode(json!({"revision":"a".repeat(64),"values":{"hiddenSessionDirectories":{"broken":true},"sessionDirectoryIgnore":["**/build/"]},"validationError":"Invalid saved folders"})).unwrap();
        assert!(reply.filter.is_err());
        assert!(reply.editor.hidden.is_none());
        assert_eq!(reply.editor.rules.as_deref(), Some("**/build/"));
        for value in [
            json!(["x".repeat(4097)]),
            json!(["bad\u{0000}line"]),
            json!(["bad\nline"]),
            json!(vec!["x"; 4097]),
        ] {
            assert!(editable(&value, false).is_none());
        }
        assert!(editable(&json!([""]), true).is_none());
        assert_eq!(
            editable(&json!(["", "# comment"]), false).as_deref(),
            Some("\n# comment")
        );
    }
}
