//! Real native diagnostics controls against private metadata/provider fixtures.
use super::*;
use std::{path::Path, time::Instant};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(30) {
            return Err(format!("Connection diagnostics timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
fn find<T: IsA<gtk::Widget> + IsA<glib::Object> + Clone + glib::types::StaticType>(
    root: &gtk::Widget,
    test: &impl Fn(&T) -> bool,
) -> Option<T> {
    if let Ok(value) = root.clone().downcast::<T>() {
        if test(&value) {
            return Some(value);
        }
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(value) = find(&widget, test) {
            return Some(value);
        }
    }
    None
}
fn button(root: &gtk::Widget, name: &str) -> Option<gtk::Button> {
    find(root, &|b: &gtk::Button| {
        b.is_mapped()
            && (b.label().as_deref() == Some(name) || b.tooltip_text().as_deref() == Some(name))
    })
}
fn literal(root: &gtk::Widget, text: &str) -> bool {
    find(root, &|l: &gtk::Label| l.text().as_str() == text).is_some()
}
fn text(root: &gtk::Widget, text: &str) -> bool {
    find(root, &|l: &gtk::Label| l.text().contains(text)).is_some()
}
async fn panel(ui: &Rc<Ui>) -> Result<Rc<crate::connections_ui::Panel>, String> {
    let panel = ui
        .connections
        .borrow()
        .clone()
        .ok_or("Connections dialog did not open")?;
    until("loaded dialog", || {
        panel.smoke_state()["loaded"] == true
            && button(panel.dialog.upcast_ref(), "Refresh connection information").is_some()
    })
    .await?;
    Ok(panel)
}
async fn open(ui: &Rc<Ui>) -> Result<Rc<crate::connections_ui::Panel>, String> {
    gtk::prelude::WidgetExt::activate_action(&ui.window, "win.connections", None)
        .map_err(|e| e.to_string())?;
    panel(ui).await
}
async fn refresh(panel: &crate::connections_ui::Panel) -> Result<(), String> {
    let button = button(panel.dialog.upcast_ref(), "Refresh connection information")
        .ok_or("Missing Refresh")?;
    let serial = panel.smoke_state()["serial"].as_u64().unwrap();
    button.emit_clicked();
    button.emit_clicked();
    if panel.smoke_state()["serial"] != serial + 1 {
        return Err("Refresh admitted overlapping diagnostics reads".into());
    }
    until("refreshed snapshot", || {
        panel.smoke_state()["pending"] == false
    })
    .await
}
pub(super) async fn run(ui: &Rc<Ui>, directory: &Path) -> Result<Value, String> {
    until("ready", || {
        let state = ui.state.borrow();
        state.ready && state.history_verified && !state.loading
    })
    .await?;
    let draft = "Unsent connection diagnostics draft 中文😀 !literal";
    ui.composer.buffer().set_text(draft);
    let selected = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("Missing session")?;
    let first = open(ui).await?;
    let root = first.dialog.upcast_ref();
    if first.smoke_state()["events"]["codex"] != "Connected"
        || first.smoke_state()["events"]["extra"] != "Connect on demand"
        || first.smoke_state()["events"]["opencode"] != "Connect on demand"
    {
        return Err(format!(
            "Wrong live connection states: {}",
            first.smoke_state()
        ));
    }
    if !text(root, "must-not-run")
        || !text(root, "native-runtime")
        || !literal(root, "<Native extra> 中文")
        || !text(root, "HTTP/OpenAPI + SSE via Studio gateway")
    {
        return Err("Connection metadata missing or interpreted as markup".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &first.dialog,
        &directory.join("connections-initial.png"),
    )
    .await?;
    first.dialog.close();
    until("closed", || ui.connections.borrow().is_none()).await?;
    if ui.composer_text() != draft || ui.state.borrow().selected != Some(selected.clone()) {
        return Err("Diagnostics changed the conversation or draft".into());
    }
    // Open the same dialog through its actual Backends preference action row.
    ui.open_settings();
    let settings = ui.settings.borrow().clone().ok_or("Missing Settings")?;
    settings.dialog.set_visible_page_name("backends");
    until("connection settings row", || {
        find(settings.dialog.upcast_ref(), &|r: &adw::ActionRow| {
            r.title() == "Connection diagnostics" && r.is_mapped()
        })
        .is_some()
    })
    .await?;
    let row = find(settings.dialog.upcast_ref(), &|r: &adw::ActionRow| {
        r.title() == "Connection diagnostics"
    })
    .unwrap();
    row.emit_by_name::<()>("activated", &[]);
    let current = panel(ui).await?;
    std::fs::write(
        directory.join("diagnostics-unavailable"),
        b"private health failure",
    )
    .map_err(|e| e.to_string())?;
    refresh(&current).await?;
    if !text(current.dialog.upcast_ref(), "OpenCode returned HTTP 503")
        || !text(current.dialog.upcast_ref(), "must-not-run")
    {
        return Err("A failed health check discarded successful backend diagnostics".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &current.dialog,
        &directory.join("connections-unavailable.png"),
    )
    .await?;
    std::fs::remove_file(directory.join("diagnostics-unavailable")).map_err(|e| e.to_string())?;
    refresh(&current).await?;
    if text(current.dialog.upcast_ref(), "OpenCode returned HTTP 503") {
        return Err("Refresh retained an obsolete provider error".into());
    }
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if current.dialog.title() != "连接与账号"
        || !literal(current.dialog.upcast_ref(), "网关版本")
        || !literal(current.dialog.upcast_ref(), "修改此文件后，请重启 Studio。")
        || !literal(current.dialog.upcast_ref(), "<Native extra> 中文")
    {
        return Err("Connection labels did not translate or literal metadata changed".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &current.dialog,
        &directory.join("connections-chinese.png"),
    )
    .await?;
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    current.dialog.close();
    until("nested connection closed", || {
        ui.connections.borrow().is_none()
    })
    .await?;
    settings.dialog.close();
    until("settings closed", || ui.settings.borrow().is_none()).await?;
    // Connect OpenCode normally, then observe both independently ready event streams.
    let index = ui
        .state
        .borrow()
        .backends
        .iter()
        .position(|b| b.kind == "opencode")
        .ok_or("Missing OpenCode")?;
    ui.backend.set_selected(index as u32);
    until("OpenCode connected", || {
        ui.state.borrow().ready && ui.state.borrow().backend == "opencode"
    })
    .await?;
    let current = open(ui).await?;
    if current.smoke_state()["events"]["codex"] != "Connected"
        || current.smoke_state()["events"]["opencode"] != "Connected"
    {
        return Err(format!(
            "Background stream status was lost: {}",
            current.smoke_state()
        ));
    }
    std::fs::write(directory.join("diagnostics-hold"), b"private held metadata")
        .map_err(|e| e.to_string())?;
    button(
        current.dialog.upcast_ref(),
        "Refresh connection information",
    )
    .unwrap()
    .emit_clicked();
    until("held provider read", || {
        directory.join("diagnostics-health-waiting").exists()
    })
    .await?;
    let old_id = current.id;
    current.dialog.close();
    until("closed pending dialog", || {
        ui.connections.borrow().is_none()
    })
    .await?;
    gtk::prelude::WidgetExt::activate_action(&ui.window, "win.connections", None)
        .map_err(|e| e.to_string())?;
    let next = ui
        .connections
        .borrow()
        .clone()
        .ok_or("New diagnostics did not open")?;
    if next.id == old_id {
        return Err("Reopened diagnostics reused a retired dialog identity".into());
    }
    std::fs::remove_file(directory.join("diagnostics-hold")).map_err(|e| e.to_string())?;
    until("reopened snapshot", || {
        next.smoke_state()["loaded"] == true && next.smoke_state()["pending"] == false
    })
    .await?;
    // The owned proxy closes the real gateway parent pipe; the application receives
    // an actual managed-process stop and later performs its normal Reconnect.
    std::fs::write(directory.join("stop-gateway"), b"owned gateway only")
        .map_err(|e| e.to_string())?;
    until("gateway stopped", || {
        ui.gateway_stopped.get() && next.smoke_state()["refreshEnabled"] == false
    })
    .await?;
    if next.smoke_state()["events"]["codex"] != "Gateway stopped" {
        return Err("Diagnostics retained live status after gateway exit".into());
    }
    next.dialog.close();
    until("stopped dialog closed", || {
        ui.connections.borrow().is_none()
    })
    .await?;
    std::fs::remove_file(directory.join("stop-gateway")).map_err(|e| e.to_string())?;
    gtk::prelude::WidgetExt::activate_action(&ui.window, "win.reconnect", None)
        .map_err(|e| e.to_string())?;
    until("gateway restarted", || {
        let state = ui.state.borrow();
        !ui.gateway_stopped.get() && state.ready && state.startup_ready
    })
    .await?;
    let current = open(ui).await?;
    if current.smoke_state()["refreshEnabled"] != true {
        return Err("Reconnected gateway cannot refresh diagnostics".into());
    }
    current.dialog.close();
    until("final dialog closed", || ui.connections.borrow().is_none()).await?;
    let index = ui
        .state
        .borrow()
        .backends
        .iter()
        .position(|b| b.id == "codex")
        .unwrap();
    ui.backend.set_selected(index as u32);
    until("original session restored", || {
        let state = ui.state.borrow();
        state.ready
            && state.history_verified
            && !state.loading
            && state.backend == "codex"
            && state.selected.as_ref().is_some_and(|t| t.id == selected.id)
    })
    .await?;
    if ui.composer_text() != draft {
        return Err("Diagnostic/reconnect workflow lost the original chat draft".into());
    }
    Ok(
        json!({"ok":true,"registryAndMetadata":true,"literalMarkup":true,"noInferredCodexAvailability":true,"liveBackgroundConnections":true,"settingsEntry":true,"refreshAdmission":true,"partialFailureAndRecovery":true,"liveLanguage":true,"closedReplyDiscarded":true,"gatewayStopAndReconnect":true,"draftAndSessionPreserved":true}),
    )
}
