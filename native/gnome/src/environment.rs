//! Shared project environment profiles; secrets stay redacted when read.
use crate::gateway::Api;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
pub type Applied = std::sync::Arc<tokio::sync::Mutex<HashMap<(String, String), String>>>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub root: String,
    pub configured: bool,
    pub revision: String,
    pub variables: BTreeMap<String, String>,
    pub secret_names: Vec<String>,
    pub network_policy: String,
    pub allowed_hosts: Vec<String>,
    pub cache_variables: BTreeMap<String, String>,
}
pub async fn read(api: &Api, root: &str) -> Result<Profile, String> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("root", root)
        .finish();
    let value = api
        .json(Method::GET, &format!("/studio/environment?{query}"), None)
        .await?;
    crate::worker::compute(move || {
        serde_json::from_value(value)
            .map_err(|error| format!("Invalid environment profile: {error}"))
    })
    .await
}
pub async fn prepare(
    api: &Api,
    backend: &str,
    thread: &str,
    root: &str,
    applied: &mut HashMap<(String, String), String>,
) -> Result<(), String> {
    let profile = read(api, root).await?;
    let key = (thread.to_owned(), profile.root.clone());
    if !profile.configured && !applied.contains_key(&key) {
        return Ok(());
    }
    if applied.get(&key) == Some(&profile.revision) {
        return Ok(());
    }
    api.json(
        Method::POST,
        "/studio/environment/apply",
        Some(&json!({"backend":backend,"threadId":thread,"root":profile.root,"excludeTurns":true})),
    )
    .await?;
    if applied.len() >= 256 {
        applied.clear();
    }
    applied.insert(key, profile.revision);
    Ok(())
}
