//! Native shell state, separate from shared content/backend settings and drafts.
use crate::{json_error, json_response, GatewayState};
use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    theme: String,
    window_width: i32,
    window_height: i32,
    maximized: bool,
    sidebar_collapsed: bool,
    selected_backend: String,
    selected_session: String,
    notification_ids: Vec<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            window_width: 1280,
            window_height: 850,
            maximized: false,
            sidebar_collapsed: false,
            selected_backend: String::new(),
            selected_session: String::new(),
            notification_ids: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    theme: Option<String>,
    window_width: Option<i32>,
    window_height: Option<i32>,
    maximized: Option<bool>,
    sidebar_collapsed: Option<bool>,
    selected_backend: Option<String>,
    selected_session: Option<String>,
    notification_ids: Option<Vec<String>>,
}

impl Preferences {
    fn patch(&mut self, patch: Patch) -> Result<(), String> {
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(width) = patch.window_width {
            self.window_width = width;
        }
        if let Some(height) = patch.window_height {
            self.window_height = height;
        }
        if let Some(maximized) = patch.maximized {
            self.maximized = maximized;
        }
        if let Some(collapsed) = patch.sidebar_collapsed {
            self.sidebar_collapsed = collapsed;
        }
        if let Some(backend) = patch.selected_backend {
            self.selected_backend = backend;
        }
        if let Some(session) = patch.selected_session {
            self.selected_session = session;
        }
        if let Some(ids) = patch.notification_ids {
            self.notification_ids = ids;
        }
        self.validate()
    }
    fn validate(&self) -> Result<(), String> {
        if self.notification_ids.len() > crate::notification_ids::MAX_IDS
            || self
                .notification_ids
                .iter()
                .any(|id| !crate::notification_ids::valid(id))
        {
            return Err("Invalid notification identifiers".into());
        }
        if !matches!(self.theme.as_str(), "system" | "light" | "dark")
            || !(640..=8192).contains(&self.window_width)
            || !(480..=8192).contains(&self.window_height)
        {
            return Err("Invalid native appearance or window size".into());
        }
        if self.selected_backend.len() > 64
            || self.selected_session.len() > 256
            || self
                .selected_backend
                .chars()
                .chain(self.selected_session.chars())
                .any(char::is_control)
        {
            return Err("Invalid native session selection".into());
        }
        Ok(())
    }
}

fn load(path: &std::path::Path) -> Result<Preferences, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Preferences::default())
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(32769)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 32768 {
        return Err("Native preferences exceed the size limit".into());
    }
    let preferences: Preferences =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    preferences.validate()?;
    Ok(preferences)
}

async fn operation(state: GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let Ok(_guard) = state.preferences_lock.lock() else {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Preferences lock is unavailable",
            );
        };
        let path = state.preferences_path.with_file_name("gnome.json");
        let result = (|| -> Result<Preferences, String> {
            let mut preferences = load(&path)?;
            if let Some(patch) = patch {
                preferences.patch(patch)?;
                let parent = path.parent().ok_or("Missing preferences directory")?;
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                let mut temporary =
                    tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
                serde_json::to_writer_pretty(&mut temporary, &preferences)
                    .map_err(|error| error.to_string())?;
                temporary.flush().map_err(|error| error.to_string())?;
                temporary
                    .as_file()
                    .sync_all()
                    .map_err(|error| error.to_string())?;
                temporary
                    .persist(&path)
                    .map_err(|error| error.to_string())?;
            }
            Ok(preferences)
        })();
        match result {
            Ok(preferences) => json_response(StatusCode::OK, &preferences),
            Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}
pub async fn get(State(state): State<GatewayState>) -> Response<Body> {
    operation(state, None).await
}
pub async fn patch(
    State(state): State<GatewayState>,
    patch: Result<Json<Patch>, axum::extract::rejection::JsonRejection>,
) -> Response<Body> {
    match patch {
        Ok(Json(patch)) => operation(state, Some(patch)).await,
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error.body_text()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_preserves_other_fields_and_rejects_drafts_or_invalid_values() {
        let mut preferences = Preferences {
            selected_backend: "codex".into(),
            selected_session: "thread".into(),
            ..Preferences::default()
        };
        preferences
            .patch(serde_json::from_str(r#"{"theme":"dark"}"#).unwrap())
            .unwrap();
        assert_eq!(preferences.selected_session, "thread");
        assert_eq!(preferences.theme, "dark");
        assert!(serde_json::from_str::<Patch>(r#"{"draft":"must not persist"}"#).is_err());
        assert!(preferences
            .patch(serde_json::from_str(r#"{"windowWidth":20}"#).unwrap())
            .is_err());
    }
    #[test]
    fn notification_ledger_is_bounded_and_fits_the_native_file_budget() {
        let id = crate::notification_ids::tag(&"b".repeat(64), &"t".repeat(256)).unwrap();
        let mut prefs = Preferences::default();
        prefs.notification_ids = vec![id; crate::notification_ids::MAX_IDS];
        prefs.validate().unwrap();
        assert!(serde_json::to_vec_pretty(&prefs).unwrap().len() < 32768);
        prefs.notification_ids.push("invalid".into());
        assert!(prefs.validate().is_err());
    }
}
