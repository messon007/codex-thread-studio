//! Worker-validated, bounded address metadata; never a content engine or cookie store.
use crate::browser_policy::BrowserPreferences;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Tabs {
    pub tabs: Vec<String>,
    pub active: usize,
}
impl Tabs {
    // At most eight small addresses. Revalidate local unsaved state when a
    // settings generation changes while its request is in flight.
    pub fn permitted(self, preferences: &BrowserPreferences) -> Self {
        let mut result = Self::default();
        if !preferences.restore_tabs || !preferences.enabled {
            return result;
        }
        for (index, url) in self.tabs.iter().enumerate() {
            if let Ok(url) = crate::browser_policy::validate_browser_url(url, preferences) {
                if index <= self.active {
                    result.active = result.tabs.len();
                }
                result.tabs.push(url.to_string());
            }
        }
        result.active = result.active.min(result.tabs.len().saturating_sub(1));
        result
    }
}
#[derive(Debug, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub preferences: BrowserPreferences,
    #[serde(flatten)]
    pub selection: Tabs,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save { revision: String, selection: Tabs },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Snapshot, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save {
            revision,
            selection,
        } => (
            reqwest::Method::PUT,
            Some(
                serde_json::json!({"expectedRevision":revision,"tabs":selection.tabs,"active":selection.active}),
            ),
        ),
    };
    let value = api
        .json(method, "/studio/native/browser-tabs", body.as_ref())
        .await?;
    crate::worker::compute(move || {
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid saved browser tabs response")?;
        if snapshot.revision.len() != 64
            || !snapshot.revision.bytes().all(|c| c.is_ascii_hexdigit())
            || snapshot.selection.tabs.len() > 8
            || snapshot.selection.active >= snapshot.selection.tabs.len().max(1)
        {
            return Err("Invalid saved browser tabs response".into());
        }
        snapshot.preferences.validate()?;
        for url in &snapshot.selection.tabs {
            if url.len() > 8192 {
                return Err("Saved browser address exceeds the address budget".into());
            }
            crate::browser_policy::validate_browser_url(url, &snapshot.preferences)
                .map_err(|e| e.to_string())?;
        }
        Ok(snapshot)
    })
    .await
}
