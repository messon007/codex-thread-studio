//! Read only the private fake portal's bounded, atomically replaced trace.
use gtk::{gio, glib};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};
pub async fn read(root: &Path) -> Result<Value, String> {
    let path = root.join("file-portal.json");
    gio::spawn_blocking(move || {
        use std::io::Read;
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(serde_json::json!({})),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(262145)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 262144 {
            return Err("Private portal trace exceeded its budget".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Portal fixture worker stopped")?
}
pub async fn wait(root: &Path, key: &str, count: usize) -> Result<Value, String> {
    let at = Instant::now();
    loop {
        let value = read(root).await?;
        if let Some(error) = value["failure"].as_str() {
            return Err(error.into());
        }
        if value[key].as_array().is_some_and(|v| v.len() >= count) {
            return Ok(value);
        }
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!(
                "Private portal timed out: {key} >= {count}; {value}"
            ));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
}
