//! Browser navigation controls in an isolated application and owned compositor.
use super::*;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(25) {
            return Err(format!("Browser settings acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
pub(super) async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    until("catalog and history", || {
        ui.state.borrow().ready && !ui.state.borrow().loading && ui.window.is_mapped()
    })
    .await?;
    let url = std::env::var("STUDIO_GNOME_SMOKE_BROWSER_SETTINGS_URL")
        .map_err(|_| "Missing private browser fixture URL")?;
    if std::env::var("STUDIO_GNOME_SMOKE_BROWSER_RESTORE_PHASE").as_deref() == Ok("restart") {
        ui.show_tool("browser");
        return ui.browser.smoke_tab_restart(&url).await;
    }
    let keyboard = std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some();
    if keyboard {
        let key = root.join("browser-settings-key");
        gio::spawn_blocking(move || std::fs::write(key, b"settings"))
            .await
            .map_err(|_| "Keyboard fixture worker stopped")?
            .map_err(|e| e.to_string())?;
    } else {
        ui.open_settings();
    }
    until("Settings shortcut", || ui.settings.borrow().is_some()).await?;
    let settings = ui.settings.borrow().clone().unwrap();
    let panel = settings.smoke_browser();
    panel.smoke_ready().await?;
    glib::timeout_future(Duration::from_millis(300)).await;
    smoke::capture_dialog(&panel.dialog, &root.join("browser-settings.png"));
    let guards = panel
        .smoke_guards(
            &root.join("config/codex-thread-studio/settings.json"),
            || settings.may_close(),
        )
        .await?;
    ui.show_tool("browser");
    let live = ui.browser.smoke_navigation(&panel, &url).await?;
    let restoration = ui.browser.smoke_restoration(&panel, &url).await?;
    panel.smoke_restore(true).await?;
    panel.dialog.close();
    until("Browser preferences closed", || !panel.dialog.is_mapped()).await?;
    settings.dialog.close();
    until("Settings closed", || ui.settings.borrow().is_none()).await?;
    ui.browser.smoke_prepare_tab_shutdown(&url).await?;
    // Let the setup save settle before introducing a genuine foreign revision.
    until("shutdown setup saved", || ui.browser.smoke_tabs_idle()).await?;
    ui.browser.smoke_tab_save_failure(root).await?;
    ui.browser.smoke_prepare_tab_shutdown(&url).await?;
    ui.window.close();
    if !ui.window.is_mapped() {
        return Err("Window closed before the pending tab save completed".into());
    }
    Ok(
        json!({"ok":true,"tabSaveCloseGuard":true,"saveConflictCancelRecovery":true,"settingsShortcut":keyboard,"guards":guards,"live":live,"restoration":restoration}),
    )
}
