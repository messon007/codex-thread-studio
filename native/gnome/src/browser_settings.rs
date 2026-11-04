//! Scoped browser settings transport and worker-side response validation.
use crate::browser_policy::NavigationPreferences;
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub values: NavigationPreferences,
    #[serde(default, rename = "restoreTabs")]
    pub restore_tabs: bool,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save {
        revision: String,
        values: NavigationPreferences,
        restore_tabs: bool,
    },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Snapshot, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save {
            revision,
            values,
            restore_tabs,
        } => (
            reqwest::Method::PATCH,
            Some(
                serde_json::json!({"expectedRevision":revision,"values":values,"restoreTabs":restore_tabs}),
            ),
        ),
    };
    let value = api
        .json(method, "/studio/native/browser-settings", body.as_ref())
        .await?;
    crate::worker::compute(move || {
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid browser settings response")?;
        if snapshot.revision.len() != 64
            || !snapshot.revision.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Invalid browser settings revision".into());
        }
        Ok(snapshot)
    })
    .await
}
