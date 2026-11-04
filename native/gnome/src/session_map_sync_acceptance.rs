//! Actual conversation Send/Queue controls with a private Codex app-server fixture.
use super::*;
async fn until(ui: &Ui, step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(25) {
            return Err(format!(
                "Map sync timed out: {step}; status={}; active={:?}; sending={}; loading={}",
                ui.session_map.smoke_status(),
                ui.state.borrow().active_turn,
                ui.state.borrow().sending,
                ui.state.borrow().loading
            ));
        }
        privacy(ui)?;
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    privacy(ui)
}
fn privacy(ui: &Ui) -> Result<(), String> {
    if ui.state.borrow().items.iter().any(|i| {
        i.role == "Assistant"
            && (i.body.contains("<codex-thread-studio-session-map")
                || i.body.contains("baseRevision"))
    }) {
        Err("Private Map metadata reached the GTK transcript".into())
    } else {
        Ok(())
    }
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
fn idle(ui: &Ui) -> bool {
    let s = ui.state.borrow();
    s.active_turn.is_none() && !s.sending && !s.loading
}
async fn send(ui: &Rc<Ui>, text: &str) -> Result<(), String> {
    until(ui, "send ready", || {
        let s = ui.state.borrow();
        idle(ui) && s.options_loaded && s.history_verified && s.ready
    })
    .await?;
    ui.composer.buffer().set_text(text);
    ui.submit();
    Ok(())
}
pub(super) async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    until(ui, "source loaded", || {
        let s = ui.state.borrow();
        s.ready
            && !s.loading
            && s.options_loaded
            && s.history_verified
            && s.selected.as_ref().is_some_and(|t| t.id == "target")
            && ui.router.borrow().registry.is_some()
    })
    .await?;
    let source = ui.state.borrow().selected.clone().unwrap();
    ui.open_session_map();
    until(ui, "initial Map", || {
        revision(ui) == 1 && ui.session_map.smoke_idle()
    })
    .await?;
    send(ui, "Native tool turn").await?;
    until(ui, "held tool turn", || root.join("tool-held").exists()).await?;
    let other = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "hidden")
        .cloned()
        .ok_or("Missing alternate conversation")?;
    ui.select(other);
    flag(root, "release-tool").await?;
    until(ui, "offscreen tool completion", || {
        root.join("tool-completed").exists() && !ui.state.borrow().loading
    })
    .await?;
    if ui.session_map.smoke_snapshot().is_some() {
        return Err("An offscreen tool update populated another conversation's Map".into());
    }
    ui.select(source.clone());
    until(ui, "tool update", || revision(ui) == 2 && idle(ui)).await?;
    if ui
        .session_map
        .smoke_snapshot()
        .unwrap()
        .map
        .current_item_id
        .as_deref()
        != Some("root")
    {
        return Err("Fallback overrode a successful tool update".into());
    }
    flag(root, "no-map-tools").await?;
    send(ui, "Native fallback turn").await?;
    until(ui, "fallback update", || revision(ui) == 3 && idle(ui)).await?;
    if ui
        .session_map
        .smoke_snapshot()
        .unwrap()
        .map
        .current_item_id
        .as_deref()
        != Some("child")
    {
        return Err("Fallback failed to set the current topic".into());
    }
    send(ui, "Native offscreen turn").await?;
    until(ui, "held offscreen turn", || {
        root.join("offscreen-held").exists()
    })
    .await?;
    ui.composer.buffer().set_text("Native queued turn");
    ui.enqueue()?;
    until(ui, "queue persisted", || {
        ui.state
            .borrow()
            .queues
            .entries
            .get("codex:target")
            .is_some_and(|e| !e.messages.is_empty())
    })
    .await?;
    let other = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "hidden")
        .cloned()
        .ok_or("Missing alternate conversation")?;
    ui.select(other);
    flag(root, "release-offscreen").await?;
    until(ui, "offscreen queued completion", || {
        root.join("queued-completed").exists() && !ui.state.borrow().loading
    })
    .await?;
    if ui.session_map.smoke_snapshot().is_some() {
        return Err("An offscreen Map update populated the selected conversation".into());
    }
    ui.select(source);
    until(ui, "source/Map restored", || {
        revision(ui) == 5 && idle(ui) && ui.session_map.smoke_idle()
    })
    .await?;
    send(ui, "Native stale turn").await?;
    until(ui, "held stale turn", || root.join("stale-held").exists()).await?;
    glib::timeout_future(Duration::from_millis(100)).await;
    ui.session_map.smoke_topic("Root topic", true)?;
    until(ui, "newer human revision", || {
        revision(ui) == 6 && ui.session_map.smoke_idle()
    })
    .await?;
    flag(root, "release-stale").await?;
    until(ui, "stale rejection", || {
        idle(ui) && ui.session_map.smoke_status().contains("conflict")
    })
    .await?;
    if revision(ui) != 6 || ui.session_map.smoke_snapshot().unwrap().done != 1 {
        return Err("A stale assistant update changed human completion".into());
    }
    until(ui, "conflict snapshot reloaded", || {
        ui.session_map.smoke_idle()
    })
    .await?;
    if !ui.session_map.smoke_writable() {
        return Err("Conflict recovery left explicit Map actions disabled".into());
    }
    send(ui, "Native empty turn").await?;
    until(ui, "empty update", || {
        idle(ui) && ui.session_map.smoke_status().contains("did not require")
    })
    .await?;
    if revision(ui) != 6 {
        return Err("An empty suffix changed the Map revision".into());
    }
    send(ui, "Native malformed turn").await?;
    until(ui, "malformed rejection", || {
        idle(ui) && ui.session_map.smoke_status().contains("not valid JSON")
    })
    .await?;
    if revision(ui) != 6 {
        return Err("Malformed private metadata changed the Map".into());
    }
    fn copy_button(widget: &gtk::Widget) -> Option<gtk::Button> {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            if button.tooltip_text().as_deref() == Some("Copy message Markdown")
                && button
                    .parent()
                    .and_then(|p| p.first_child())
                    .and_downcast::<gtk::Label>()
                    .is_some_and(|l| l.text() == "Assistant")
            {
                return Some(button.clone());
            }
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            if let Some(button) = copy_button(&c) {
                return Some(button);
            }
            child = c.next_sibling();
        }
        None
    }
    let copy =
        copy_button(ui.transcript_view.upcast_ref()).ok_or("No realized assistant Copy action")?;
    copy.emit_clicked();
    let copied = ui
        .window
        .clipboard()
        .read_text_future()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No owned clipboard text")?;
    if !copied.starts_with("Visible Native ")
        || copied.contains("<codex-thread-studio-session-map")
        || copied.contains("baseRevision")
    {
        return Err("Actual Copy action exposed private metadata or lost the answer".into());
    }
    smoke::capture(&ui.window, &root.join("map-sync.png"));
    Ok(
        json!({"ok":true,"sameTurnTool":true,"offscreenTool":true,"duplicateToolDelivery":true,"toolWinsOverFallback":true,"unsupportedToolsFallback":true,"splitMarkerPrivacy":true,"copyPrivacy":true,"offscreenUpdates":true,"queuedTurnUsesLatestRevision":true,"staleHumanEditProtected":true,"emptyNoop":true,"malformedNoop":true,"noRepairInference":true,"revision":6}),
    )
}
