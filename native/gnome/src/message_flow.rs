//! Shared message-flow settings; strict decoding and bounded Rust I/O.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub queue_depth: u8,
    pub continue_behavior: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub values: Values,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save { revision: String, values: Values },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Snapshot, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save { revision, values } => (
            reqwest::Method::PATCH,
            Some(serde_json::json!({"expectedRevision":revision,"values":values})),
        ),
    };
    let value = api
        .json(method, "/studio/native/message-flow", body.as_ref())
        .await?;
    crate::worker::compute(move || {
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid message-flow settings response")?;
        if snapshot.revision.len() != 64
            || !snapshot.revision.bytes().all(|b| b.is_ascii_hexdigit())
            || !(1..=3).contains(&snapshot.values.queue_depth)
            || !matches!(
                snapshot.values.continue_behavior.as_str(),
                "sessionModelDraft" | "ollamaDraft" | "quickSend"
            )
        {
            return Err("Invalid message-flow settings response".into());
        }
        Ok(snapshot)
    })
    .await
}
