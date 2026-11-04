//! Startup backend-file transport; schema validation remains in the Rust gateway.
use serde::Deserialize;
pub const MAX_BYTES: usize = 256 * 1024;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Check {
    pub validation_error: Option<String>,
    pub backend_count: Option<usize>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub revision: String,
    pub path: String,
    pub source: String,
    pub exists: bool,
    pub validation_error: Option<String>,
    pub backend_count: Option<usize>,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Validate(String),
    Save { revision: String, source: String },
}
#[derive(Debug)]
pub enum Reply {
    Read(Snapshot),
    Saved(Snapshot),
    Checked(Check),
}
fn validate_check(error: &Option<String>, count: Option<usize>) -> Result<(), String> {
    if error.as_ref().is_some_and(|error| error.len() > 8192)
        || match error {
            Some(_) => count.is_some(),
            None => count.is_none_or(|count| count > 16),
        }
    {
        Err("Invalid backend configuration validation response".into())
    } else {
        Ok(())
    }
}
fn snapshot(value: serde_json::Value) -> Result<Snapshot, String> {
    let snapshot: Snapshot =
        serde_json::from_value(value).map_err(|_| "Invalid backend configuration response")?;
    if snapshot.source.len() > MAX_BYTES
        || snapshot.path.len() > 16 * 1024
        || snapshot.revision.len() != 64
        || !snapshot
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Invalid backend configuration response bounds".into());
    }
    validate_check(&snapshot.validation_error, snapshot.backend_count)?;
    Ok(snapshot)
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Reply, String> {
    let (method, body, kind) = crate::worker::compute(move || {
        Ok(match request {
            Request::Read => (reqwest::Method::GET, None, 0),
            Request::Validate(source) => {
                if source.len() > MAX_BYTES {
                    return Err("Backend configuration exceeds 256 KiB".into());
                }
                (
                    reqwest::Method::POST,
                    Some(serde_json::json!({"source":source})),
                    1,
                )
            }
            Request::Save { revision, source } => {
                if source.len() > MAX_BYTES {
                    return Err("Backend configuration exceeds 256 KiB".into());
                }
                (
                    reqwest::Method::PUT,
                    Some(serde_json::json!({"expectedRevision":revision,"source":source})),
                    2,
                )
            }
        })
    })
    .await?;
    let value = api
        .json(method, "/studio/native/backend-config", body.as_ref())
        .await?;
    crate::worker::compute(move || match kind {
        0 => Ok(Reply::Read(snapshot(value)?)),
        1 => {
            let check: Check = serde_json::from_value(value)
                .map_err(|_| "Invalid backend configuration validation response")?;
            validate_check(&check.validation_error, check.backend_count)?;
            Ok(Reply::Checked(check))
        }
        _ => Ok(Reply::Saved(snapshot(value)?)),
    })
    .await
}
