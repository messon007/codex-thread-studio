//! GTK chooser and FileLauncher calls cross the real private D-Bus boundary.
use super::*;
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    let at = std::time::Instant::now();
    while !ui.state.borrow().history_verified || !ui.window.is_mapped() {
        if at.elapsed() > Duration::from_secs(25) {
            return Err("File portal history readiness timed out".into());
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    ui.composer
        .buffer()
        .set_text("Unsent file portal draft 中文😀 ");
    ui.show_tool("document");
    let reader = ui
        .reader
        .smoke_file_picker(root, || ui.show_tool("document"))
        .await?;
    if ui.tools.visible_child_name().as_deref() != Some("document") {
        return Err("Reader chooser test left a different workspace visible".into());
    }
    glib::timeout_future(Duration::from_millis(100)).await;
    smoke::capture(&ui.window, &root.join("file-portal-reader.png"));
    let shell = shell_picker(ui, root).await?;
    ui.open_settings();
    let settings = ui
        .settings
        .borrow()
        .clone()
        .ok_or("Settings did not open")?;
    let shared = settings.smoke_shared();
    let folder = shared.smoke_portal_picker(root).await?;
    settings.dialog.close();
    ui.show_tool("files");
    ui.files.open(&root.join("project").to_string_lossy());
    let launcher = ui
        .files
        .smoke_portal_launch(root, &root.join("picked shared 中文 # "))
        .await?;
    if ui.composer_text() != "Unsent file portal draft 中文😀 " {
        return Err("File portal work changed the chat draft".into());
    }
    Ok(
        json!({"ok":true,"reader":reader,"shell":shell,"sharedFolders":folder,"launcher":launcher,"draftRetained":true}),
    )
}

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Shell chooser timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn shell_picker(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    crate::smoke_portal::wait(root, "calls", 8).await?;
    until("cancel released", || !ui.file_picker.is_pending()).await?;
    if !ui.reader.is_ready() {
        return Err("Shell cancel lost the Markdown reader".into());
    }
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    crate::smoke_portal::wait(root, "calls", 9).await?;
    until("Rust source opened", || {
        ui.source
            .smoke_file_state()
            .is_some_and(|(p, _)| p.ends_with("picked.rs"))
    })
    .await?;
    if ui.source.smoke_file_state().unwrap().1.as_deref() != Some("rust") {
        return Err("Chosen Rust file lost syntax highlighting".into());
    }
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    crate::smoke_portal::wait(root, "calls", 10).await?;
    until("error displayed", || {
        !ui.file_picker.is_pending() && ui.status.text().starts_with("Unable to open file chooser:")
    })
    .await?;
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    crate::smoke_portal::wait(root, "calls", 11).await?;
    until("nonlocal rejected", || {
        !ui.file_picker.is_pending() && ui.status.text().starts_with("Choose a local file")
    })
    .await?;
    if !ui
        .source
        .smoke_file_state()
        .is_some_and(|(p, _)| p.ends_with("picked.rs"))
    {
        return Err("Failed shell chooser changed source".into());
    }
    ui.source.smoke_edit_mode(true);
    until("editor ready", || !ui.source.edit_text().is_empty()).await?;
    ui.source.edit_insert("// unsaved portal draft 中文😀\n");
    let draft = ui.source.edit_text();
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    crate::smoke_portal::wait(root, "calls", 12).await?;
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    glib::timeout_future(Duration::from_millis(120)).await;
    if crate::smoke_portal::read(root).await?["calls"]
        .as_array()
        .map(Vec::len)
        != Some(12)
    {
        return Err("Shell Open did not coalesce pending requests".into());
    }
    let other = root.join("other directory/picked.sh");
    ui.open_local_file(&other);
    crate::smoke_portal::wait(root, "closed", 4).await?;
    ui.source.edit_response("cancel").await?;
    if ui.source.edit_text() != draft {
        return Err("Picker replacement bypassed dirty editor Cancel".into());
    }
    ui.open_local_file(&other);
    ui.source.edit_response("discard").await?;
    until("shell source", || {
        ui.source
            .smoke_file_state()
            .is_some_and(|(p, l)| p.ends_with("picked.sh") && l.as_deref() == Some("sh"))
    })
    .await?;
    glib::timeout_future(Duration::from_millis(140)).await;
    if ui.file_picker.is_pending()
        || !ui
            .source
            .smoke_file_state()
            .is_some_and(|(p, _)| p.ends_with("picked.sh"))
    {
        return Err("Late canceled shell result replaced newer file".into());
    }
    smoke::capture(&ui.window, &root.join("file-portal-source.png"));
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None);
    crate::smoke_portal::wait(root, "calls", 13).await?;
    ui.show_tool("files");
    ui.window
        .lookup_action("open-file")
        .ok_or("Missing Open file action")?
        .activate(None); // reopen before old cancellation callback has run
    crate::smoke_portal::wait(root, "closed", 5).await?;
    crate::smoke_portal::wait(root, "calls", 14).await?;
    glib::timeout_future(Duration::from_millis(140)).await;
    if !ui.file_picker.is_pending() {
        return Err("Old shell callback cleared newer chooser".into());
    }
    ui.window.set_visible(false);
    crate::smoke_portal::wait(root, "closed", 6).await?;
    if ui.file_picker.is_pending() {
        return Err("Unmapped app retained chooser".into());
    }
    ui.window.present();
    until("remapped window", || ui.window.is_mapped()).await?;
    Ok(
        json!({"allFiles":true,"sourceSyntax":true,"localUnicodePath":true,"errorsVisible":true,"nonlocalRejected":true,"duplicateOpenCoalesced":true,"dirtyReplacementCancelDiscard":true,"lateReplyIgnored":true,"newChooserRetained":true,"navigationCancellation":true,"unmapCancellation":true}),
    )
}
