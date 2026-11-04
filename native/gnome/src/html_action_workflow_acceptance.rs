//! Actual saved comment library and HTML translation controls, private profile only.
use super::*;
use std::path::Path;

pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
        crate::acceptance_wait::until(step, test).await
    }
    let project = root.join("project");
    let project = project.to_str().ok_or("Non-UTF8 HTML fixture path")?;
    let buffer = ui.composer.buffer();
    let original_draft = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    ui.show_tool("source");
    let mut records = Vec::new();
    for (paragraph, quote, warning) in [
        (0, "Unique literal 中文😀", false),
        (1, "A & B bold 中文😀", true),
        (2, "Duplicate passage", true),
    ] {
        let capture = ui
            .source
            .smoke_html_capture(project, paragraph, quote, true)
            .await?;
        if capture.source["anchor"]["textKind"] != "preview"
            || !capture.source["anchor"]["startLine"].is_null()
            || !capture.source["anchor"]["startOffset"].is_null()
        {
            return Err("HTML capture fabricated source coordinates".into());
        }
        let panel = ui
            .annotations
            .borrow()
            .clone()
            .ok_or("HTML Comment button did not open library")?;
        let record = panel.smoke_save_source_capture(&capture).await?;
        let id = record["id"]
            .as_str()
            .ok_or("HTML saved comment ID missing")?;
        if !panel.smoke_open_record(id)? {
            return Err("HTML saved comment could not reopen".into());
        }
        until("HTML saved source reopen", || {
            ui.annotations.borrow().is_none()
                && ui
                    .source
                    .smoke_html_reopened(warning, (!warning).then_some(quote))
        })
        .await?;
        // A fresh Comments panel loads the shared store again. Reopen must not
        // mutate its anchor or silently verify an unsafe location.
        ui.comment_panel(None);
        let panel = ui
            .annotations
            .borrow()
            .clone()
            .ok_or("HTML library did not reload")?;
        until("HTML saved library reloaded", || {
            panel.smoke_chat_records().is_some()
        })
        .await?;
        if !panel.smoke_chat_records().unwrap().contains(&record) {
            return Err("HTML comment changed after shared-store reload".into());
        }
        if !panel.smoke_open_record(id)? {
            return Err("HTML reloaded comment could not reopen".into());
        }
        until("HTML reloaded source reopen", || {
            ui.annotations.borrow().is_none()
                && ui
                    .source
                    .smoke_html_reopened(warning, (!warning).then_some(quote))
        })
        .await?;
        records.push(record);
    }
    crate::window_capture::capture(&ui.window, &root.join("html-comment-unverified.png")).await?;
    let quote = "A & B bold 中文😀";
    ui.source
        .smoke_html_capture(project, 1, quote, false)
        .await?;
    ui.source.smoke_html_translate(root, false).await?;
    until("HTML keyboard translation", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, text, _, _, _, ready)| text == quote && ready)
    })
    .await?;
    let (dialog, ..) = ui.translation.smoke_view().unwrap();
    crate::window_capture::capture_dialog(&ui.window, &dialog, &root.join("html-translation.png"))
        .await?;
    ui.translation.smoke_close();
    until("HTML translation closed", || {
        ui.translation.smoke_view().is_none()
    })
    .await?;
    ui.source.smoke_html_translate(root, true).await?;
    until("HTML menu cached translation", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, text, _, _, status, ready)| {
                    text == quote && ready && status.contains("cache")
                })
    })
    .await?;
    ui.translation.smoke_close();
    until("HTML menu translation closed", || {
        ui.translation.smoke_view().is_none()
    })
    .await?;
    let buffer = ui.composer.buffer();
    buffer.place_cursor(&buffer.start_iter());
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    ui.source.smoke_html_clear_selection().await?;
    ui.source.smoke_html_translate(root, false).await?;
    glib::timeout_future(Duration::from_millis(150)).await;
    if ui.translation.smoke_view().is_some() || !ui.translation.smoke_idle() {
        return Err("Empty focused HTML translated another widget's selection".into());
    }
    let buffer = ui.composer.buffer();
    if buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .as_str()
        != original_draft
    {
        return Err("HTML actions changed the conversation draft".into());
    }
    ui.hide_tools();
    Ok(json!({"ok":true,"records":records,"sourceReopen":true,
        "literalVerified":true,"entitiesAndMarkupUnverified":true,"duplicateUnverified":true,
        "sharedStoreReload":true,"keyboardTranslation":true,"contextMenuAction":true,"contextMenuPointerAnchor":true,"emptySelectionScoped":true,
        "translationCache":true,"conversationDraftRetained":true}))
}
