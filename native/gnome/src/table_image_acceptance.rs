//! Private Wayland coverage for table thumbnails, headers and frozen originals.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Table image timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn nodes(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut out = vec![root.clone()];
    let mut child = root.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        out.extend(nodes(&w));
    }
    out
}
fn button(root: &gtk::Widget, name: &str) -> Option<gtk::Button> {
    nodes(root)
        .into_iter()
        .filter(|w| w.is_mapped())
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.widget_name() == name || b.tooltip_text().as_deref() == Some(name))
}
fn table(root: &gtk::Widget) -> Option<gtk::ColumnView> {
    nodes(root)
        .into_iter()
        .filter(|w| w.is_mapped())
        .find_map(|w| w.downcast::<gtk::ColumnView>().ok())
}
fn dialog() -> Option<gtk::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.is_mapped() && w.widget_name() == "studio-selection-window")
}
fn signature(root: &gtk::Widget) -> Option<u64> {
    nodes(root)
        .into_iter()
        .filter(|w| w.is_mapped())
        .filter_map(|w| w.downcast::<crate::scaled_picture::ScaledPicture>().ok())
        .find_map(|p| p.smoke_signature().and_then(|v| v["pixels"].as_u64()))
}
fn in_view(root: &gtk::Widget) -> bool {
    if !root.is_mapped() || root.width() <= 0 || root.height() <= 0 {
        return false;
    }
    let mut p = root.parent();
    while let Some(w) = p {
        p = w.parent();
        if let Some(v) = w.downcast_ref::<gtk::ScrolledWindow>() {
            if !root.compute_bounds(v).is_some_and(|b| {
                b.x() < v.width() as f32
                    && b.y() < v.height() as f32
                    && b.x() + b.width() > 0.0
                    && b.y() + b.height() > 0.0
            }) {
                return false;
            }
        }
    }
    true
}
fn cell(root: &gtk::Widget, text: &str, line: usize) -> Option<gtk::Widget> {
    let expected = format!("Source line {line}");
    nodes(root)
        .into_iter()
        .filter(|w| in_view(w))
        .filter_map(|w| w.downcast::<gtk::Label>().ok())
        .find(|l| {
            l.is_selectable()
                && l.text() == text
                && l.tooltip_text().as_deref() == Some(expected.as_str())
        })?
        .parent()
}
fn cell_signature(root: &gtk::Widget, text: &str, line: usize) -> Option<u64> {
    signature(&cell(root, text, line)?)
}
fn limit(root: &gtk::Widget, max: i32) -> bool {
    nodes(root)
        .into_iter()
        .filter(|w| w.is_mapped())
        .filter_map(|w| w.downcast::<crate::scaled_picture::ScaledPicture>().ok())
        .filter(|p| p.paintable().is_some())
        .all(|p| p.height() <= max)
}
fn scroll(view: &gtk::ColumnView) -> Option<gtk::ScrolledWindow> {
    let mut p = view.parent();
    while let Some(w) = p {
        p = w.parent();
        if let Ok(s) = w.downcast::<gtk::ScrolledWindow>() {
            return Some(s);
        }
    }
    None
}
fn caption(root: &gtk::Widget) -> Option<gtk::Label> {
    nodes(root)
        .into_iter()
        .filter(|w| w.is_mapped())
        .filter_map(|w| w.downcast::<gtk::Label>().ok())
        .find(|l| l.is_selectable() && l.text() == "Outer Inner tail")
}
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let ticks = Rc::new(std::cell::Cell::new(0u64));
    let count = ticks.clone();
    let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
        count.set(count.get() + 1);
        glib::ControlFlow::Continue
    });
    struct Timer(Option<glib::SourceId>);
    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(t) = self.0.take() {
                t.remove();
            }
        }
    }
    let _timer = Timer(Some(timer));
    until("chat table images", || {
        ui.state.borrow().history_verified
            && table(ui.transcript_view.upcast_ref()).is_some()
            && signature(ui.transcript_view.upcast_ref()).is_some()
    })
    .await?;
    let chat_ms = started.elapsed().as_millis();
    ui.composer
        .buffer()
        .set_text("Unsent table image draft 中文😀");
    smoke::capture(&ui.window, &root.join("chat-table-images.png"));
    if !limit(ui.transcript_view.upcast_ref(), 120) {
        return Err("Chat thumbnails exceeded the cell height bound".into());
    }
    button(ui.transcript_view.upcast_ref(), "studio-cell-open")
        .ok_or("Chat full cell missing")?
        .emit_clicked();
    until("chat full-cell images", || {
        dialog().is_some_and(|w| signature(w.upcast_ref()).is_some())
    })
    .await?;
    let full = dialog().unwrap();
    if !limit(full.upcast_ref(), 320) {
        return Err("Full-cell images exceeded their height bound".into());
    }
    smoke::capture(&full, &root.join("chat-full-cell.png"));
    button(full.upcast_ref(), "Open image viewer")
        .ok_or("Chat cell image action missing")?
        .emit_clicked();
    until("chat original viewer", || {
        !full.is_mapped() && ui.image_viewer.smoke_ready()
    })
    .await?;
    if crate::images_ui::loaded_count(full.upcast_ref(), false) != 0 {
        return Err("Closed cell retained image pixels".into());
    }
    ui.image_viewer
        .smoke_file_original_save(
            root,
            "saved-chat.png",
            &root.join("project/outer.png"),
            (128, 512),
        )
        .await?;
    ui.image_viewer.close();

    let reader_at = std::time::Instant::now();
    ui.reader.open_path(&root.join("project/table.md"));
    ui.show_tool("document");
    until("reader table images", || {
        ui.reader.is_ready()
            && table(ui.reader.widget.upcast_ref()).is_some()
            && cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3).is_some()
    })
    .await?;
    let reader_ms = reader_at.elapsed().as_millis();
    let view = table(ui.reader.widget.upcast_ref()).unwrap();
    let first = cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3).unwrap();
    if !limit(ui.reader.widget.upcast_ref(), 120) {
        return Err("Reader thumbnails exceeded the cell height bound".into());
    }
    caption(ui.reader.widget.upcast_ref())
        .ok_or("Reader caption missing")?
        .select_region(0, -1);
    ui.reader.smoke_inline_comment("Outer Inner tail", 3)?;
    smoke::capture(&ui.window, &root.join("reader-table-images.png"));
    view.activate_action("cell-header-0.read", None)
        .map_err(|e| e.to_string())?;
    until("header images", || {
        dialog().is_some_and(|w| signature(w.upcast_ref()).is_some())
    })
    .await?;
    let full = dialog().unwrap();
    button(full.upcast_ref(), "Open image viewer")
        .ok_or("Header image action missing")?
        .emit_clicked();
    until("header original viewer", || {
        !full.is_mapped() && ui.image_viewer.smoke_ready()
    })
    .await?;
    ui.image_viewer
        .smoke_embedded_save(root, "saved-header.png", (128, 128))
        .await?;
    ui.image_viewer.close();
    ui.show_tool("document");

    until("reader thumbnails remap", || {
        cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3) == Some(first)
    })
    .await?;
    view.scroll_to(2, None, gtk::ListScrollFlags::NONE, None);
    glib::timeout_future(Duration::from_millis(100)).await;
    smoke::capture(&ui.window, &root.join("reader-svg-loading.png"));
    until("SVG cell pixels", || {
        cell_signature(ui.reader.widget.upcast_ref(), "SVG caption 中文", 5).is_some()
    })
    .await?;
    let svg_cell =
        cell(ui.reader.widget.upcast_ref(), "SVG caption 中文", 5).ok_or("SVG cell missing")?;
    button(&svg_cell, "studio-cell-open")
        .ok_or("SVG full cell missing")?
        .emit_clicked();
    until("SVG full-cell pixels", || {
        dialog().is_some_and(|w| signature(w.upcast_ref()).is_some())
    })
    .await?;
    let full = dialog().unwrap();
    smoke::capture(&full, &root.join("reader-svg-cell.png"));
    button(full.upcast_ref(), "Open image viewer")
        .ok_or("SVG image action missing")?
        .emit_clicked();
    until("SVG original viewer", || {
        !full.is_mapped() && ui.image_viewer.smoke_ready()
    })
    .await?;
    ui.image_viewer
        .smoke_embedded_save(root, "saved-svg.svg", (2048, 1366))
        .await?;
    ui.image_viewer.close();
    ui.show_tool("document");
    let viewport = scroll(&view).ok_or("Table viewport missing")?;
    let adjustment = viewport.vadjustment();
    adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
    until("newly visible bottom image", || {
        cell_signature(ui.reader.widget.upcast_ref(), "Bottom Inner tail", 52)
            .is_some_and(|s| s != first)
    })
    .await?;
    if crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), true) > 16 {
        return Err("Offscreen table pixels remained loaded".into());
    }
    adjustment.set_value(0.0);
    until("top image restoration", || {
        cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3) == Some(first)
    })
    .await?;
    button(
        &cell(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3)
            .ok_or("Visible first cell missing")?,
        "studio-cell-open",
    )
    .ok_or("Reader full cell missing")?
    .emit_clicked();
    if let Err(error) = until("reader full-cell images", || {
        dialog().is_some_and(|w| signature(w.upcast_ref()).is_some())
    })
    .await
    {
        smoke::capture(&ui.window, &root.join("reader-full-cell-failed.png"));
        let details = dialog().map(|w| {
            smoke::capture(&w, &root.join("reader-cell-failed.png"));
            nodes(w.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>().map(|l| l.text().to_string()))
                .take(12)
                .collect::<Vec<_>>()
        });
        return Err(format!(
            "{error}: dialog={details:?}; {}",
            ui.reader.smoke_comment_debug()
        ));
    }
    let full = dialog().unwrap();
    let stale =
        button(full.upcast_ref(), "Open image viewer").ok_or("Reader image action missing")?;
    stale.emit_clicked();
    until("reader original viewer", || {
        !full.is_mapped() && ui.image_viewer.smoke_ready()
    })
    .await?;
    // The image viewer owns a frozen original even after its source document closes.
    ui.reader.smoke_close();
    ui.image_viewer
        .smoke_embedded_save(root, "saved-reader.png", (128, 512))
        .await?;
    ui.image_viewer.close();
    ui.reader.open_path(&root.join("project/table.md"));
    ui.show_tool("document");
    until("reader reopened", || {
        ui.reader.is_ready()
            && cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3).is_some()
    })
    .await?;
    stale.emit_clicked();
    glib::timeout_future(Duration::from_millis(80)).await;
    if ui.image_viewer.widget.is_mapped() {
        return Err("Unmapped old cell reopened its image".into());
    }
    let page_at = std::time::Instant::now();
    button(ui.reader.widget.upcast_ref(), "Next table page")
        .ok_or("Next table page missing")?
        .emit_clicked();
    // Owned headless surfaces can be covered and receive no compositor tick.
    // Drive the same initial layout as the existing dialog capture fixtures.
    glib::timeout_future(Duration::from_millis(100)).await;
    smoke::capture(&ui.window, &root.join("reader-page-loading.png"));
    if let Err(error) = until("next table page", || {
        nodes(ui.reader.widget.upcast_ref()).iter().any(|w| {
            w.downcast_ref::<gtk::Label>()
                .is_some_and(|l| l.is_mapped() && l.text() == "Row 50")
        }) && cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 53).is_some()
    })
    .await
    {
        smoke::capture(&ui.window, &root.join("reader-page-failed.png"));
        let labels = nodes(ui.reader.widget.upcast_ref())
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .filter(|l| l.is_mapped() && in_view(l.upcast_ref()))
            .map(|l| {
                (
                    l.text().to_string(),
                    l.tooltip_text().map(|t| t.to_string()),
                )
            })
            .take(20)
            .collect::<Vec<_>>();
        return Err(format!(
            "{error}; {labels:?}; {}",
            ui.reader.smoke_comment_debug()
        ));
    }
    nodes(
        &cell(ui.reader.widget.upcast_ref(), "Outer Inner tail", 53)
            .ok_or("Visible paged cell missing")?,
    )
    .into_iter()
    .filter_map(|w| w.downcast::<gtk::Label>().ok())
    .find(|l| l.is_selectable() && l.text() == "Outer Inner tail")
    .ok_or("Paged caption missing")?
    .select_region(0, -1);
    ui.reader.smoke_inline_comment("Outer Inner tail", 53)?;
    let page_ms = page_at.elapsed().as_millis();
    smoke::capture(&ui.window, &root.join("reader-next-page.png"));
    ui.reader.show_source();
    until("source releases images", || {
        ui.reader.source_mode()
            && ui.reader.is_ready()
            && crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), false) == 0
    })
    .await?;
    ui.reader.show_preview();
    glib::timeout_future(Duration::from_millis(100)).await;
    smoke::capture(&ui.window, &root.join("reader-preview-loading.png"));
    until("preview restores images", || {
        !ui.reader.source_mode()
            && cell_signature(ui.reader.widget.upcast_ref(), "Outer Inner tail", 3).is_some()
    })
    .await?;
    if ui.composer.buffer().text(
        &ui.composer.buffer().start_iter(),
        &ui.composer.buffer().end_iter(),
        false,
    ) != "Unsent table image draft 中文😀"
    {
        return Err("Table images lost composer draft".into());
    }
    Ok(
        json!({"ok":true,"chatAndReader":true,"headerAction":true,"svgCells":true,"boundedThumbnails":true,"fullCellImages":true,"modalViewerHandoff":true,"viewportAdmission":true,"paging":true,"sourceCommentLines":true,"sourcePreviewCleanup":true,"staleActionRejected":true,"originalSurvivesReaderClose":true,"mainLoopTicks":ticks.get(),"chatReadyMs":chat_ms,"readerReadyMs":reader_ms,"pageReadyIncludingCaptureMs":page_ms,"maximumImageCallbackMs":ui.images.maximum_callback_ms.get(),"maximumVisibilityPassMs":ui.images.maximum_visibility_ms.get()}),
    )
}
