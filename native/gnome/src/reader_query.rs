//! Native adapters for the shared reader query service, not another evaluator.
use crate::gateway::Api;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn jq_supported(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "json" | "jsonl" | "ndjson"
            )
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub key: u64,
    pub generation: u64,
    pub revision: String,
}

#[derive(Debug)]
pub enum Operation {
    Jq { filter: String, slurp: bool },
    JqPage { id: String, page: usize },
    JqClose { id: String },
}

fn result_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

impl Operation {
    fn payload(&self, revision: &str) -> Result<Value, String> {
        Ok(match self {
            Self::Jq { filter, slurp } => {
                if filter.trim().is_empty() || filter.len() > 16384 || filter.contains('\0') {
                    return Err("Enter a jq filter of at most 16 KiB".into());
                }
                json!({"revision":revision,"filter":filter,"slurp":slurp})
            }
            Self::JqPage { id, page } if result_id(id) && *page < 10000 => {
                json!({"revision":revision,"id":id,"page":page})
            }
            Self::JqClose { id } if result_id(id) => {
                json!({"revision":revision,"id":id,"close":true})
            }
            _ => return Err("Invalid reader query result identity or page".into()),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JqPage {
    kind: String,
    pub id: String,
    pub revision: String,
    pub page: usize,
    pub pages: usize,
    pub total_values: usize,
    pub text: String,
    pub elapsed_ms: f64,
}

impl JqPage {
    fn validate(&self, revision: &str, operation: &Operation) -> Result<(), String> {
        if self.kind != "jqQuery"
            || self.revision != revision
            || !result_id(&self.id)
            || self.pages == 0
            || self.pages > 10000
            || self.page >= self.pages
            || self.text.len() > 48 * 1024
            || self.text.contains('\0')
            || !self.elapsed_ms.is_finite()
            || self.elapsed_ms < 0.0
            || self.total_values > 256 * 1024 * 1024
            || match operation {
                Operation::Jq { .. } => self.page != 0,
                Operation::JqPage { id, page } => self.id != *id || self.page != *page,
                Operation::JqClose { .. } => true,
            }
        {
            return Err(
                "Query response does not match the requested document or page budget".into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Reply {
    Jq(JqPage),
    Closed,
}

pub async fn execute(
    api: &Api,
    reader: &str,
    document: &str,
    revision: &str,
    operation: Operation,
) -> Result<Reply, String> {
    let payload = operation.payload(revision)?;
    let value = api
        .json_document(
            Method::POST,
            &format!("/studio/native/markdown/{reader}/jq-query"),
            Some(&payload),
            Some(document),
        )
        .await?;
    if matches!(operation, Operation::JqClose { .. }) {
        return if value["closed"] == true {
            Ok(Reply::Closed)
        } else {
            Err("The query result was not closed".into())
        };
    }
    let page: JqPage = serde_json::from_value(value)
        .map_err(|error| format!("Invalid query response: {error}"))?;
    page.validate(revision, &operation)?;
    Ok(Reply::Jq(page))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_or_misrouted_pages_and_oversized_text_are_rejected() {
        let mut page = JqPage {
            kind: "jqQuery".into(),
            id: "result-id".into(),
            revision: "source-revision".into(),
            page: 1,
            pages: 2,
            total_values: 2,
            text: "\"中文😀\"\n".into(),
            elapsed_ms: 1.0,
        };
        let operation = Operation::JqPage {
            id: "result-id".into(),
            page: 1,
        };
        assert!(page.validate("source-revision", &operation).is_ok());
        assert!(page.validate("old-revision", &operation).is_err());
        assert!(page
            .validate(
                "source-revision",
                &Operation::JqPage {
                    id: "other".into(),
                    page: 1
                }
            )
            .is_err());
        page.text = "x".repeat(48 * 1024 + 1);
        assert!(page.validate("source-revision", &operation).is_err());
    }
}
