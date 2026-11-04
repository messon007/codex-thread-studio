//! Real menu/Send/Queue controls with the isolated provider fixture.
use super::*;
use crate::{acceptance_wait::until, turn_supervision::Stage};
async fn flag(root: &std::path::Path, name: &str) -> Result<(), String> {
    let path = root.join(name);
    gio::spawn_blocking(move || std::fs::write(path, b"1").map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Fixture writer stopped")?
}
fn stage(ui: &Ui) -> Option<Stage> {
    ui.supervision
        .borrow()
        .entries
        .get("codex:target")
        .and_then(|e| e.view.as_ref().map(|v| v.stage))
}
fn idle(ui: &Ui) -> bool {
    let s = ui.state.borrow();
    s.ready
        && s.history_verified
        && s.options_loaded
        && !s.loading
        && !s.sending
        && s.active_turn.is_none()
}
fn stale_popup(ui: &Ui) -> bool {
    fn contains(w: &gtk::Widget) -> bool {
        if let Some(label) = w.downcast_ref::<gtk::Label>() {
            if label
                .text()
                .contains(&crate::language::tr("Supervision complete"))
            {
                return true;
            }
        }
        let mut child = w.first_child();
        while let Some(w) = child {
            if contains(&w) {
                return true;
            }
            child = w.next_sibling();
        }
        false
    }
    ui.supervision_menu
        .button
        .popover()
        .is_some_and(|p| contains(p.upcast_ref()))
}
fn press(ui: &Ui, label: &str) -> Result<(), String> {
    fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
        if let Some(button) = widget
            .downcast_ref::<gtk::Button>()
            .filter(|b| b.label().as_deref() == Some(label) && b.is_sensitive())
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            if let Some(button) = find(&widget, label) {
                return Some(button);
            }
            child = widget.next_sibling();
        }
        None
    }
    ui.supervision_menu.button.popup();
    let popup = ui
        .supervision_menu
        .button
        .popover()
        .ok_or("Missing supervision menu")?;
    let button = find(popup.upcast_ref(), &crate::language::tr(label))
        .ok_or_else(|| format!("Missing supervision action: {label}"))?;
    button.emit_clicked();
    Ok(())
}
async fn send(ui: &Rc<Ui>, text: &str) -> Result<(), String> {
    until("source ready", || idle(ui)).await?;
    ui.composer.buffer().set_text(text);
    ui.send.emit_clicked();
    until("source accepted", || {
        ui.state.borrow().active_turn.is_some() && !ui.state.borrow().sending
    })
    .await
}
async fn enqueue(ui: &Rc<Ui>, text: &str, key: &str) -> Result<(), String> {
    ui.composer.buffer().set_text(text);
    ui.queue_button.emit_clicked();
    until("queued follow-up saved", || {
        ui.state
            .borrow()
            .queues
            .entries
            .get(key)
            .is_some_and(|e| e.messages.len() == 1 && e.saving.is_none())
    })
    .await
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    root: &std::path::Path,
    mut capture: impl FnMut(&str) -> Result<(), String>,
) -> Result<Value, String> {
    until("native source startup", || {
        idle(ui)
            && ui.router.borrow().registry.is_some()
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| t.id == "target")
    })
    .await?;
    send(ui, "Supervision original A and B").await?;
    press(ui, "Supervise this turn")?;
    if !ui.supervision_holds("codex:target") {
        return Err("Supervision did not acquire its optimistic queue hold".into());
    }
    until("source waiting", || stage(ui) == Some(Stage::Waiting)).await?;
    if !ui.fast.is_sensitive() {
        return Err("Fixture source lacks Fast control".into());
    }
    ui.fast.emit_clicked();
    until("source Fast option changed", || {
        ui.state.borrow().options.service_tier == "fast"
    })
    .await?;
    enqueue(ui, "Queued follow-up after supervision", "codex:target").await?;
    ui.composer
        .buffer()
        .set_text("Unsent supervision draft 中文😀 ");
    flag(root, "release-source-1").await?;
    until("authoritative evaluation held", || {
        stage(ui) == Some(Stage::Evaluating) && root.join("evaluation-held-1").exists()
    })
    .await?;
    if !ui.supervision_holds("codex:target") || root.join("source-held-2").exists() {
        return Err("Queue or continuation escaped while evaluation was held".into());
    }
    ui.supervision_menu.button.popup();
    glib::timeout_future(Duration::from_millis(100)).await;
    if !ui.supervision_status.text().contains("Evaluating")
        || !ui.supervision_menu.button.has_css_class("suggested-action")
    {
        return Err("Active supervision is missing its visible native status".into());
    }
    let popup = ui
        .supervision_menu
        .button
        .popover()
        .ok_or("Missing live supervision popup")?;
    crate::window_capture::capture_widget(&ui.window, &popup, &root.join("supervision-menu.png"))
        .await?;
    capture("supervision-evaluating.png")?;
    flag(root, "release-evaluation-1").await?;
    until("automatic continuation accepted", || {
        root.join("source-held-2").exists()
            && ui
                .supervision
                .borrow()
                .entries
                .get("codex:target")
                .and_then(|e| e.view.as_ref())
                .is_some_and(|v| v.current == "target-turn-2" && v.count == 1)
    })
    .await?;
    if ui.state.borrow().queues.entries["codex:target"]
        .messages
        .len()
        != 1
        || ui.composer_text() != "Unsent supervision draft 中文😀 "
    {
        return Err("Supervision changed the draft or consumed a queued follow-up early".into());
    }
    flag(root, "release-source-2").await?;
    until("supervision complete then queued follow-up", || {
        stage(ui) == Some(Stage::Complete)
            && idle(ui)
            && ui
                .state
                .borrow()
                .queues
                .entries
                .get("codex:target")
                .is_some_and(|e| e.messages.is_empty() && !e.busy() && e.flight.is_none())
    })
    .await?;
    if ui.supervision_holds("codex:target") {
        return Err("Completed supervision retained the source queue hold".into());
    }
    send(ui, "Stop supervised source").await?;
    if stale_popup(ui) {
        return Err(
            "A previous terminal supervisor status leaked onto the new running turn".into(),
        );
    }
    press(ui, "Supervise this turn")?;
    until("second source waiting", || {
        stage(ui) == Some(Stage::Waiting)
    })
    .await?;
    press(ui, "Stop supervision")?;
    until("explicit supervision stop settled", || {
        stage(ui) == Some(Stage::Stopped) && !ui.supervision_holds("codex:target")
    })
    .await?;
    if !ui.state.borrow().queues.entries["codex:target"].paused {
        return Err("Stopping supervision did not pause the source queue".into());
    }
    flag(root, "release-source-4").await?;
    until("manually stopped source completes", || idle(ui)).await?;
    glib::timeout_future(Duration::from_millis(300)).await;
    if root.join("evaluation-held-3").exists() {
        return Err("Stopped supervision restarted automatically".into());
    }
    let router = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "router")
        .cloned()
        .ok_or("Missing Router fixture session")?;
    ui.select(router);
    until("Router controls ready", || {
        ui.router_selected() && ui.state.borrow().options_loaded && !ui.state.borrow().loading
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Routed supervision task A and B");
    ui.send.emit_clicked();
    until("running routed source menu", || {
        root.join("source-held-5").exists()
            && ui
                .router
                .borrow()
                .jobs
                .values()
                .any(|d| d.job.controller == "codex:router" && d.job.status == "running")
    })
    .await?;
    press(ui, "Supervise this turn")?;
    if !ui.supervision_holds("codex:router") || !ui.supervision_holds("codex:target") {
        return Err("Routed supervision did not hold both queues".into());
    }
    until("routed source waiting", || {
        stage(ui) == Some(Stage::Waiting)
    })
    .await?;
    enqueue(ui, "Queued Router follow-up", "codex:router").await?;
    flag(root, "release-source-5").await?;
    until("routed evaluation held", || {
        stage(ui) == Some(Stage::Evaluating) && root.join("evaluation-held-3").exists()
    })
    .await?;
    if ui.state.borrow().queues.entries["codex:router"]
        .messages
        .len()
        != 1
        || !ui.supervision_holds("codex:router")
    {
        return Err(
            "Routed completion released the controller queue before supervision settled".into(),
        );
    }
    flag(root, "release-evaluation-3").await?;
    until("routed continuation with durable source", || {
        root.join("source-held-6").exists()
            && ui
                .supervision
                .borrow()
                .entries
                .get("codex:target")
                .and_then(|e| e.view.as_ref())
                .is_some_and(|v| v.current == "target-turn-6")
    })
    .await?;
    flag(root, "release-source-6").await?;
    // The next Router message is an ordinary queued task, never a supervisor replay.
    flag(root, "release-source-7").await?;
    until("Router follow-up after supervision", || {
        stage(ui) == Some(Stage::Complete)
            && ui
                .state
                .borrow()
                .queues
                .entries
                .get("codex:router")
                .is_some_and(|e| {
                    e.messages.is_empty()
                        && !e.busy()
                        && e.flight.is_none()
                        && e.router_wait.is_none()
                })
    })
    .await?;
    if ui.supervision_holds("codex:router") {
        return Err("Completed routed supervision retained the controller queue".into());
    }
    capture("supervision-router.png")?;
    let target = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "target")
        .cloned()
        .ok_or("Missing source after Router")?;
    ui.select(target);
    send(ui, "Cancel held evaluation source").await?;
    press(ui, "Supervise this turn")?;
    until("cancellable source waiting", || {
        stage(ui) == Some(Stage::Waiting)
    })
    .await?;
    enqueue(ui, "Queued follow-up remains paused", "codex:target").await?;
    flag(root, "release-source-8").await?;
    until("cancellable evaluation held", || {
        stage(ui) == Some(Stage::Evaluating) && root.join("evaluation-held-5").exists()
    })
    .await?;
    ui.supervision_menu.button.popup();
    glib::timeout_future(Duration::from_millis(100)).await;
    capture("supervision-cancel-evaluation.png")?;
    press(ui, "Stop supervision")?;
    until("owned evaluator cancellation and cleanup", || {
        stage(ui) == Some(Stage::Stopped)
            && !ui.supervision_holds("codex:target")
            && root.join("utility-cleaned-5").exists()
    })
    .await?;
    glib::timeout_future(Duration::from_millis(300)).await;
    let state = ui.state.borrow();
    let queue = &state.queues.entries["codex:target"];
    if !queue.paused || queue.messages.len() != 1 || queue.flight.is_some() {
        return Err("Stopping an evaluator released or consumed the queued follow-up".into());
    }
    drop(state);
    Ok(
        json!({"ok":true,"nativeMenuOptIn":true,"runningSource":true,"actualUtilityEvaluation":true,"authoritativeUtilityOutput":true,"exactContinuation":true,"queueHeldDuringEvaluation":true,"queueReleasedAfterComplete":true,"stopPausesQueue":true,"noResumeAfterStop":true,"routedSource":true,"controllerQueueHeld":true,"durableRouterHandoff":true,"currentSourceFast":true,"draftPreserved":true,"utilityCancellation":true,"queuePausedAfterEvaluationStop":true}),
    )
}
pub(super) async fn restore(
    ui: &Rc<Ui>,
    root: &std::path::Path,
    mut capture: impl FnMut(&str) -> Result<(), String>,
) -> Result<Value, String> {
    until("restored target source loaded", || {
        ui.state
            .borrow()
            .selected
            .as_ref()
            .is_some_and(|t| t.id == "target")
            && ui.state.borrow().history_verified
            && ui.state.borrow().active_turn.as_deref() == Some("target-turn-1")
            && ui.state.borrow().options_loaded
            && ui.router.borrow().registry.is_some()
    })
    .await?;
    if !ui.router.borrow().jobs.is_empty() {
        return Err("Restart fixture loaded full Router replies before target-side opt-in".into());
    }
    press(ui, "Supervise this turn")?;
    if !ui.supervision_holds("codex:router") {
        return Err("Ownership discovery did not provisionally hold Router follow-ups".into());
    }
    until("cold target resolves controller", || {
        stage(ui) == Some(Stage::Waiting)
            && ui
                .supervision
                .borrow()
                .entries
                .get("codex:target")
                .is_some_and(|e| e.source.controller.as_deref() == Some("codex:router"))
    })
    .await?;
    if !ui.router.borrow().jobs.is_empty() {
        return Err("Ownership discovery loaded full Router replies".into());
    }
    let router = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "router")
        .cloned()
        .ok_or("Restored Router missing")?;
    ui.select(router);
    until("restored Router ready", || {
        ui.router_selected() && ui.state.borrow().options_loaded && !ui.state.borrow().loading
    })
    .await?;
    enqueue(ui, "Queued Router follow-up", "codex:router").await?;
    flag(root, "release-source-1").await?;
    until("restored evaluation holds controller", || {
        stage(ui) == Some(Stage::Evaluating) && root.join("evaluation-held-1").exists()
    })
    .await?;
    if !ui.supervision_holds("codex:router")
        || ui.state.borrow().queues.entries["codex:router"]
            .messages
            .len()
            != 1
    {
        return Err("Restored controller queue escaped supervision".into());
    }
    capture("supervision-restored-evaluating.png")?;
    flag(root, "release-evaluation-1").await?;
    until("restored continuation accepted", || {
        root.join("source-held-2").exists()
    })
    .await?;
    flag(root, "release-source-2").await?;
    flag(root, "release-source-3").await?;
    until(
        "restored supervision and ordinary Router follow-up settle",
        || {
            stage(ui) == Some(Stage::Complete)
                && ui
                    .state
                    .borrow()
                    .queues
                    .entries
                    .get("codex:router")
                    .is_some_and(|e| {
                        e.messages.is_empty()
                            && e.flight.is_none()
                            && e.router_wait.is_none()
                            && !e.busy()
                    })
        },
    )
    .await?;
    capture("supervision-restored-complete.png")?;
    Ok(
        json!({"ok":true,"targetSideAfterRestart":true,"noHiddenReplyLoad":true,"provisionalControllerGate":true,"resolvedControllerGate":true,"queueHeldDuringEvaluation":true,"queueReleasedAfterComplete":true,"exactContinuation":true,"durableRouterHandoff":true}),
    )
}
