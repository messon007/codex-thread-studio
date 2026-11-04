//! Private provider history, actual native fields/buttons and shared-store saves.
use super::*;
use std::path::Path;
const ORIGINAL: &str = "Original opening question 中文😀";
const EDITED: &str = "Edited opening question 中文😀";
const ROLE: &str = "Own native session metadata and routing summaries 中文😀";
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(30) {
            return Err(format!("Session information timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
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
        if let Some(found) = find(&widget, test) {
            return Some(found);
        }
    }
    None
}
fn button(root: &gtk::Widget, name: &str) -> Result<gtk::Button, String> {
    find(root, &|b: &gtk::Button| {
        b.is_mapped()
            && (b.tooltip_text().as_deref() == Some(name) || b.label().as_deref() == Some(name))
    })
    .ok_or_else(|| format!("Missing session information button: {name}"))
}
fn views(panel: &crate::session_info_ui::Panel) -> Result<Vec<gtk::TextView>, String> {
    fn collect(root: &gtk::Widget, views: &mut Vec<gtk::TextView>) {
        if let Some(v) = root.downcast_ref::<gtk::TextView>() {
            views.push(v.clone());
        }
        let mut child = root.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            collect(&w, views);
        }
    }
    let mut result = Vec::new();
    collect(panel.dialog.upcast_ref(), &mut result);
    if result.len() != 2 {
        return Err(format!(
            "Expected two session information fields, got {}",
            result.len()
        ));
    }
    Ok(result)
}
async fn open(ui: &Rc<Ui>) -> Result<Rc<crate::session_info_ui::Panel>, String> {
    gtk::prelude::WidgetExt::activate_action(&ui.window, "win.session-info", None)
        .map_err(|e| e.to_string())?;
    let panel = ui
        .session_info
        .borrow()
        .clone()
        .ok_or("Session information did not open")?;
    until("loaded dialog", || {
        panel.dialog.is_mapped()
            && panel.smoke_state()["loaded"] == true
            && button(panel.dialog.upcast_ref(), "Copy opening question").is_ok()
    })
    .await?;
    Ok(panel)
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &Path,
    _capture: impl Fn(&adw::Dialog, &str),
) -> Result<Value, String> {
    until("session ready", || {
        let state = ui.state.borrow();
        state.history_verified && state.options_loaded && !state.loading && state.ready
    })
    .await?;
    if ui
        .state
        .borrow()
        .items
        .iter()
        .any(|i| i.body.contains(ORIGINAL))
    {
        return Err("Fixture opening question was already in the live tail".into());
    }
    let draft = "Unsent metadata draft 中文😀 !not a command";
    ui.composer.buffer().set_text(draft);
    let original_thread = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("Missing selected session")?;
    let panel = open(ui).await?;
    if panel.smoke_state()["question"] != ORIGINAL {
        return Err(format!("Wrong history opening: {}", panel.smoke_state()));
    }
    let root = panel.dialog.upcast_ref::<gtk::Widget>();
    if find(root, &|l: &gtk::Label| l.text() == "native-fixture-cli").is_none() {
        return Err("CLI metadata was not displayed literally".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &panel.dialog,
        &directory.join("session-info-history.png"),
    )
    .await?;
    let fields = views(&panel)?;
    fields[0].buffer().set_text(EDITED);
    fields[1].buffer().set_text("Manual responsibility 中文😀");
    button(root, "Copy opening question")?.emit_clicked();
    let text = root
        .clipboard()
        .read_text_future()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Opening question copy was empty")?;
    if text != EDITED {
        return Err("Copy did not use the current opening field".into());
    }
    button(root, "Save")?.emit_clicked();
    until("saved dialog closed", || ui.session_info.borrow().is_none()).await?;
    let panel = open(ui).await?;
    if panel.smoke_state()["question"] != EDITED
        || panel.smoke_state()["role"] != "Manual responsibility 中文😀"
    {
        return Err("Session information did not survive reopening".into());
    }
    until("generation ready", || {
        panel.smoke_state()["generateEnabled"] == true
    })
    .await?;
    button(panel.dialog.upcast_ref(), "Generate with AI")?.emit_clicked();
    until("generated role", || {
        panel.smoke_state()["role"] == ROLE && panel.smoke_state()["generating"] == false
    })
    .await?;
    if ui.composer_text() != draft {
        return Err("Generation consumed the composer draft".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &panel.dialog,
        &directory.join("session-info-generated.png"),
    )
    .await?;
    // A second editor reads the shared row, proving generation alone did not save.
    let peer_id = {
        let mut state = ui.state.borrow_mut();
        state.sequence += 1;
        state.sequence
    };
    let peer = crate::session_info_ui::Panel::new(
        &ui.window,
        ui.sender.clone(),
        panel.input.clone(),
        peer_id,
    );
    ui.session_info.replace(Some(peer.clone()));
    peer.dialog.present(Some(&ui.window));
    peer.begin();
    until("second client read", || {
        peer.dialog.is_mapped()
            && peer.smoke_state()["loaded"] == true
            && button(peer.dialog.upcast_ref(), "Copy opening question").is_ok()
    })
    .await?;
    if peer.smoke_state()["role"] != "Manual responsibility 中文😀" {
        return Err("Generated responsibility was saved without explicit Save".into());
    }
    views(&peer)?[0]
        .buffer()
        .set_text("Peer opening question 中文😀");
    button(peer.dialog.upcast_ref(), "Save")?.emit_clicked();
    until("second editor saved", || {
        !peer.dialog.is_mapped() && peer.smoke_state()["writing"] == false
    })
    .await?;
    ui.session_info.replace(Some(panel.clone()));
    panel.dialog.present(Some(&ui.window));
    until("original editor restored", || {
        button(panel.dialog.upcast_ref(), "Save").is_ok_and(|b| b.is_sensitive())
    })
    .await?;
    button(panel.dialog.upcast_ref(), "Save")?.emit_clicked();
    until("conflict retained", || {
        panel.smoke_state()["reloadRequired"] == true && panel.smoke_state()["writing"] == false
    })
    .await?;
    if panel.smoke_state()["question"] != EDITED
        || panel.smoke_state()["role"] != ROLE
        || panel.smoke_state()["dirty"] != true
    {
        return Err("A conflicting save discarded edited fields".into());
    }
    button(panel.dialog.upcast_ref(), "Reload session information")?.emit_clicked();
    until("reload close guard", || {
        ui.window
            .visible_dialog()
            .is_some_and(|d| d.is::<adw::AlertDialog>() && button(d.upcast_ref(), "Cancel").is_ok())
    })
    .await?;
    let alert = ui
        .window
        .visible_dialog()
        .unwrap()
        .downcast::<adw::AlertDialog>()
        .unwrap();
    button(alert.upcast_ref(), "Cancel")?.emit_clicked();
    until("cancel reload", || {
        ui.window
            .visible_dialog()
            .is_some_and(|d| d == panel.dialog)
    })
    .await?;
    if panel.smoke_state()["role"] != ROLE {
        return Err("Cancelling Reload discarded the generated result".into());
    }
    button(panel.dialog.upcast_ref(), "Reload session information")?.emit_clicked();
    until("discard choice", || {
        ui.window
            .visible_dialog()
            .is_some_and(|d| d.is::<adw::AlertDialog>() && button(d.upcast_ref(), "Cancel").is_ok())
    })
    .await?;
    button(ui.window.visible_dialog().unwrap().upcast_ref(), "Discard")?.emit_clicked();
    until("peer snapshot reloaded", || {
        panel.smoke_state()["question"] == "Peer opening question 中文😀"
            && panel.smoke_state()["role"] == "Manual responsibility 中文😀"
            && panel.smoke_state()["reloadRequired"] == false
    })
    .await?;
    let fields = views(&panel)?;
    fields[0].buffer().set_text(EDITED);
    fields[1].buffer().set_text(ROLE);
    button(panel.dialog.upcast_ref(), "Save")?.emit_clicked();
    until("explicit final save", || ui.session_info.borrow().is_none()).await?;
    let panel = open(ui).await?;
    if panel.smoke_state()["question"] != EDITED || panel.smoke_state()["role"] != ROLE {
        return Err("Generated responsibility did not persist on explicit Save".into());
    }
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    crate::window_capture::capture_dialog(
        &ui.window,
        &panel.dialog,
        &directory.join("session-info-chinese.png"),
    )
    .await?;
    if panel.dialog.title() != "会话信息"
        || panel.smoke_state()["question"] != EDITED
        || panel.smoke_state()["role"] != ROLE
        || find(panel.dialog.upcast_ref(), &|l: &gtk::Label| {
            l.text() == "CLI 版本"
        })
        .is_none()
    {
        return Err("Session information locale changed model values".into());
    }
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    // Freeze a dirty editor, switch sessions and prove it cannot save to either.
    views(&panel)?[0]
        .buffer()
        .set_text("Keep stale session question 中文😀");
    let other = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| {
            t.id != original_thread.id
                && t.parent.is_none()
                && t.id != "router"
                && t.id != "oc-router"
        })
        .cloned()
        .ok_or("Session information fixture lacks another ordinary session")?;
    {
        ui.select(other);
        until("retired session editor", || {
            panel.smoke_state()["valid"] == false
        })
        .await?;
        if panel.smoke_state()["saveEnabled"] == true
            || panel.smoke_state()["question"] != "Keep stale session question 中文😀"
        {
            return Err("A stale editor changed its draft or remained writable".into());
        }
    }
    // Discard only the private editor draft; persisted metadata stays intact.
    panel.dialog.force_close();
    until("closed stale editor", || ui.session_info.borrow().is_none()).await?;
    ui.select(original_thread);
    until("original restored", || {
        ui.state.borrow().history_verified && !ui.state.borrow().loading
    })
    .await?;
    if ui.composer_text() != draft {
        return Err("Session information changed the cached composer draft".into());
    }
    // A second normal generation stays active until the actual Stop control is used.
    // The fixture stores interruption in provider history; no UI replies are injected.
    until("restored session idle", || {
        let state = ui.state.borrow();
        state.ready && state.options_loaded && state.active_turn.is_none() && !state.sending
    })
    .await?;
    let opencode = ui
        .state
        .borrow()
        .backends
        .iter()
        .any(|b| b.id == ui.state.borrow().backend && b.kind == "opencode");
    std::fs::write(
        directory.join(if opencode {
            "hold-next-oc-target"
        } else {
            "hold-target"
        }),
        b"held private generation",
    )
    .map_err(|e| e.to_string())?;
    let panel = open(ui).await?;
    until("stop generation ready", || {
        panel.smoke_state()["generateEnabled"] == true
    })
    .await?;
    button(panel.dialog.upcast_ref(), "Generate with AI")?.emit_clicked();
    until("generation can stop", || {
        button(panel.dialog.upcast_ref(), "Stop generation").is_ok_and(|b| b.is_sensitive())
    })
    .await?;
    button(panel.dialog.upcast_ref(), "Stop generation")?.emit_clicked();
    until("generation stopped", || {
        panel.smoke_state()["generating"] == false
    })
    .await?;
    if panel.smoke_state()["role"] != ROLE
        || panel.smoke_state()["dirty"] != false
        || ui.composer_text() != draft
    {
        return Err("Stopping generation changed saved information or the composer draft".into());
    }
    panel.dialog.close();
    until("stopped dialog closed", || {
        ui.session_info.borrow().is_none()
    })
    .await?;
    Ok(
        json!({"ok":true,"openingOutsideTail":true,"copyEditedQuestion":true,"manualSaveReopen":true,"normalSessionGeneration":true,"generatedRequiresExplicitSave":true,"conflictPreservesDraft":true,"reloadCancelAndDiscard":true,"liveLanguage":true,"staleEditorBlocked":true,"composerDraftRetained":true,"generationStopPreservesSavedRole":true}),
    )
}
