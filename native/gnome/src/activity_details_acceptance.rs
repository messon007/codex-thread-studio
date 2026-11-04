use super::*;
use std::{path::Path, time::Instant};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(30) {
            return Err(format!("Activity snapshot timed out: {step}"));
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
fn displayed(probe: &crate::conversation_output_ui::ActivityProbe) -> String {
    probe
        .buffer
        .text(&probe.buffer.start_iter(), &probe.buffer.end_iter(), true)
        .to_string()
}
async fn inspect(ui: &Rc<Ui>, id: &str) -> Result<(), String> {
    let position = || {
        (0..ui.transcript.n_items()).find(|p| {
            ui.transcript
                .item(*p)
                .and_downcast::<glib::BoxedAnyObject>()
                .is_some_and(|object| {
                    let row = object.borrow::<crate::chat_ui::Row>();
                    row.part == crate::chat_ui::Part::Header && row.item.id == id
                })
        })
    };
    // History transport settles before the coalesced GTK reconciliation runs.
    // Wait for the actual row rather than assuming the same event-loop tick.
    until(&format!("activity header {id}"), || position().is_some()).await?;
    let position = position().ok_or_else(|| format!("Missing activity header {id}"))?;
    ui.transcript_view
        .scroll_to(position, gtk::ListScrollFlags::NONE, None);
    let name = format!("activity:{id}");
    until("inspect button mapped", || {
        find(ui.transcript_view.upcast_ref(), &|b: &gtk::Button| {
            b.widget_name() == name && b.is_mapped()
        })
        .is_some()
    })
    .await?;
    find(ui.transcript_view.upcast_ref(), &|b: &gtk::Button| {
        b.widget_name() == name && b.is_mapped()
    })
    .unwrap()
    .emit_clicked();
    until("activity page", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.dialog.is_mapped() && p.copy.is_mapped())
    })
    .await
}
async fn close(ui: &Rc<Ui>) -> Result<(), String> {
    ui.output
        .activity_probe()
        .ok_or("Missing snapshot viewer")?
        .dialog
        .close();
    until("closed snapshot", || ui.output.activity_probe().is_none()).await
}
fn emit(directory: &Path, value: Value) -> Result<(), String> {
    std::fs::write(
        directory.join("activity-events.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .map_err(|e| e.to_string())
}
pub(super) async fn run(ui: &Rc<Ui>, directory: &Path) -> Result<Value, String> {
    until("history ready", || {
        let s = ui.state.borrow();
        s.ready && s.history_verified && !s.loading
    })
    .await?;
    if ui.state.borrow().backend == "opencode" {
        return opencode(ui, directory).await;
    }
    let raw: Vec<Value> = serde_json::from_slice(
        &std::fs::read(directory.join("activity-items.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let draft = "Unsent activity-inspection draft 中文😀 !literal";
    ui.composer.buffer().set_text(draft);
    let selected = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("Missing selected session")?;
    let before = ui.state.borrow().active_turn.clone();
    if before.is_some() {
        return Err("Completed fixture history unexpectedly has an active turn".into());
    }
    let small = raw.iter().find(|v| v["id"] == "tool-small").unwrap();
    inspect(ui, "tool-small").await?;
    let snapshot = ui.output.activity_probe().unwrap();
    let text = displayed(&snapshot);
    if serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())? != *small {
        return Err("Tool input/error/status/future fields were lost in the viewer".into());
    }
    snapshot.copy.emit_clicked();
    let copied = snapshot
        .dialog
        .clipboard()
        .read_text_future()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Empty snapshot clipboard")?;
    if copied != text {
        return Err("Copy did not use the exact activity page".into());
    }
    if find(snapshot.dialog.upcast_ref(), &|v: &sourceview::View| {
        v.is_editable()
    })
    .is_some()
    {
        return Err("Activity viewer is editable".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &snapshot.dialog,
        &directory.join("activity-tool.png"),
    )
    .await?;
    let mut updated = small.clone();
    updated["arguments"]["query"] = "Updated metadata without changing output 中文😀".into();
    let previous = ui
        .state
        .borrow()
        .items
        .iter()
        .find(|i| i.id == "tool-small")
        .cloned()
        .unwrap();
    emit(
        directory,
        json!([{"method":"item/completed","params":{"threadId":selected.id,"turnId":"activity-turn","item":updated}}]),
    )?;
    until("metadata-only provider update", || {
        ui.state
            .borrow()
            .items
            .iter()
            .find(|i| i.id == "tool-small")
            .is_some_and(|i| i.details != previous.details)
    })
    .await?;
    if displayed(&snapshot) != text {
        return Err("A provider update rewrote an immutable open snapshot".into());
    }
    close(ui).await?;
    inspect(ui, "tool-small").await?;
    if serde_json::from_str::<Value>(&displayed(&ui.output.activity_probe().unwrap()))
        .map_err(|e| e.to_string())?
        != updated
    {
        return Err("Rebound activity header opened an old metadata snapshot".into());
    }
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    let snapshot = ui.output.activity_probe().unwrap();
    if snapshot.dialog.title() != "活动详情"
        || !displayed(&snapshot).contains("Updated metadata")
        || snapshot.copy.tooltip_text().as_deref() != Some("复制当前页")
        || snapshot.search.placeholder_text().as_deref() != Some("在完整输出中查找原文")
    {
        return Err(
            "Locale switching changed provider data or failed to update the inspector".into(),
        );
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &snapshot.dialog,
        &directory.join("activity-chinese.png"),
    )
    .await?;
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    close(ui).await?;
    for id in [
        "reasoning-content",
        "array-command",
        "changed-files",
        "web-search",
        "future-tool",
        "progress-commentary",
    ] {
        inspect(ui, id).await?;
        let snapshot = ui.output.activity_probe().unwrap();
        let actual: Value =
            serde_json::from_str(&displayed(&snapshot)).map_err(|e| e.to_string())?;
        let expected = raw.iter().find(|v| v["id"] == id).unwrap();
        if id == "reasoning-content" {
            if actual["content"] != expected["content"] || actual.get("encryptedContent").is_some()
            {
                return Err("Visible reasoning fallback or envelope isolation failed".into());
            }
        } else if actual != *expected {
            return Err(format!("Activity details changed for {id}"));
        }
        close(ui).await?;
    }
    inspect(ui, "large-tool").await?;
    let snapshot = ui.output.activity_probe().unwrap();
    if snapshot.pages.unwrap_or(0) < 2
        || displayed(&snapshot).len() > crate::conversation_output::PAGE_BYTES + 64
    {
        return Err("Large activity snapshot was not paged".into());
    }
    snapshot.next.emit_clicked();
    until("next activity page", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.page == Some(1))
    })
    .await?;
    snapshot.last.emit_clicked();
    until("last activity page", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.page == p.pages.map(|n| n - 1))
    })
    .await?;
    snapshot.first.emit_clicked();
    until("first activity page", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.page == Some(0))
    })
    .await?;
    if snapshot.source != ui.output.activity_probe().unwrap().source {
        return Err("Paging replaced the activity snapshot identity".into());
    }
    snapshot.search.set_text("raw-tail-marker 中文😀");
    snapshot.find.emit_clicked();
    until("snapshot exact search", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.selected)
    })
    .await?;
    if !displayed(&ui.output.activity_probe().unwrap()).contains("raw-tail-marker 中文😀") {
        return Err("Full snapshot search missed provider metadata".into());
    }
    let output = directory.join("activity-export.json");
    ui.output.activity_export(output.clone())?;
    until("complete activity export", || {
        !ui.output.writing() && output.is_file()
    })
    .await?;
    let exported: Value =
        serde_json::from_slice(&std::fs::read(&output).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if exported != *raw.iter().find(|v| v["id"] == "large-tool").unwrap() {
        return Err("Export wrote only the visible activity page".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &snapshot.dialog,
        &directory.join("activity-large.png"),
    )
    .await?;
    close(ui).await?;
    let mut plan_id = None;
    for status in ["inProgress", "completed"] {
        emit(
            directory,
            json!([{"method":"turn/plan/updated","params":{"threadId":selected.id,"turnId":"activity-turn","explanation":"Native plan updates 中文😀","plan":[{"step":"Inspect preserved activity metadata","status":status}]}}]),
        )?;
        until("plan notification", || {
            ui.state
                .borrow()
                .items
                .iter()
                .any(|i| i.role == "Plan" && i.body.contains(status))
        })
        .await?;
        let s = ui.state.borrow();
        let plans = s
            .items
            .iter()
            .filter(|i| i.role == "Plan")
            .collect::<Vec<_>>();
        if plans.len() != 1
            || plan_id.as_ref().is_some_and(|id| id != &plans[0].id)
            || s.active_turn != before
        {
            return Err("Plan update duplicated rows or changed active-turn state".into());
        }
        plan_id = Some(plans[0].id.clone());
    }
    inspect(ui, plan_id.as_ref().unwrap()).await?;
    close(ui).await?;
    if ui.composer_text() != draft
        || ui.state.borrow().selected != Some(selected)
        || ui.state.borrow().active_turn != before
    {
        return Err("Activity inspection changed the live session or draft".into());
    }
    Ok(
        json!({"ok":true,"nativeInspectControls":true,"inputErrorStatusFutureFields":true,"metadataOnlyRebind":true,"immutableSnapshot":true,"exactPageCopy":true,"readOnly":true,"reasoningFallback":true,"commandArrays":true,"fileChangeKinds":true,"searchAndFutureActivities":true,"largePagingSearchExport":true,"liveLanguage":true,"planNotificationReplacement":true,"draftAndLifecyclePreserved":true}),
    )
}

/// Exercise the OpenCode HTTP/history and SDK event adapter rather than feeding
/// already-normalized Codex items into the GTK model.
async fn opencode(ui: &Rc<Ui>, directory: &Path) -> Result<Value, String> {
    let parts: Vec<Value> = serde_json::from_slice(
        &std::fs::read(directory.join("activity-opencode.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let selected = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("Missing OpenCode selection")?;
    let before = ui.state.borrow().active_turn.clone();
    if before.is_some() {
        return Err("Completed OpenCode fixture unexpectedly active".into());
    }
    let draft = "Unsent OpenCode inspection draft 中文😀";
    ui.composer.buffer().set_text(draft);
    inspect(ui, "tool-small").await?;
    let original = ui.output.activity_probe().unwrap();
    let text = displayed(&original);
    let raw: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if raw["opencode"] != parts[0]
        || raw["status"] != "error"
        || raw["aggregatedOutput"] != "Provider error 中文😀"
    {
        return Err("OpenCode tool input/error/metadata or server identity was lost".into());
    }
    original.copy.emit_clicked();
    if original
        .dialog
        .clipboard()
        .read_text_future()
        .await
        .map_err(|e| e.to_string())?
        .as_deref()
        != Some(&text)
    {
        return Err("OpenCode activity Copy changed the displayed page".into());
    }
    let mut updated = parts[0].clone();
    updated["state"]["input"]["command"] = "Updated OpenCode metadata 中文😀".into();
    let previous = original.source.clone();
    std::fs::write(directory.join("sdk-events.json"), serde_json::to_vec(&json!([
        {"directory":selected.cwd,"payload":{"type":"message.updated","properties":{"info":{"id":"activity-assistant","parentID":"activity-user","role":"assistant","sessionID":selected.id,"time":{"completed":123}}}}},
        {"directory":selected.cwd,"payload":{"type":"message.part.updated","properties":{"part":updated}}}
    ])).unwrap()).map_err(|e| e.to_string())?;
    until("OpenCode metadata event", || {
        ui.state
            .borrow()
            .items
            .iter()
            .find(|i| i.id == "tool-small")
            .is_some_and(|i| i.details.as_ref() != Some(&previous))
    })
    .await?;
    if displayed(&original) != text {
        return Err("OpenCode event rewrote an immutable snapshot".into());
    }
    close(ui).await?;
    inspect(ui, "tool-small").await?;
    let snapshot = ui.output.activity_probe().unwrap();
    let raw: Value = serde_json::from_str(&displayed(&snapshot)).map_err(|e| e.to_string())?;
    if raw["opencode"] != updated {
        return Err("OpenCode metadata-only update did not rebind inspector".into());
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &snapshot.dialog,
        &directory.join("activity-opencode.png"),
    )
    .await?;
    close(ui).await?;
    inspect(ui, "reasoning-content").await?;
    if !displayed(&ui.output.activity_probe().unwrap()).contains("Visible OpenCode reasoning") {
        return Err("OpenCode visible reasoning was lost".into());
    }
    close(ui).await?;
    inspect(ui, "large-tool").await?;
    let snapshot = ui.output.activity_probe().unwrap();
    if snapshot.pages.unwrap_or(0) < 2
        || displayed(&snapshot).len() > crate::conversation_output::PAGE_BYTES + 64
    {
        return Err("Large OpenCode activity was not bounded into pages".into());
    }
    snapshot.last.emit_clicked();
    until("OpenCode last page", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.page == p.pages.map(|n| n - 1))
    })
    .await?;
    snapshot.search.set_text("raw-tail-marker 中文😀");
    snapshot.find.emit_clicked();
    until("OpenCode exact search", || {
        ui.output
            .activity_probe()
            .is_some_and(|p| p.ready && p.selected)
    })
    .await?;
    let output = directory.join("activity-export.json");
    ui.output.activity_export(output.clone())?;
    until("OpenCode complete export", || {
        !ui.output.writing() && output.is_file()
    })
    .await?;
    let exported: Value =
        serde_json::from_slice(&std::fs::read(output).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if exported["opencode"] != parts[2] {
        return Err("OpenCode export lost original metadata".into());
    }
    close(ui).await?;
    if ui.composer_text() != draft
        || ui.state.borrow().selected != Some(selected)
        || ui.state.borrow().active_turn != before
    {
        return Err("OpenCode inspection changed selection, lifecycle or draft".into());
    }
    Ok(
        json!({"ok":true,"provider":"opencode","historyAdapter":true,"sdkMetadataUpdate":true,"immutableSnapshot":true,"originalInputErrorMetadata":true,"serverIdentity":true,"exactCopy":true,"visibleReasoning":true,"largePagingSearchExport":true,"draftAndLifecyclePreserved":true}),
    )
}
