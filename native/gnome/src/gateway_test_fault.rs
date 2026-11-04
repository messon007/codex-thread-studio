//! Private acceptance latch, absent from optimized builds.
pub async fn export_latch(path: &std::path::Path) {
    let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from)
    else {
        return;
    };
    if !std::env::var("STUDIO_GNOME_SMOKE_DISCOVERY").is_ok_and(|m| m.starts_with("runtime-"))
        || std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
            != Some(root.join("config"))
        || std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
            != Some(root.join("fake-codex.py"))
        || path != root.join("gateway-export.txt")
    {
        return;
    }
    let marker = root.join("export-admitted");
    let _ = crate::worker::compute(move || std::fs::write(marker, b"1").map_err(|e| e.to_string()))
        .await;
    loop {
        let flag = root.join("hold-gateway-export");
        if !crate::worker::compute(move || Ok(flag.exists()))
            .await
            .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
