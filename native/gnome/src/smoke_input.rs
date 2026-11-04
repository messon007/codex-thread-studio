//! Debug-only input handshake with a fixture-owned Wayland compositor.
use gtk::{gio, glib};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub async fn request(root: &Path, request: Value) -> Result<Value, String> {
    if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_none()
        || std::env::var_os("STUDIO_GNOME_SMOKE_DIR").as_deref() != Some(root.as_os_str())
    {
        return Err("Input handshake requires the private fixture root".into());
    }
    let path = root.to_owned();
    gio::spawn_blocking(move || {
        let temporary = path.join("clipboard-request.tmp");
        std::fs::write(&temporary, serde_json::to_vec(&request).unwrap())
            .and_then(|_| std::fs::rename(temporary, path.join("clipboard-request.json")))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Clipboard fixture worker stopped")??;
    let start = Instant::now();
    loop {
        let path = root.join("clipboard-response.json");
        let reply = gio::spawn_blocking(move || match std::fs::read(&path) {
            Ok(bytes) => {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
                serde_json::from_slice::<Value>(&bytes)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        })
        .await
        .map_err(|_| "Clipboard fixture worker stopped")??;
        if let Some(value) = reply {
            if let Some(error) = value["error"].as_str() {
                return Err(error.to_owned());
            }
            return Ok(value);
        }
        if start.elapsed() > Duration::from_secs(12) {
            return Err("Clipboard input fixture did not reply".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
