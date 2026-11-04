//! Real GTK controls and isolated fake-provider sessions, with a persisted-store restart.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let started = std::time::Instant::now();
    while !test() {
        if started.elapsed() > Duration::from_secs(25) {
            return Err(format!("Session Map acceptance timed out: {step}"));
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
pub(super) async fn run(ui: &Rc<Ui>, root: &std::path::Path, phase: &str) -> Result<Value, String> {
    until("source history", || {
        let s = ui.state.borrow();
        s.ready && !s.loading && s.selected.as_ref().is_some_and(|v| v.id == "target")
    })
    .await?;
    let source = ui.state.borrow().selected.clone().unwrap();
    let p = &ui.session_map;
    ui.composer.buffer().set_text("Unsent Map draft 中文😀");
    ui.open_session_map();
    until("Map load", || p.smoke_idle()).await?;
    if phase == "restart" {
        let s = p.smoke_snapshot().ok_or("Map did not survive restart")?;
        if s.rows.len() != 2 || s.map.goal != "Inferred goal 中文😀" || s.map.revision < 6 {
            return Err("Restored native Map changed".into());
        }
        p.smoke_delete();
        let dialog = ui
            .window
            .visible_dialog()
            .and_downcast::<adw::AlertDialog>()
            .ok_or("Missing native Delete Map confirmation")?;
        fn find(widget: &gtk::Widget) -> Option<gtk::Button> {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if button.label().as_deref() == Some("Delete Map") {
                    return Some(button.clone());
                }
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                if let Some(button) = find(&c) {
                    return Some(button);
                }
                child = c.next_sibling();
            }
            None
        }
        find(dialog.upcast_ref())
            .ok_or("Missing Delete Map response button")?
            .emit_clicked();
        until("delete", || p.smoke_idle() && p.smoke_snapshot().is_none()).await?;
        return Ok(
            json!({"ok":true,"restart":true,"deleteConfirmation":true,"sharedStorePersistence":true}),
        );
    }
    if p.smoke_snapshot().is_some() {
        return Err("Private source unexpectedly had a Map".into());
    }
    if !ui.state.borrow().items.iter().any(|v| v.role == "You") {
        return Err("Provider history was not loaded for goal inference".into());
    }
    p.smoke_create("")?;
    until("AI-created outline", || {
        p.smoke_idle() && p.smoke_snapshot().is_some()
    })
    .await?;
    let s = p.smoke_snapshot().unwrap();
    if s.map.goal != "Inferred goal 中文😀" || s.rows.len() != 2 || s.map.revision != 2 {
        return Err(format!("Unexpected generated Map: {}", p.smoke_status()));
    }
    until("GTK topic layout", || p.widget.is_mapped()).await?;
    glib::timeout_future(Duration::from_millis(150)).await;
    p.smoke_topic("Root topic 中文😀", false)?;
    until("set current", || p.smoke_idle()).await?;
    if p.smoke_snapshot().unwrap().map.current_item_id.as_deref() != Some("root") {
        return Err("Set current did not update the shared Map".into());
    }
    p.smoke_topic("Root topic 中文😀", true)?;
    until("human Done", || p.smoke_idle()).await?;
    if p.smoke_snapshot().unwrap().done != 1 {
        return Err("Human completion was not stored".into());
    }
    p.smoke_undo();
    until("undo", || p.smoke_idle()).await?;
    if p.smoke_snapshot().unwrap().done != 0 {
        return Err("Undo did not restore completion".into());
    }
    p.smoke_generate();
    until("AI update", || p.smoke_idle()).await?;
    let revision = p.smoke_snapshot().unwrap().map.revision;
    if revision != 6 || p.smoke_snapshot().unwrap().map.current_item_id.as_deref() != Some("child")
    {
        return Err(format!(
            "AI update did not apply safe operations: {}",
            p.smoke_status()
        ));
    }
    smoke::capture(&ui.window, &root.join("session-map.png"));
    flag(root, "hold-map").await?;
    p.smoke_generate();
    until("held utility", || root.join("map-held").exists()).await?;
    p.smoke_cancel();
    flag(root, "release-map").await?;
    until("cancelled task", || p.smoke_idle()).await?;
    if p.smoke_snapshot().unwrap().map.revision != revision {
        return Err("Cancelled generation changed the Map".into());
    }
    let hold = root.join("release-map");
    gio::spawn_blocking(move || std::fs::remove_file(hold))
        .await
        .map_err(|_| "Fixture cleanup stopped")?
        .map_err(|e| e.to_string())?;
    let held = root.join("map-held");
    gio::spawn_blocking(move || std::fs::remove_file(held))
        .await
        .map_err(|_| "Fixture cleanup stopped")?
        .map_err(|e| e.to_string())?;
    p.smoke_generate();
    until("second held utility", || root.join("map-held").exists()).await?;
    let other = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "hidden")
        .cloned()
        .ok_or("Missing alternate session")?;
    ui.select(other);
    flag(root, "release-map").await?;
    until("alternate selection", || {
        !ui.state.borrow().loading && p.smoke_idle()
    })
    .await?;
    if p.smoke_snapshot().is_some() {
        return Err("Late utility populated another conversation's Map".into());
    }
    ui.select(source);
    until("source restored", || {
        !ui.state.borrow().loading && p.smoke_idle() && p.smoke_snapshot().is_some()
    })
    .await?;
    if p.smoke_snapshot().unwrap().map.revision != revision
        || ui.composer_text() != "Unsent Map draft 中文😀"
    {
        return Err("Session switch changed the Map or source draft".into());
    }
    Ok(
        json!({"ok":true,"optionalGoalInference":true,"safeOutline":true,"currentTopic":true,"humanDone":true,"undo":true,"aiUpdate":true,"cancel":true,"sessionSwitchIsolation":true,"sourceDraftRetained":true,"revision":revision}),
    )
}
