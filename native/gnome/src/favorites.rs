//! Shared favorites API; native widgets never open a second database.
use crate::{gateway::Api, model::Item};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    pub scope: String,
    pub backend: String,
    pub thread_id: String,
    pub thread_title: String,
    pub project_path: String,
    pub turn_id: String,
    pub item_id: String,
    pub title: String,
    pub question: String,
    pub content: String,
    pub note: String,
    pub tags: Vec<String>,
    pub created_at: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub thread_title: String,
    pub snippet: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub items: Vec<Summary>,
    pub total: usize,
    pub all_total: usize,
}
#[derive(Debug)]
pub enum Request {
    Preview(Favorite),
    List(String),
    Get(String),
    Save { favorite: Favorite, new: bool },
    Delete(String),
    Export,
}
#[derive(Debug)]
pub enum Reply {
    Draft {
        favorite: Favorite,
        preview: Arc<Item>,
    },
    List(List),
    Detail {
        favorite: Favorite,
        preview: Arc<Item>,
    },
    Saved {
        favorite: Favorite,
        preview: Arc<Item>,
    },
    Deleted,
    Export(String),
}
pub fn path(id: &str) -> String {
    format!(
        "/studio/favorites/{}",
        percent_encoding::utf8_percent_encode(id, percent_encoding::NON_ALPHANUMERIC)
    )
}
pub fn tags(value: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for tag in value
        .split([',', '，'])
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
    {
        if !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.to_owned());
        }
    }
    tags
}
pub fn title(content: &str) -> String {
    let first = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Favorite AI response");
    first
        .trim_start_matches(['#', '*', '-', ' ', '>'])
        .chars()
        .take(80)
        .collect()
}
fn detail(value: Value, saved: bool) -> Result<Reply, String> {
    let favorite: Favorite =
        serde_json::from_value(value).map_err(|_| "The server returned an invalid favorite")?;
    let mut preview = Item::from_value(
        &serde_json::json!({"id":favorite.item_id,"turnId":favorite.turn_id,"type":"agentMessage","text":favorite.content}),
    );
    preview.role = "Favorite".into();
    preview.rich = crate::chat_content::Content::parse(&preview.body).ok();
    let preview = Arc::new(preview);
    Ok(if saved {
        Reply::Saved { favorite, preview }
    } else {
        Reply::Detail { favorite, preview }
    })
}
pub async fn execute(api: &Api, request: Request) -> Result<Reply, String> {
    match request {
        Request::Preview(favorite) => {
            crate::worker::compute(move || {
                let value = serde_json::to_value(favorite).map_err(|error| error.to_string())?;
                let Reply::Detail { favorite, preview } = detail(value, false)? else {
                    unreachable!()
                };
                Ok(Reply::Draft { favorite, preview })
            })
            .await
        }
        Request::List(query) => {
            let query =
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC);
            let value = api
                .json(
                    Method::GET,
                    &format!("/studio/favorites?q={query}&limit=2000"),
                    None,
                )
                .await?;
            crate::worker::compute(move || {
                serde_json::from_value(value)
                    .map(Reply::List)
                    .map_err(|_| "The server returned an invalid favorites list".into())
            })
            .await
        }
        Request::Get(id) => {
            let value = api.json(Method::GET, &path(&id), None).await?;
            crate::worker::compute(move || detail(value, false)).await
        }
        Request::Save { favorite, new } => {
            let endpoint = if new {
                "/studio/favorites".into()
            } else {
                path(&favorite.id)
            };
            let value = crate::worker::compute(move || {
                serde_json::to_value(favorite).map_err(|error| error.to_string())
            })
            .await?;
            let value = api
                .json(
                    if new { Method::POST } else { Method::PUT },
                    &endpoint,
                    Some(&value),
                )
                .await?;
            crate::worker::compute(move || detail(value, true)).await
        }
        Request::Delete(id) => {
            api.json(Method::DELETE, &path(&id), None).await?;
            Ok(Reply::Deleted)
        }
        Request::Export => api
            .text("/studio/favorites/export")
            .await
            .map(Reply::Export),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn saved_content_keeps_full_markdown_and_source_identity() {
        let value = json!({"id":"f","scope":"selection","backend":"codex","threadId":"t","threadTitle":"Title","projectPath":"/tmp/project","turnId":"turn","itemId":"item","title":"标题","question":"question","content":format!("# 中文😀\n\n{}", "content ".repeat(20000)),"note":"note","tags":["test"],"createdAt":"2026-10-24T12:00:00Z"});
        let Reply::Detail { favorite, preview } = detail(value, false).unwrap() else {
            panic!()
        };
        assert_eq!(favorite.item_id, preview.id);
        assert!(favorite.content.len() > 128 * 1024);
        assert!(preview.truncated);
        assert!(preview.body.len() < 129 * 1024);
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("complete.md");
        crate::conversation_output::export(preview.output.clone().unwrap(), output_path.clone())
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(output_path).unwrap(),
            favorite.content
        );
        assert!(preview.rich.is_some());
        assert_eq!(tags(" foo,bar，foo , 中文"), ["foo", "bar", "中文"]);
        assert_eq!(path("f/?#"), "/studio/favorites/f%2F%3F%23");
    }
}
