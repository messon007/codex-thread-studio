//! Real Router Send controls, target selection and gateway-owned Map notifications.
use super::*;
async fn until(ui: &Ui, step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(30) {
            return Err(format!(
                "Routed Map UI timed out: {step}; status={}",
                ui.session_map.smoke_status()
            ));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn flag(root: &std::path::Path, name: &str) -> Result<(), String> {
    let path = root.join(name);
    gio::spawn_blocking(move || std::fs::write(path, b"1").map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Fixture writer stopped")?
}
fn revision(ui: &Ui) -> u64 {
    ui.session_map
        .smoke_snapshot()
        .map_or(0, |s| s.map.revision)
}
fn text_visible(widget: &gtk::Widget, needle: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|l| l.text().contains(needle))
    {
        return true;
    }
    if let Some(v) = widget.downcast_ref::<gtk::TextView>() {
        let b = v.buffer();
        if b.text(&b.start_iter(), &b.end_iter(), true)
            .contains(needle)
        {
            return true;
        }
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if text_visible(&c, needle) {
            return true;
        }
        child = c.next_sibling();
    }
    false
}
pub(super) async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    until(ui, "controller ready", || {
        let s = ui.state.borrow();
        s.ready
            && !s.loading
            && s.options_loaded
            && s.selected.as_ref().is_some_and(|t| t.id == "router")
            && ui.router.borrow().registry.is_some()
    })
    .await?;
    let controller = ui.state.borrow().selected.clone().unwrap();
    ui.composer.buffer().set_text("MAP_TOOL");
    ui.submit();
    until(ui, "held routed tool", || {
        root.join("gui-tool-held").exists()
    })
    .await?;
    let target = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "target")
        .cloned()
        .ok_or("Missing routed target")?;
    ui.select(target.clone());
    ui.open_session_map();
    until(ui, "captured target Map", || {
        revision(ui) == 1 && ui.session_map.smoke_idle()
    })
    .await?;
    flag(root, "gui-release-tool").await?;
    until(ui, "live tool Map notification", || {
        revision(ui) == 2 && ui.session_map.smoke_idle()
    })
    .await?;
    if ui
        .session_map
        .smoke_snapshot()
        .unwrap()
        .map
        .current_item_id
        .as_deref()
        != Some("root")
    {
        return Err("Routed tool was overwritten by fallback metadata".into());
    }
    smoke::capture(&ui.window, &root.join("routed-map-tool.png"));
    ui.select(controller.clone());
    until(ui, "routed record", || {
        ui.router
            .borrow()
            .jobs
            .values()
            .any(|j| j.job.status == "completed" && j.job.map_notice.contains("revision 2"))
    })
    .await?;
    until(ui, "native Map notice in routed chat", || {
        text_visible(
            ui.transcript_view.upcast_ref(),
            "Session Map updated to revision 2.",
        )
    })
    .await?;
    flag(root, "no-map-tools").await?;
    flag(root, "gui-hold-fallback").await?;
    until(ui, "controller options", || {
        !ui.state.borrow().loading && ui.state.borrow().options_loaded
    })
    .await?;
    ui.composer.buffer().set_text("MAP_FALLBACK");
    ui.submit();
    until(ui, "held fallback", || {
        root.join("gui-fallback-held").exists()
    })
    .await?;
    ui.select(target);
    ui.open_session_map();
    until(ui, "next captured Map", || {
        revision(ui) == 2 && ui.session_map.smoke_idle()
    })
    .await?;
    flag(root, "gui-release-fallback").await?;
    until(ui, "fallback Map notification", || {
        revision(ui) == 3 && ui.session_map.smoke_idle()
    })
    .await?;
    if ui.state.borrow().items.iter().any(|i| {
        i.body.contains("<codex-thread-studio-session-map-update")
            || i.body.contains("baseRevision")
    }) {
        return Err("Routed metadata reached native target rendering".into());
    }
    smoke::capture(&ui.window, &root.join("routed-map-fallback.png"));
    ui.select(controller);
    until(ui, "completed fallback record", || {
        ui.router
            .borrow()
            .jobs
            .values()
            .any(|j| j.job.status == "completed" && j.job.map_notice.contains("revision 3"))
    })
    .await?;
    if revision(ui) != 0 && revision(ui) != 1 {
        return Err("A target Map update overwrote the controller Map".into());
    }
    if ui.router.borrow().jobs.values().any(|j| {
        j.job.response.contains("baseRevision")
            || j.job
                .response_items
                .iter()
                .any(|i| i.text.contains("<codex-thread-studio-session-map-update"))
    }) {
        return Err("Private metadata leaked into a routed record".into());
    }
    smoke::capture(&ui.window, &root.join("routed-map-record.png"));
    Ok(
        json!({"ok":true,"actualRouterSend":true,"gatewayOwnsTools":true,"duplicateReplies":true,"liveTargetMap":true,"recordMapNotice":true,"fallback":true,"privateTargetAndRecord":true,"controllerMapIsolated":true,"revision":3}),
    )
}
