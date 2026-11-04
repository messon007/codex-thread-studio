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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub content_bytes: usize,
    pub question_bytes: usize,
}

#[derive(Debug)]
pub struct Capture {
    pub favorite: Favorite,
    pub content: Option<crate::conversation_output::Reference>,
    pub question: Option<crate::conversation_output::Reference>,
    pub question_lookup: Option<crate::conversation_focus::Request>,
}
impl Capture {
    fn resolve(mut self, limits: Limits) -> Result<(Favorite, Option<String>), String> {
        if [limits.content_bytes, limits.question_bytes]
            .iter()
            .any(|limit| *limit == 0 || *limit > crate::conversation_output::MAX_TEXT)
        {
            return Err("Invalid Favorites source limit".into());
        }
        fn restore(
            current: &mut String,
            source: Option<crate::conversation_output::Reference>,
            limit: usize,
            message: &'static str,
        ) -> Result<(), String> {
            // Limits come from the shared store. The immutable snapshot is the
            // same one used by the full-output viewer; GTK never expands it.
            if let Some(source) = source {
                let (text, clipped) = crate::conversation_output::prefix(&source, limit)?;
                if clipped {
                    return Err(message.into());
                }
                *current = text;
            }
            if current.len() > limit {
                return Err(message.into());
            }
            Ok(())
        }
        restore(
            &mut self.favorite.content,
            self.content,
            limits.content_bytes,
            "Complete source exceeds the shared Favorites limit. Select response text to save an excerpt.",
        )?;
        let question_issue = restore(
            &mut self.favorite.question,
            self.question,
            limits.question_bytes,
            "The question exceeds the shared Favorites limit.",
        )
        .err();
        if question_issue.is_some() {
            self.favorite.question.clear();
        }
        Ok((self.favorite, question_issue))
    }
}
#[derive(Debug)]
pub enum Request {
    Preview(Favorite),
    Capture(Capture),
    List(String),
    Get(String),
    Save { favorite: Favorite, new: bool },
    Delete(String),
    Export,
}
#[derive(Debug)]
pub enum Reply {
    Captured {
        favorite: Favorite,
        preview: Arc<Item>,
        question_issue: Option<String>,
    },
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
        Request::Capture(mut capture) => {
            let limits: Limits = serde_json::from_value(
                api.json(Method::GET, "/studio/native/favorites/limits", None)
                    .await?,
            )
            .map_err(|_| "The server returned invalid Favorites source limits")?;
            let question_lookup = capture.question_lookup.take();
            let question_limit = limits.question_bytes;
            let retry_question = capture.question.as_ref().is_none_or(|source| {
                source.unavailable.is_some() || source.bytes <= question_limit
            });
            let (mut favorite, mut question_issue) =
                crate::worker::compute(move || capture.resolve(limits)).await?;
            if retry_question && (question_issue.is_some() || favorite.question.is_empty()) {
                if let Some(request) = question_lookup {
                    match crate::conversation_question::load(api, request, question_limit).await {
                        Ok(question) => {
                            favorite.question = question;
                            question_issue = None;
                        }
                        Err(error) => {
                            favorite.question.clear();
                            question_issue = Some(error);
                        }
                    }
                }
            }
            crate::worker::compute(move || {
                let value = serde_json::to_value(favorite).map_err(|error| error.to_string())?;
                let Reply::Detail { favorite, preview } = detail(value, false)? else {
                    unreachable!()
                };
                Ok(Reply::Captured {
                    favorite,
                    preview,
                    question_issue,
                })
            })
            .await
        }
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
    fn capture() -> Capture {
        Capture {
            favorite: serde_json::from_value(json!({"id":"f","scope":"message","backend":"codex","threadId":"t","threadTitle":"Title","projectPath":"/tmp/project","turnId":"turn","itemId":"item","title":"Source","question":"question","content":"preview","note":"","tags":[],"createdAt":"now"})).unwrap(),
            content: None,
            question: None,
            question_lookup: None,
        }
    }
    #[test]
    fn capture_restores_complete_response_and_question_without_expanding_preview() {
        let text = format!(
            "# Original 中文😀\r\n\r\n{}\0tail",
            "context 中文😀\r\n".repeat(7500)
        );
        assert!(text.len() > 128 * 1024 && text.len() < 192 * 1024);
        let question = format!("Original question\r\n{}?", "字😀".repeat(20000));
        let item = Item::from_value(
            &json!({"id":"item","turnId":"turn","type":"agentMessage","text":text}),
        );
        let user = Item::from_value(
            &json!({"id":"user","turnId":"turn","type":"userMessage","content":[{"type":"text","text":question}]}),
        );
        assert!(item.truncated && user.truncated);
        let mut source = capture();
        source.content = item.output.clone();
        source.question = user.output.clone();
        let (favorite, issue) = source
            .resolve(Limits {
                content_bytes: 192 * 1024,
                question_bytes: 192 * 1024,
            })
            .unwrap();
        assert!(issue.is_none());
        assert_eq!(favorite.content, text);
        assert_eq!(favorite.question, question);
        assert_eq!(favorite.scope, "message");
        assert!(item.body.len() < 129 * 1024);
        assert!(user.body.len() < 129 * 1024);
    }
    #[test]
    fn capture_never_silently_clips_oversized_or_missing_snapshots() {
        let mut source = capture();
        source.content = Some(crate::conversation_output::retain("full original".into()));
        assert!(source
            .resolve(Limits {
                content_bytes: 4,
                question_bytes: 100
            })
            .unwrap_err()
            .contains("shared Favorites limit"));
        let mut source = capture();
        source.content = Some(crate::conversation_output::Reference {
            token: "expired-favorite-source".into(),
            bytes: 9,
            unavailable: None,
        });
        assert!(source
            .resolve(Limits {
                content_bytes: 100,
                question_bytes: 100
            })
            .is_err());
        let mut source = capture();
        source.favorite.scope = "selection".into();
        source.favorite.content = "Chosen excerpt".into();
        let (favorite, issue) = source
            .resolve(Limits {
                content_bytes: 100,
                question_bytes: 100,
            })
            .unwrap();
        assert!(issue.is_none());
        assert_eq!(favorite.content, "Chosen excerpt");
        assert_eq!(favorite.scope, "selection");
    }
    #[test]
    fn unavailable_optional_question_does_not_block_saving_the_response() {
        let mut source = capture();
        source.question = Some(crate::conversation_output::retain(
            "oversized question".into(),
        ));
        let (favorite, issue) = source
            .resolve(Limits {
                content_bytes: 100,
                question_bytes: 4,
            })
            .unwrap();
        assert_eq!(favorite.content, "preview");
        assert_eq!(favorite.question, "");
        assert!(issue.unwrap().contains("question exceeds"));
    }
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
