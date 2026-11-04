//! Owned profile and fake-provider translation workflow; no real prompts/audio.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(35) {
            return Err(format!("Translation acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn write(root: &std::path::Path, name: &str, text: &str) -> Result<(), String> {
    let path = root.join(name);
    let text = text.to_owned();
    gio::spawn_blocking(move || std::fs::write(path, text).map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Translation fixture writer stopped")?
}
fn draft(ui: &Ui) -> String {
    let b = ui.composer.buffer();
    b.text(&b.start_iter(), &b.end_iter(), true).into()
}
async fn capture_translation(
    ui: &Ui,
    dialog: &impl IsA<adw::Dialog>,
    path: &std::path::Path,
) -> Result<(), String> {
    crate::window_capture::capture_dialog(&ui.window, dialog, path).await
}
pub(super) async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    until("source conversation", || {
        ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| t.id == "target")
    })
    .await?;
    let source = ui.state.borrow().selected.clone().unwrap();
    let unsent =
        "Unsent draft: Translate **English** <b>literal</b> 中文😀\n.\nCANCEL all\n!.. raw text";
    ui.composer.buffer().set_text(unsent);
    let b = ui.composer.buffer();
    b.select_range(&b.start_iter(), &b.end_iter());
    ui.composer
        .activate_action("translation.translate", None)
        .map_err(|e| e.to_string())?;
    until("native translation", || {
        ui.translation
            .smoke_view()
            .is_some_and(|(_, _, _, _, _, ready)| ready)
            && ui.translation.smoke_idle()
    })
    .await?;
    let (dialog, selected, result, profile, _, _) = ui.translation.smoke_view().unwrap();
    if selected != unsent
        || result != "译文 **Markdown** <b>literal</b> 中文😀"
        || !profile.contains("translate-codex")
    {
        return Err(format!(
            "Native translation fields differ: {selected:?} {result:?} {profile:?}"
        ));
    }
    ui.translation.smoke_copy();
    let copied = ui
        .window
        .clipboard()
        .read_text_future()
        .await
        .map_err(|e| e.to_string())?;
    if copied.as_deref() != Some(result.as_str())
        || draft(ui) != unsent
        || ui.state.borrow().selected.as_ref() != Some(&source)
        || ui.state.borrow().active_turn.is_some()
    {
        return Err("Translation changed source conversation/draft or copy".into());
    }
    capture_translation(ui, &dialog, &root.join("translation-codex.png")).await?;
    write(root, "speech-mode", "hold").await?;
    ui.translation.smoke_listen(false);
    until("English speech held", || {
        root.join("speech-held-1").exists() && ui.translation.smoke_audio().0
    })
    .await?;
    ui.translation.smoke_stop_audio();
    until("English speech stopped", || {
        let (busy, status) = ui.translation.smoke_audio();
        !busy && status == "Playback stopped"
    })
    .await?;
    write(root, "speech-mode", "early-end").await?;
    ui.translation.smoke_listen(true);
    until("Chinese speech complete before ACK", || {
        let (busy, status) = ui.translation.smoke_audio();
        !busy && status == "Playback complete"
    })
    .await?;
    write(root, "speech-mode", "hold").await?;
    ui.translation.smoke_listen(false);
    until("speech before close", || {
        root.join("speech-held-3").exists()
    })
    .await?;
    capture_translation(ui, &dialog, &root.join("translation-speech.png")).await?;
    ui.translation.smoke_close();
    until("closing cancels speech", || {
        !ui.translation.smoke_audio().0 && ui.translation.smoke_view().is_none()
    })
    .await?;
    ui.translation.start(unsent.into());
    until("cached translation", || {
        ui.translation
            .smoke_view()
            .is_some_and(|(_, _, _, _, status, ready)| ready && status.contains("cache"))
    })
    .await?;
    write(root, "speech-mode", "hold-unconfirmed").await?;
    ui.translation.smoke_listen(false);
    until("unresponsive speech", || {
        root.join("speech-held-4").exists()
    })
    .await?;
    ui.translation.smoke_stop_audio();
    until("unconfirmed stop released", || {
        let (busy, status) = ui.translation.smoke_audio();
        !busy && status.contains("stop was not confirmed")
    })
    .await?;
    ui.translation.smoke_close();
    ui.show_tool("document");
    let projected = ui
        .reader
        .smoke_projected_translation(&ui.translation, root)
        .await?;
    if draft(ui) != unsent {
        return Err("Projected translation changed the source draft".into());
    }
    let html = super::html_action_acceptance::run(ui, root).await?;
    // Browser selection stays in the isolated content world; only literal text
    // reaches the native task. The cached source avoids another provider turn.
    let page = root.join("translation-page-url");
    let url = gio::spawn_blocking(move || std::fs::read_to_string(page))
        .await
        .map_err(|_| "Fixture URL worker stopped")?
        .map_err(|e| e.to_string())?;
    ui.show_tool("browser");
    ui.browser.smoke_translation(&url).await?;
    until("isolated browser selection", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, source, _, _, status, ready)| {
                    source == unsent && ready && status.contains("cache")
                })
    })
    .await?;
    ui.translation.smoke_close();
    ui.hide_tools();
    ui.show_tool("epub");
    ui.epub.open(&source.cwd, "translation.epub");
    ui.epub.smoke_translation().await?;
    until("EPUB retained selection", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, selected, _, _, status, ready)| {
                    selected == unsent && ready && status.contains("cache")
                })
    })
    .await?;
    ui.translation.smoke_close();
    ui.epub.close();
    ui.show_tool("pdf");
    ui.pdf.open(&source.cwd, "translation.pdf", 0);
    ui.pdf.smoke_translation(true).await?;
    until("PDF Preview translation", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, selected, _, _, _, ready)| selected == "Page" && ready)
    })
    .await?;
    write(root, "speech-mode", "early-end").await?;
    ui.translation.smoke_listen(true);
    until("Mandarin fallback without pinyin", || {
        let (busy, status) = ui.translation.smoke_audio();
        !busy && status == "Playback complete"
    })
    .await?;
    ui.translation.smoke_close();
    ui.pdf.smoke_translation(false).await?;
    until("PDF Text cached translation", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, selected, _, _, status, ready)| {
                    selected == "Page" && ready && status.contains("cache")
                })
    })
    .await?;
    ui.translation.smoke_close();
    ui.pdf.close();
    ui.hide_tools();
    // Cancel after creation was sent but before its identity reply. The late
    // identity must be cleaned without sending a translation turn.
    write(root, "hold-utility-create", "hold").await?;
    ui.translation.start("Cancel during creation".into());
    until("held creation", || {
        root.join("utility-create-held").exists()
    })
    .await?;
    ui.translation.smoke_close();
    write(root, "release-utility-create", "release").await?;
    until("cancelled creation cleaned", || ui.translation.smoke_idle()).await?;
    ui.translation
        .start("Invalid structured translation".into());
    until("invalid result", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, _, _, _, status, ready)| !ready && status.contains("structured"))
    })
    .await?;
    ui.translation.smoke_close();
    // The settings page edits shared profiles without running a model.
    ui.open_settings();
    let settings = ui.settings.borrow().clone().unwrap();
    let panel = settings.smoke_translation();
    panel.smoke_ready().await?;
    panel
        .smoke_save("ollama", "translate-codex", "low", "fixture-local")
        .await?;
    panel.smoke_models().await?;
    capture_translation(ui, &panel.dialog, &root.join("translation-settings.png")).await?;
    panel.dialog.close();
    until("translation settings closed", || !panel.dialog.is_mapped()).await?;
    settings.dialog.close();
    until("settings closed", || ui.settings.borrow().is_none()).await?;
    ui.translation.start("Local independent translation".into());
    until("local translation", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, _, _, profile, _, ready)| {
                    ready && profile.contains("fixture-local")
                })
    })
    .await?;
    ui.translation.smoke_close();
    ui.open_settings();
    let settings = ui.settings.borrow().clone().unwrap();
    let panel = settings.smoke_translation();
    panel.smoke_ready().await?;
    panel
        .smoke_save("backend", "translate-codex", "low", "fixture-local")
        .await?;
    panel.dialog.close();
    until("settings child closed", || !panel.dialog.is_mapped()).await?;
    settings.dialog.close();
    until("settings parent closed", || ui.settings.borrow().is_none()).await?;
    if draft(ui) != unsent
        || ui.state.borrow().selected.as_ref() != Some(&source)
        || ui.state.borrow().active_turn.is_some()
    {
        return Err("Utility work changed the source conversation".into());
    }
    let codex = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "codex")
        .cloned()
        .ok_or("Missing Codex fixture")?;
    let opencode = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "opencode")
        .cloned()
        .ok_or("Missing OpenCode fixture")?;
    ui.connect_backend_to(
        opencode,
        false,
        Some(Thread {
            id: "oc-target".into(),
            parent: None,
            title: "Native oc-target".into(),
            cwd: root.join("project-oc").to_string_lossy().into(),
            pinned: false,
            activity: String::new(),
        }),
    );
    until("OpenCode source", || {
        let state = ui.state.borrow();
        state.backend == "opencode"
            && state.ready
            && !state.loading
            && state.selected.as_ref().is_some_and(|t| t.id == "oc-target")
    })
    .await?;
    ui.composer.buffer().set_text("OpenCode unsent draft");
    write(root, "hold-oc-translation", "hold").await?;
    ui.translation
        .start("Independent OpenCode translation".into());
    until("live OpenCode utility", || {
        root.join("oc-translation-held").exists()
    })
    .await?;
    until("ordinary OpenCode event received", || {
        ui.state
            .borrow()
            .activity
            .get("opencode:oc-similar")
            .is_some_and(|a| a.active)
    })
    .await?;
    if ui
        .state
        .borrow()
        .activity
        .keys()
        .any(|key| key.starts_with("opencode:oc-translation-"))
    {
        return Err("Owned utility events entered native background activity".into());
    }
    ui.catalog(false);
    until("OpenCode catalog refreshed", || {
        !ui.state.borrow().catalog_loading
    })
    .await?;
    {
        let state = ui.state.borrow();
        if state
            .threads
            .iter()
            .any(|t| t.id.starts_with("oc-translation-"))
            || !state.threads.iter().any(|t| t.id == "oc-similar")
        {
            return Err("Live utility filtering hid a user row or exposed a temporary task".into());
        }
    }
    ui.open_router_settings();
    let panel = ui
        .router_settings
        .borrow()
        .clone()
        .ok_or("Missing Router settings")?;
    until("live Router targets", || {
        panel.smoke_target_keys().is_some()
    })
    .await?;
    let keys = panel.smoke_target_keys().unwrap();
    if keys
        .iter()
        .any(|key| key.starts_with("opencode:oc-translation-"))
        || !keys.iter().any(|key| key == "opencode:oc-similar")
    {
        return Err("Router picker exposed a temporary task or hid a user session".into());
    }
    capture_translation(
        ui,
        &panel.dialog,
        &root.join("translation-live-router-targets.png"),
    )
    .await?;
    panel.dialog.close();
    until("Router settings closed", || {
        ui.router_settings.borrow().is_none()
    })
    .await?;
    write(root, "release-oc-translation", "release").await?;
    until("OpenCode translation", || {
        ui.translation.smoke_idle()
            && ui
                .translation
                .smoke_view()
                .is_some_and(|(_, _, result, _, _, ready)| {
                    ready && result == "OpenCode 译文 中文😀"
                })
    })
    .await?;
    until("ordinary OpenCode completion received", || {
        ui.state
            .borrow()
            .activity
            .get("opencode:oc-similar")
            .is_some_and(|a| !a.active && a.attention)
    })
    .await?;
    if ui
        .state
        .borrow()
        .activity
        .keys()
        .any(|key| key.starts_with("opencode:oc-translation-"))
    {
        return Err("Owned utility completion entered native attention/notifications".into());
    }
    if draft(ui) != "OpenCode unsent draft" || ui.state.borrow().active_turn.is_some() {
        return Err("OpenCode utility changed its source conversation".into());
    }
    let (dialog, ..) = ui.translation.smoke_view().unwrap();
    capture_translation(ui, &dialog, &root.join("translation-opencode.png")).await?;
    ui.translation.smoke_close();
    let stale_selection = crate::translation_ui::capture().ok_or("Missing translation context")?;
    ui.connect_backend_to(codex, false, Some(source.clone()));
    until("Codex source restored", || {
        let state = ui.state.borrow();
        state.backend == "codex"
            && state.ready
            && !state.loading
            && state.selected.as_ref().is_some_and(|t| t.id == source.id)
    })
    .await?;
    if draft(ui) != unsent {
        return Err("Backend switch lost Codex draft".into());
    }
    stale_selection("Late text from the old OpenCode document".into());
    if !ui.translation.smoke_idle() || ui.translation.smoke_view().is_some() {
        return Err("Stale document selection started a task on another backend".into());
    }
    Ok(
        json!({"ok":true,"projectedSelection":projected,"htmlActions":html,"nativeSelectionAction":true,"browserSelection":true,"epubTranslation":true,"pdfPreviewTranslation":true,"pdfTextTranslation":true,"scopedSpeech":true,"speechCloseCleanup":true,"speechTimeout":true,"staleContextRejected":true,"earlyCompletion":true,"structuredLiteralOutput":true,"exactCopy":true,"cache":true,"cancelBeforeCreateAck":true,"invalidStructuredResult":true,"sharedSettings":true,"localModels":true,"localTranslation":true,"opencodeTranslation":true,"liveUtilityCatalogHidden":true,"liveUtilityPickerHidden":true,"utilityEventsIsolated":true,"ordinarySessionActivityRetained":true,"similarUserSessionRetained":true,"sourceDraftRetained":true,"sourceSessionRetained":true}),
    )
}
