//! Exercise real dispatch/close signals against private files, without provider turns.
use super::*;
use crate::workspace_navigation::Pane;
use std::path::Path;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(25) {
            return Err(format!("Viewer navigation timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn close(widget: &gtk::Widget, tooltip: &str) -> Result<(), String> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        if button.tooltip_text().as_deref() == Some(tooltip) {
            button.emit_clicked();
            return Ok(());
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if close(&widget, tooltip).is_ok() {
            return Ok(());
        }
    }
    Err(format!("Missing viewer control: {tooltip}"))
}
fn current(ui: &Ui, pane: Pane) -> Result<(), String> {
    if ui.current_tool() == Some(pane) {
        Ok(())
    } else {
        Err(format!("Expected {pane:?}, got {:?}", ui.current_tool()))
    }
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    until("session", || ui.state.borrow().history_verified).await?;
    ui.composer
        .buffer()
        .set_text("Unsent viewer navigation draft 中文😀");
    let root = directory.join("project");
    ui.show_tool("files");
    ui.open_local_file(&root.join("start.md"));
    until("Markdown", || ui.reader.is_ready()).await?;
    current(ui, Pane::Document)?;
    let original = ui.reader.location().ok_or("Missing original document")?;
    let retained = if std::env::var_os("STUDIO_GNOME_SMOKE_NAV_SELECTION").is_some() {
        Some(ui.reader.smoke_workspace_transfer(directory, None).await?)
    } else {
        None
    };
    let stale_offer = retained
        .as_ref()
        .map(crate::reader::Reader::smoke_workspace_offer)
        .transpose()?;
    let mut pulse = super::conversation_pages_acceptance::Probe::new();

    // The actual Markdown link dispatch, not a test-only pane switch.
    ui.reader.activate_link("sample.png");
    until("image", || ui.image_viewer.smoke_ready()).await?;
    current(ui, Pane::Image)?;
    close(ui.image_viewer.widget.upcast_ref(), "Close image")?;
    current(ui, Pane::Document)?;
    if ui.reader.location() != Some(original.clone()) || !ui.reader.is_ready() {
        return Err("Image close rebuilt or lost the parent document".into());
    }
    if let Some(view) = &retained {
        ui.reader
            .smoke_workspace_transfer(directory, Some(view))
            .await?;
        crate::reader::Reader::smoke_workspace_stale_offer(stale_offer.as_ref().unwrap()).await?;
        ui.reader.smoke_workspace_transfer_races().await?;
        ui.reader
            .smoke_workspace_transfer(directory, Some(view))
            .await?;
    }
    ui.open_local_file(&root.join("sample.csv"));
    until("table", || ui.tables.smoke_ready()).await?;
    current(ui, Pane::Table)?;
    close(ui.tables.widget.upcast_ref(), "Close table")?;
    current(ui, Pane::Document)?;
    if let Some(view) = &retained {
        ui.reader
            .smoke_workspace_transfer(directory, Some(view))
            .await?;
    }
    ui.open_local_file(&root.join("sample.patch"));
    current(ui, Pane::Source)?;
    close(ui.source.widget.upcast_ref(), "Close source")?;
    current(ui, Pane::Document)?;
    if let Some(view) = &retained {
        ui.reader
            .smoke_workspace_transfer(directory, Some(view))
            .await?;
    }

    ui.open_local_file(&root.join("sample.png"));
    until("nested image", || ui.image_viewer.smoke_ready()).await?;
    ui.open_local_file(&root.join("sample.pdf"));
    until("PDF", || ui.pdf.smoke_ready()).await?;
    current(ui, Pane::Pdf)?;
    let opening = pulse.stop();
    capture("navigation-pdf.png");
    close(ui.pdf.widget.upcast_ref(), "Close PDF")?;
    current(ui, Pane::Image)?;
    close(ui.image_viewer.widget.upcast_ref(), "Close image")?;
    current(ui, Pane::Document)?;

    if let Some(view) = &retained {
        ui.reader
            .smoke_workspace_transfer(directory, Some(view))
            .await?;
    }

    ui.open_local_file(&root.join("sample.epub"));
    until("EPUB", || ui.epub.smoke_ready()).await?;
    current(ui, Pane::Epub)?;
    close(ui.epub.widget.upcast_ref(), "Close EPUB")?;
    current(ui, Pane::Document)?;
    if let Some(view) = &retained {
        ui.reader
            .smoke_workspace_transfer(directory, Some(view))
            .await?;
    }
    // A second request can be held until the previous reading position is saved.
    // Only the user's open intent may present it; late save/load ACKs must not.
    ui.open_local_file(&root.join("sample.epub"));
    current(ui, Pane::Epub)?;
    ui.show_tool("files");
    until("deferred EPUB", || ui.epub.smoke_ready()).await?;
    current(ui, Pane::Files)?;
    close(ui.epub.widget.upcast_ref(), "Close EPUB")?;
    current(ui, Pane::Files)?;

    ui.open_local_file(&root.join("start.md"));
    until("parent reopened", || ui.reader.is_ready()).await?;
    ui.browser.open(None);
    current(ui, Pane::Browser)?;
    close(ui.browser.widget.upcast_ref(), "Hide browser workspace")?;
    current(ui, Pane::Document)?;
    until("browser preference acknowledgement", || {
        ui.browser.smoke_preferences_ready()
    })
    .await?;
    current(ui, Pane::Document)?;
    ui.open_local_file(&root.join("sample.png"));
    until("stale-close image", || ui.image_viewer.smoke_ready()).await?;
    ui.open_local_file(&root.join("sample.pdf"));
    until("stale-close PDF", || ui.pdf.smoke_ready()).await?;
    close(ui.image_viewer.widget.upcast_ref(), "Close image")?;
    current(ui, Pane::Pdf)?;
    close(ui.pdf.widget.upcast_ref(), "Close PDF")?;
    current(ui, Pane::Document)?;
    ui.open_local_file(&root.join("sample.png"));
    until("closed parent image", || ui.image_viewer.smoke_ready()).await?;
    ui.reader.smoke_close();
    current(ui, Pane::Image)?;
    close(ui.image_viewer.widget.upcast_ref(), "Close image")?;
    current(ui, Pane::Files)?;

    // Returning to an ancestor must collapse the chain, not create a pane cycle.
    ui.open_local_file(&root.join("start.md"));
    until("cycle parent", || ui.reader.is_ready()).await?;
    ui.open_local_file(&root.join("sample.png"));
    ui.open_local_file(&root.join("start.md"));
    until("cycle return", || ui.reader.is_ready()).await?;
    ui.reader.smoke_close();
    current(ui, Pane::Files)?;
    ui.hide_tools();
    ui.open_local_file(&root.join("sample.png"));
    close(ui.image_viewer.widget.upcast_ref(), "Close image")?;
    if ui.current_tool().is_some() {
        return Err("Closed last viewer resurrected an old pane".into());
    }
    if ui.composer.buffer().text(
        &ui.composer.buffer().start_iter(),
        &ui.composer.buffer().end_iter(),
        true,
    ) != "Unsent viewer navigation draft 中文😀"
    {
        return Err("Viewer navigation changed the unsent conversation draft".into());
    }
    ui.open_local_file(&root.join("start.md"));
    until("final document", || ui.reader.is_ready()).await?;
    capture("navigation-return.png");
    Ok(
        json!({"ok":true,"markdownLink":true,"image":true,"source":true,"table":true,"pdf":true,
        "epub":true,"nestedReturn":true,"staleClose":true,"closedParentSkipped":true,
        "deferredEpubNoFocusSteal":true,"browserNoFocusSteal":true,"cycleCollapsed":true,"hiddenReset":true,"unsentDraft":true,"retainedTableTransfers":retained.is_some(),"cancelledTransferCannotPublish":retained.is_some(),"retiredDragOfferRejected":retained.is_some(),"openingHeartbeat":opening}),
    )
}
