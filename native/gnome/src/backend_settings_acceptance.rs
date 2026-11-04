//! Native controls plus the shared file, startup registry and private provider process.
use super::*;
use std::path::{Path, PathBuf};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("Backend configuration timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn read(path: PathBuf) -> Result<String, String> {
    gio::spawn_blocking(move || std::fs::read_to_string(path).map_err(|error| error.to_string()))
        .await
        .map_err(|_| "Configuration fixture worker stopped")?
}
async fn write(path: PathBuf, text: String) -> Result<(), String> {
    gio::spawn_blocking(move || std::fs::write(path, text).map_err(|error| error.to_string()))
        .await
        .map_err(|_| "Configuration fixture worker stopped")?
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    until("session and language", || {
        ui.state.borrow().history_verified && ui.language.smoke_language().is_some()
    })
    .await?;
    let mut pulse = super::conversation_pages_acceptance::Probe::new();
    if std::env::var("STUDIO_GNOME_SMOKE_BACKENDS").ok().as_deref() == Some("restart") {
        let backend = ui
            .state
            .borrow()
            .backends
            .iter()
            .find(|backend| backend.id == "native-extra")
            .cloned()
            .ok_or("Saved backend not registered after restart")?;
        if backend.name != "<Source> {count} 中文"
            || ui
                .state
                .borrow()
                .backends
                .iter()
                .any(|backend| backend.id == "disabled-extra")
        {
            return Err("Backend names or enabled flags changed after restart".into());
        }
        let started = gio::spawn_blocking({
            let path = root.join("extra.pid");
            move || path.exists()
        })
        .await
        .map_err(|_| "Configuration fixture worker stopped")?;
        if started {
            return Err("Configured backend started before selection".into());
        }
        ui.switch_backend(backend);
        until("new backend catalog", || {
            let state = ui.state.borrow();
            state.backend == "native-extra"
                && state.ready
                && !state.catalog_loading
                && state.threads.iter().any(|thread| thread.id == "target")
        })
        .await?;
        let thread = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "target")
            .cloned()
            .unwrap();
        ui.select(thread);
        until("configured backend history", || {
            ui.state.borrow().backend == "native-extra" && ui.state.borrow().history_verified
        })
        .await?;
        return Ok(
            json!({"ok":true,"restartApplied":true,"lazyBackend":true,"configuredSelection":true,"literalName":true,"disabledExcluded":true,"heartbeat":pulse.stop()}),
        );
    }
    ui.composer
        .buffer()
        .set_text("Unsent backend settings draft 中文😀");
    ui.open_settings();
    let settings = ui
        .settings
        .borrow()
        .clone()
        .ok_or("Settings did not open")?;
    let panel = settings.smoke_backend_config();
    until("configuration read", || {
        panel.smoke_ready() && panel.dialog.is_mapped()
    })
    .await?;
    if panel.dialog.title() != "后端配置"
        || panel.smoke_text() != "broken JSON\n"
        || !panel.smoke_status().contains("invalid")
    {
        return Err("Invalid configuration was not editable in the selected language".into());
    }
    let path = root.join("config/codex-thread-studio/backends.json");
    panel.smoke_edit("{");
    panel.smoke_validate();
    until("invalid validation", || panel.smoke_ready()).await?;
    panel.smoke_save();
    until("invalid save", || panel.smoke_ready()).await?;
    if panel.smoke_text() != "{" || read(path.clone()).await? != "broken JSON\n" {
        return Err("Invalid save changed the file or discarded the draft".into());
    }
    let source = gio::spawn_blocking({ let root = root.to_path_buf(); move || {
        serde_json::to_string_pretty(&json!({"version":1,"backends":[
        {"id":"native-extra","name":"<Source> {count} 中文","tag":"NC","adapter":"codex-app-server","command":root.join("fake-extra.py"),"args":["--fixture-extra","中文 arg"],"enabled":true},
        {"id":"disabled-extra","name":"Disabled fixture","tag":"OFF","adapter":"codex-app-server","command":root.join("never-start-disabled"),"args":[],"enabled":false}
        ]})).map(|source| source.replace('\n', "\r\n") + "\r\n").map_err(|error| error.to_string())
    }}).await.map_err(|_| "Configuration fixture worker stopped")??;
    panel.smoke_edit(&source);
    panel.smoke_validate();
    until("valid schema", || panel.smoke_ready()).await?;
    if !panel.smoke_status().contains("1") || panel.may_close() || settings.may_close() {
        return Err("Validation marked an unsaved configuration as saved".into());
    }
    panel.dialog.close();
    panel.smoke_confirm("cancel")?;
    if panel.smoke_text() != source || !panel.dialog.is_visible() {
        return Err("Cancel closing discarded configuration edits".into());
    }
    panel.smoke_reload();
    panel.smoke_confirm("cancel")?;
    if panel.smoke_text() != source {
        return Err("Cancel reload discarded configuration edits".into());
    }
    let outside = "{\"version\":1,\"backends\":[]}\n";
    write(path.clone(), outside.into()).await?;
    panel.smoke_save();
    until("outside-file conflict", || panel.smoke_ready()).await?;
    if !panel.smoke_status().contains("changed outside")
        || panel.smoke_text() != source
        || read(path.clone()).await? != outside
    {
        return Err("Conflicted save overwrote external data or the local draft".into());
    }
    panel.smoke_reload();
    panel.smoke_confirm("discard")?;
    until("confirmed reload", || {
        panel.smoke_ready() && panel.smoke_text() == outside
    })
    .await?;
    panel.smoke_edit(&source);
    panel.smoke_save();
    if panel.may_close() || settings.may_close() {
        return Err("Pending configuration save permits closing its owner".into());
    }
    panel.dialog.close();
    if !panel.dialog.is_visible() {
        return Err("Pending configuration save closed its editor".into());
    }
    until("explicit save", || panel.smoke_ready() && panel.may_close()).await?;
    if read(path.clone()).await? != source {
        return Err("Save did not preserve the exact UTF-8/CRLF configuration".into());
    }
    if ui
        .state
        .borrow()
        .backends
        .iter()
        .any(|backend| backend.id == "native-extra")
    {
        return Err("Saving changed the live backend registry".into());
    }
    panel.smoke_undo();
    if panel.smoke_text() != outside || read(path.clone()).await? != source {
        return Err("Undo after save changed the file or lost its history".into());
    }
    panel.smoke_redo();
    if panel.smoke_text() != source {
        return Err("Redo after save lost configuration text".into());
    }
    if !panel.may_close() {
        panel.smoke_save();
        until("save after redo", || {
            panel.smoke_ready() && panel.may_close()
        })
        .await?;
    }
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    until("dark JSON highlighting", || {
        panel.smoke_scheme().as_deref() == Some("Adwaita-dark")
    })
    .await?;
    glib::timeout_future(Duration::from_millis(350)).await;
    crate::window_capture::capture(&ui.window, &root.join("backend-editor-dark.png")).await?;
    write(root.join("expected-backends.json"), source).await?;
    panel.dialog.close();
    until("editor closed", || !panel.dialog.is_mapped()).await?;
    settings.dialog.close();
    until("settings closed", || ui.settings.borrow().is_none()).await?;
    if ui.composer_text() != "Unsent backend settings draft 中文😀" {
        return Err("Configuration editing changed the composer draft".into());
    }
    Ok(
        json!({"ok":true,"invalidRepair":true,"validationNoWrite":true,"closeCancel":true,"reloadCancel":true,"busyCloseGuard":true,"conflictDraftRetention":true,"explicitSave":true,"exactUnicodeCrlf":true,"undoRedoAcrossSave":true,"noHotReload":true,"darkJsonHighlighting":true,"draftRetained":true,"heartbeat":pulse.stop()}),
    )
}
