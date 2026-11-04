//! Private-desktop image-anchor acceptance using the production renderer.
//! Decode replies are synthetic RGBA: codec/file confinement is covered by the
//! separate gateway image suite. No providers, network or user files are used.
use super::*;
use crate::{images::Source, images_ui::Renderer, protocol::Command};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use studio_markdown_native::Fragment;
use tokio::sync::mpsc;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let started = Instant::now();
    while !test() {
        if started.elapsed() > Duration::from_secs(10) {
            return Err(format!("Text-flow image probe timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn frame() -> crate::images::Frame {
    crate::images::Frame {
        animation: None,
        animation_limited: false,
        original: (64, 32),
        width: 64,
        height: 32,
        pixels: [35, 145, 120, 255].repeat(64 * 32).into(),
        identity: None,
        source: None,
    }
}
fn pending(receiver: &mut mpsc::Receiver<Command>) -> Result<Vec<(u64, Source)>, String> {
    let mut jobs = Vec::new();
    while let Ok(command) = receiver.try_recv() {
        let Command::NativeImage { id, source, .. } = command else {
            return Err("Unexpected command from image renderer".into());
        };
        jobs.push((id, source));
    }
    if jobs.len() > 2 {
        return Err("Image renderer exceeded two admitted decodes".into());
    }
    Ok(jobs)
}
fn column_view(widget: &gtk::Widget) -> Option<gtk::ColumnView> {
    if let Some(view) = widget.downcast_ref::<gtk::ColumnView>() {
        return Some(view.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(view) = column_view(&widget) {
            return Some(view);
        }
    }
    None
}
fn loaded_picture(widget: &gtk::Widget) -> Option<crate::scaled_picture::ScaledPicture> {
    if let Some(picture) = widget.downcast_ref::<crate::scaled_picture::ScaledPicture>() {
        if picture.paintable().is_some() {
            return Some(picture.clone());
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(picture) = loaded_picture(&widget) {
            return Some(picture);
        }
    }
    None
}
fn cell_window() -> Option<gtk::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.is_mapped() && w.widget_name() == "studio-selection-window")
}
async fn serve(
    renderer: &Renderer,
    receiver: &mut mpsc::Receiver<Command>,
    seen: &mut Vec<Source>,
    count: usize,
) -> Result<(), String> {
    let started = Instant::now();
    loop {
        for (id, source) in pending(receiver)? {
            if !seen.contains(&source) {
                seen.push(source);
            }
            renderer.event(id, Ok(frame()));
        }
        if seen.len() >= count {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(10) {
            return Err(format!("Image anchor admission stalled: {seen:?}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
fn fixture() -> Result<Fragment, String> {
    let text = format!(
        "# Images\n\nBefore 中文😀 ![Local](nested/sample.png) after.\n\n![Inline](data:image/png;base64,QUFB)\n\n![Snapshot](data:image/png;base64,{})\n\n![Remote](https://example.invalid/image.png)\n\n{}\n\n| ![Header](nested/header.svg) | Value |\n|---|---|\n| ![Cell](nested/cell.png) | 中文😀 |\n",
        "QUFB".repeat(4000),
        "Context 中文😀.\n\n".repeat(80),
    );
    let source = studio_markdown_native::SourceWindow {
        start: 0,
        end: text.len(),
        line: 1,
        end_line: text.lines().count(),
        text,
        projection: vec![],
        references: vec![],
    };
    Ok(Fragment {
        page: 7,
        document_id: "flow-images".into(),
        revision: "a".repeat(64),
        metadata: studio_markdown_native::FragmentMetadata {
            start: 0,
            end: source.end,
            line: 1,
            rows: 0,
            row_start: 0,
            table: None,
        },
        source: studio_markdown_native::source_projection::project(source, 7)?,
        header: None,
    })
}

/// Only called by the owned Wayland architectural probe in debug builds.
pub async fn run(window: &adw::ApplicationWindow, root: &Path) -> Result<Value, String> {
    let prepared = gio::spawn_blocking(|| prepare(&fixture()?))
        .await
        .map_err(|_| "Image projection worker stopped")??;
    if view(prepared, Rc::new(|_| {})).is_ok() {
        return Err("Image-bearing document accepted without its source renderer".into());
    }
    let prepared = gio::spawn_blocking(|| prepare(&fixture()?))
        .await
        .map_err(|_| "Image projection worker stopped")??;
    let (sender, mut receiver) = mpsc::channel(32);
    let renderer = Renderer::new(sender);
    let open = Rc::new(|_: &str| {});
    let images = renderer.flow_images(
        root.to_owned(),
        Some(root.join("review.md")),
        open.clone(),
        Some(("flow-images".into(), "a".repeat(64))),
    );
    let flow = view_with_images(prepared, open, Some(images))?;
    let scroll = gtk::ScrolledWindow::builder()
        .child(&flow.widget)
        .vexpand(true)
        .build();
    window.set_content(Some(&scroll));
    until("image anchors allocated", || {
        flow.widget.width() > 0 && flow.objects.iter().all(|w| w.height() > 0)
    })
    .await?;
    glib::timeout_future(Duration::from_millis(100)).await;
    let mut seen = Vec::new();
    serve(&renderer, &mut receiver, &mut seen, 3).await?;
    if seen.iter().any(|source| matches!(source, Source::File {path,..} if path.ends_with("header.svg") || path.ends_with("cell.png"))) {
        return Err("Offscreen table image was decoded eagerly".into());
    }
    let snapshot = Source::Document {
        reader: "flow-images".into(),
        revision: "a".repeat(64),
        page: 7,
        image: 0,
        original: false,
    };
    if !seen.contains(&snapshot)
        || !seen.iter().any(|s| matches!(s,Source::Embedded(image) if image.bytes.as_ref()==b"AAA"))
        || !seen.iter().any(|s| matches!(s,Source::File {root:base,path} if base==root.to_string_lossy().as_ref() && Path::new(path)==root.join("nested/sample.png")))
    { return Err("Image source context or decoded embedded bytes changed".into()); }
    until("native image textures", || {
        crate::images_ui::loaded_count(flow.widget.upcast_ref(), true) >= 2
    })
    .await?;
    let remote = flow.objects.iter().find(|w| {
        w.first_child()
            .and_downcast::<gtk::Label>()
            .is_some_and(|l| l.text() == "Remote")
    });
    if remote.is_none() {
        return Err("Unfetched remote image lost its alt text".into());
    }
    crate::window_capture::capture(window, &root.join("images-flow.png")).await?;

    crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    flow.widget.grab_focus();
    flow.widget.buffer().select_range(
        &flow.widget.buffer().start_iter(),
        &flow.widget.buffer().end_iter(),
    );
    crate::smoke_input::request(root, json!({"action":"key","key":"copy"})).await?;
    until("image-aware copy", || flow.copied.borrow().is_some()).await?;
    let expected = flow
        .document
        .select(0..flow.document.characters, flow::BUFFER_BYTES)?
        .text;
    if crate::smoke_input::request(root, json!({"action":"read"})).await?["text"] != expected
        || expected.matches("Local").count() != 1
        || expected.matches("Snapshot").count() != 1
    {
        return Err("Image copy duplicated or omitted alt text".into());
    }

    let buffer = flow.widget.buffer();
    buffer.place_cursor(&buffer.end_iter());
    flow.widget
        .scroll_to_iter(&mut buffer.end_iter(), 0.0, true, 0.0, 1.0);
    // TextView schedules cursor visibility after the key event. Wait for that
    // layout before placing the table at the bottom of the outer viewport.
    glib::timeout_future(Duration::from_millis(150)).await;
    scroll.vadjustment().set_value(scroll.vadjustment().upper());
    glib::timeout_future(Duration::from_millis(100)).await;
    let table = column_view(flow.widget.upcast_ref()).ok_or("Image table missing")?;
    let inner = table
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_downcast::<gtk::ScrolledWindow>()
        .ok_or("Table viewport missing")?;
    inner.vadjustment().set_value(inner.vadjustment().upper());
    glib::timeout_future(Duration::from_millis(100)).await;
    crate::window_capture::capture(window, &root.join("images-table-before-flow.png")).await?;
    serve(&renderer, &mut receiver, &mut seen, 4).await?;
    until("table image textures", || {
        crate::images_ui::loaded_count(flow.widget.upcast_ref(), true) >= 1
    })
    .await?;
    glib::timeout_future(Duration::from_millis(150)).await;
    scroll.vadjustment().set_value(scroll.vadjustment().upper());
    glib::timeout_future(Duration::from_millis(100)).await;
    let picture = loaded_picture(table.upcast_ref()).ok_or("Table picture missing")?;
    let bounds = picture
        .compute_bounds(&inner)
        .ok_or("No table picture bounds")?;
    // A bottommost scroll shows the image's caption. Center its pixels in the
    // bounded table viewport instead, as a user would when inspecting it.
    let target = inner.height() as f64 * 0.65;
    inner.vadjustment().set_value(
        inner.vadjustment().value() + bounds.y() as f64 + bounds.height() as f64 / 2.0 - target,
    );
    glib::timeout_future(Duration::from_millis(100)).await;
    let bounds = picture
        .compute_bounds(&inner)
        .ok_or("No visible table picture")?;
    let center = bounds.y() + bounds.height() / 2.0;
    if center <= 30.0 || center >= inner.height() as f32 || picture.paintable().is_none() {
        return Err("Table picture pixels are outside the visible body viewport".into());
    }
    let table_bounds = table.compute_bounds(window).ok_or("No table bounds")?;
    let anchor_bounds = flow
        .objects
        .last()
        .unwrap()
        .compute_bounds(window)
        .ok_or("No table anchor bounds")?;
    let geometry = json!({"table":{"y":table_bounds.y(),"height":table_bounds.height()},"anchor":{"y":anchor_bounds.y(),"height":anchor_bounds.height()},"outer":{"value":scroll.vadjustment().value(),"upper":scroll.vadjustment().upper(),"page":scroll.vadjustment().page_size()},"inner":{"value":inner.vadjustment().value(),"upper":inner.vadjustment().upper(),"page":inner.vadjustment().page_size()}});
    crate::window_capture::capture(window, &root.join("images-table-flow.png")).await?;
    table
        .activate_action("flow-header-0.read", None)
        .map_err(|e| e.to_string())?;
    until("header full-cell window", || cell_window().is_some()).await?;
    serve(&renderer, &mut receiver, &mut seen, 5).await?;
    let header_window = cell_window().unwrap();
    until("header full-cell image", || {
        crate::images_ui::loaded_count(header_window.upcast_ref(), true) > 0
    })
    .await?;
    if seen.len() != 5
        || !seen
            .iter()
            .any(|source| matches!(source,Source::File{path,..} if path.ends_with("header.svg")))
    {
        return Err("Header image source was lost or an unexpected source was requested".into());
    }
    crate::window_capture::capture(&header_window, &root.join("images-header-flow.png")).await?;
    crate::smoke_input::request(root, json!({"action":"key","key":"close-window"})).await?;
    until("header window closed", || !header_window.is_mapped()).await?;
    if crate::images_ui::loaded_count(header_window.upcast_ref(), false) != 0 {
        return Err("Closed full-cell header retained its texture".into());
    }
    window.set_content(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
    until("unmap releases image textures", || {
        crate::images_ui::loaded_count(flow.widget.upcast_ref(), false) == 0
    })
    .await?;

    window.set_content(Some(&scroll));
    until("image remap", || flow.widget.is_mapped()).await?;
    glib::timeout_future(Duration::from_millis(100)).await;
    let late = pending(&mut receiver)?;
    if late.is_empty() {
        return Err("No image reload after remap".into());
    }
    window.set_content(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
    for (id, _) in late {
        renderer.event(id, Ok(frame()));
    }
    glib::timeout_future(Duration::from_millis(50)).await;
    if crate::images_ui::loaded_count(flow.widget.upcast_ref(), false) != 0
        || !pending(&mut receiver)?.is_empty()
    {
        return Err("Closed image anchor accepted a late texture or retained queued work".into());
    }
    Ok(
        json!({"productionRenderer":true,"syntheticDecodeReplies":true,"sourceKinds":3,"uniqueSources":seen.len(),"offscreenTableDeferred":true,"twoDecodeSlots":true,"exactAltCopy":true,"snapshotRevisionRetained":true,"unmapReleasesTextures":true,"lateReplyIgnored":true,"geometry":geometry}),
    )
}
