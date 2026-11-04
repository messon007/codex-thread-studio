//! Public HTTPS artwork through normal chat/reader factories and native Save.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    crate::acceptance_wait::until(step, test).await
}
fn previews(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut out = Vec::new();
    if root.is_mapped() && root.widget_name() == "studio-remote-image" {
        out.push(root.clone());
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        out.extend(previews(&w));
    }
    out
}
fn expand(root: &gtk::Widget) -> Option<gtk::Button> {
    if let Some(button) = root
        .downcast_ref::<gtk::Button>()
        .filter(|b| b.tooltip_text().as_deref() == Some("Open image viewer"))
    {
        return Some(button.clone());
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        if let Some(button) = expand(&w) {
            return Some(button);
        }
    }
    None
}
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    until("remote image startup", || {
        ui.state.borrow().startup_ready && ui.state.borrow().history_verified
    })
    .await?;
    let started = std::time::Instant::now();
    until("remote chat and attachment textures", || {
        previews(ui.transcript_view.upcast_ref()).len() == 2
            && crate::images_ui::loaded_count(ui.transcript_view.upcast_ref(), true) >= 2
    })
    .await?;
    let chat_ms = started.elapsed().as_millis();
    ui.composer
        .buffer()
        .set_text("Unsent HTTPS image draft 中文😀 ");
    crate::window_capture::capture(&ui.window, &root.join("remote-chat.png")).await?;
    let cards = previews(ui.transcript_view.upcast_ref());
    expand(&cards[0])
        .ok_or("Remote chat viewer action absent")?
        .emit_clicked();
    until("remote chat viewer", || ui.image_viewer.smoke_ready()).await?;
    ui.image_viewer
        .smoke_embedded_save(root, "saved-remote.png", (512, 512))
        .await?;
    crate::window_capture::capture(&ui.window, &root.join("remote-viewer.png")).await?;
    ui.image_viewer.close();
    ui.reader.open_path(&root.join("project/remote.md"));
    ui.show_tool("document");
    until("remote reader texture", || {
        ui.reader.is_ready()
            && previews(ui.reader.widget.upcast_ref()).len() == 1
            && crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), true) >= 1
    })
    .await?;
    crate::window_capture::capture(&ui.window, &root.join("remote-reader.png")).await?;
    expand(&previews(ui.reader.widget.upcast_ref())[0])
        .ok_or("Remote reader viewer action absent")?
        .emit_clicked();
    until("remote reader viewer", || ui.image_viewer.smoke_ready()).await?;
    // Read source dimensions from the fixed upstream SVG through its own shared
    // raster rule (2048 preview edge), retaining original source for Save.
    ui.image_viewer
        .smoke_embedded_save(root, "saved-remote.svg", (2048, 2048))
        .await?;
    ui.image_viewer.close();
    if ui.composer_text() != "Unsent HTTPS image draft 中文😀 " {
        return Err("Remote image changed composer draft".into());
    }
    Ok(
        json!({"ok":true,"chatAndAttachment":true,"reader":true,"nativeViewer":true,"explicitOriginalSave":true,"draftRetained":true,"chatReadyMs":chat_ms}),
    )
}
