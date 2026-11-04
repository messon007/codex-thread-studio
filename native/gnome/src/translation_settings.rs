//! Shared translation preferences, including backend-scoped model/effort choices.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub engine: String,
    pub ollama_model: String,
    #[serde(default)]
    pub models: BTreeMap<String, String>,
    #[serde(default)]
    pub efforts: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Backend {
    pub id: String,
    pub name: String,
    pub kind: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub values: Preferences,
    pub backends: Vec<Backend>,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save {
        revision: String,
        values: Preferences,
    },
    Models,
}
#[derive(Debug)]
pub enum Reply {
    Settings(Snapshot),
    Models(Vec<String>),
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Reply, String> {
    let (method, path, body) = match request {
        Request::Read => (
            reqwest::Method::GET,
            "/studio/native/translation-settings",
            None,
        ),
        Request::Save { revision, values } => (
            reqwest::Method::PATCH,
            "/studio/native/translation-settings",
            Some(serde_json::json!({"expectedRevision":revision,"values":values})),
        ),
        Request::Models => (reqwest::Method::GET, "/studio/ollama/models", None),
    };
    let value = api.json(method, path, body.as_ref()).await?;
    crate::worker::compute(move || {
        if path.ends_with("/models") {
            let models = value["models"]
                .as_array()
                .ok_or("Invalid local model catalog")?;
            if models.len() > 4096 {
                return Err("Local model catalog exceeds its budget".into());
            }
            let mut names = models
                .iter()
                .map(|m| {
                    m["name"]
                        .as_str()
                        .filter(|n| !n.is_empty() && n.len() <= 256)
                        .map(str::to_owned)
                        .ok_or_else(|| "Invalid local model name".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            names.sort();
            names.dedup();
            return Ok(Reply::Models(names));
        }
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid translation settings response")?;
        if snapshot.revision.len() != 64
            || !snapshot.revision.bytes().all(|c| c.is_ascii_hexdigit())
            || snapshot.backends.len() > 32
            || snapshot.values.models.len() > 32
            || snapshot.values.efforts.len() > 32
        {
            return Err("Invalid translation settings response".into());
        }
        Ok(Reply::Settings(snapshot))
    })
    .await
}
pub async fn load(api: &crate::gateway::Api) -> Result<Snapshot, String> {
    match execute(api, Request::Read).await? {
        Reply::Settings(v) => Ok(v),
        _ => unreachable!(),
    }
}
