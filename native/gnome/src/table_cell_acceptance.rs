//! Real native table excerpts and full-cell windows, using private fixtures only.
use super::*;
use std::cell::Cell;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let started = std::time::Instant::now();
    while !test() {
        if started.elapsed() > Duration::from_secs(30) {
            return Err(format!("Table cell acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn find(root: &gtk::Widget, predicate: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    if root.is_mapped() && predicate(root) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(found) = find(&widget, predicate) {
            return Some(found);
        }
    }
    None
}
fn open_button(root: &gtk::Widget) -> Option<gtk::Button> {
    find(root, &|widget| widget.widget_name() == "studio-cell-open").and_downcast()
}
fn full_window() -> Option<gtk::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("Table cell") && w.is_mapped())
}
fn capture(window: &gtk::Window, path: &std::path::Path) -> Result<(), String> {
    let snapshot = gtk::Snapshot::new();
    snapshot.append_color(
        &gtk::gdk::RGBA::WHITE,
        &gtk::graphene::Rect::new(0.0, 0.0, window.width() as f32, window.height() as f32),
    );
    gtk::WidgetPaintable::new(Some(window)).snapshot(
        &snapshot,
        window.width() as f64,
        window.height() as f64,
    );
    let node = snapshot
        .to_node()
        .ok_or("Mapped cell window has no paint node")?;
    window
        .renderer()
        .ok_or("Mapped cell window has no renderer")?
        .render_texture(&node, None)
        .save_to_png(path)
        .map_err(|e| e.to_string())
}
async fn save_selection(
    ui: &Rc<Ui>,
    widget: &gtk::Widget,
    quote: &str,
    owner: &str,
    line: usize,
) -> Result<Value, String> {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        let text = label.text();
        let offset = text.find(quote).ok_or("Cell quote missing")?;
        let start = text[..offset].chars().count() as i32;
        label.select_region(start, start + quote.chars().count() as i32);
    } else {
        let view = widget
            .downcast_ref::<gtk::TextView>()
            .ok_or("Cell text widget missing")?;
        let b = view.buffer();
        let (a, z) = b
            .start_iter()
            .forward_search(quote, gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Cell quote missing")?;
        b.select_range(&a, &z);
    }
    if crate::content_widgets::selection(widget, 16384).as_deref() != Some(quote) {
        return Err(format!(
            "Fixture selection failed for {owner}: {:?}",
            crate::content_widgets::selection(widget, 16384)
        ));
    }
    let expected = if owner == "reader" {
        let result = Rc::new(RefCell::new(None));
        let captured = result.clone();
        let old = ui.reader.on_comment.replace(Some(Box::new(move |capture| {
            captured.replace(Some(capture));
        })));
        let activated = widget.activate_action("reader-comment.capture", None);
        ui.reader.on_comment.replace(old);
        activated.map_err(|e| e.to_string())?;
        let capture = result
            .borrow_mut()
            .take()
            .ok_or("Reader context capture missing")?;
        if capture.excerpt != quote
            || capture.source["provider"] != "document"
            || capture.source["anchor"]["startLine"] != line
            || capture.source["anchor"]["baseHash"]
                .as_str()
                .is_none_or(str::is_empty)
            || capture.source["anchor"]["filePath"]
                .as_str()
                .is_none_or(|p| !p.ends_with("report.md") && !p.ends_with("code.md"))
        {
            return Err(format!(
                "Reader quote/source mismatch: {:?}",
                capture.source
            ));
        }
        if let Some(callback) = ui.reader.on_comment.borrow().as_ref() {
            callback(capture.clone());
        }
        capture
    } else {
        let state = ui.state.borrow();
        let item = state
            .items
            .iter()
            .find(|i| i.id == "rich-assistant")
            .ok_or("Chat cell owner missing")?;
        let key = format!(
            "{}:{}",
            state.backend,
            state.selected.as_ref().ok_or("Session missing")?.id
        );
        let expected = crate::annotations::chat_capture(item, &key, Some(quote.into()))?;
        drop(state);
        widget
            .activate_action("chat-comment.capture", None)
            .map_err(|e| e.to_string())?;
        expected
    };
    until("cell comments dialog", || ui.annotations.borrow().is_some()).await?;
    let panel = ui.annotations.borrow().clone().unwrap();
    until("cell comments mapped", || panel.dialog.is_mapped()).await?;
    let saved = panel.smoke_save_source_capture(&expected).await?;
    panel.dialog.close();
    until("cell comments closed", || ui.annotations.borrow().is_none()).await?;
    Ok(saved)
}
async fn check(
    ui: &Rc<Ui>,
    root: &gtk::Widget,
    directory: &std::path::Path,
    name: &str,
) -> Result<Value, String> {
    let next = find(root, &|w| {
        w.downcast_ref::<gtk::Button>()
            .is_some_and(|b| b.tooltip_text().as_deref() == Some("Next table page"))
    })
    .ok_or("Table next action missing")?
    .downcast::<gtk::Button>()
    .unwrap();
    next.emit_clicked();
    until("long cell leaves table page", || {
        open_button(root).is_none()
    })
    .await?;
    let previous = find(root, &|w| {
        w.downcast_ref::<gtk::Button>()
            .is_some_and(|b| b.tooltip_text().as_deref() == Some("Previous table page"))
    })
    .ok_or("Table previous action missing")?
    .downcast::<gtk::Button>()
    .unwrap();
    until("previous table page ready", || previous.is_sensitive()).await?;
    previous.emit_clicked();
    until("long cell returns to table page", || {
        open_button(root).is_some()
    })
    .await?;
    let excerpt = find(root, &|w| w.widget_name() == "studio-cell-excerpt")
        .ok_or("Cell excerpt missing")?
        .downcast::<gtk::Label>()
        .unwrap();
    if excerpt.text().len() > studio_markdown_native::CELL_PREVIEW_BYTES + 3
        || !excerpt.text().starts_with("cell-bold 中文😀")
        || excerpt.text().contains("tail-cell")
    {
        return Err("Table excerpt is unbounded or lost Unicode".into());
    }
    let quote = excerpt.text().to_string();
    if quote.ends_with('…') {
        return Err("Synthetic ellipsis entered selectable cell text".into());
    }
    let excerpt_record = save_selection(ui, excerpt.upcast_ref(), &quote, name, 3).await?;
    let started = std::time::Instant::now();
    open_button(root)
        .ok_or("Full cell action missing")?
        .emit_clicked();
    until("full cell window", || full_window().is_some()).await?;
    let window = full_window().unwrap();
    until("styled cell page", || {
        window.width() >= 500
            && window.height() >= 350
            && find(window.upcast_ref(), &|w| {
                w.downcast_ref::<gtk::TextView>().is_some_and(|v| {
                    v.width() > 100
                        && v.height() > 100
                        && v.iter_location(&v.buffer().start_iter()).height() > 0
                })
            })
            .is_some()
    })
    .await?;
    let view = find(window.upcast_ref(), &|w| w.is::<gtk::TextView>())
        .unwrap()
        .downcast::<gtk::TextView>()
        .unwrap();
    let opened_ms = started.elapsed().as_millis();
    crate::table_cell::smoke_find(&window, "tail-cell", "tail-cell").await?;
    view.activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    let buffer = view.buffer();
    let text = || {
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    };
    if view.is_editable()
        || buffer.enables_undo()
        || text().len() > studio_markdown_native::inline::PAGE_BYTES
    {
        return Err("Full cell is not a bounded read-only page".into());
    }
    for (needle, tag) in [
        ("cell-bold 中文😀", "studio-bold"),
        ("italic", "studio-italic"),
        ("strike", "studio-strike"),
        ("code", "studio-inline-code"),
        ("linked", "studio-link"),
    ] {
        let (a, _) = buffer
            .start_iter()
            .forward_search(needle, gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Cell style text missing")?;
        if !a.tags().iter().any(|t| t.name().as_deref() == Some(tag)) {
            return Err(format!("Missing cell style: {tag}"));
        }
    }
    if !text().contains("<b>literal</b>") {
        return Err("Cell HTML was interpreted".into());
    }
    let (a, b) = buffer
        .start_iter()
        .forward_search("cell-bold 中文😀", gtk::TextSearchFlags::TEXT_ONLY, None)
        .unwrap();
    buffer.select_range(&a, &b);
    if crate::content_widgets::selection(view.upcast_ref(), 16384).as_deref()
        != Some("cell-bold 中文😀")
    {
        return Err("Full cell selection lost Unicode".into());
    }
    glib::timeout_future(Duration::from_millis(80)).await;
    capture(&window, &directory.join(format!("{name}-cell-first.png")))?;
    view.activate_action("rich.last-page", None)
        .map_err(|e| e.to_string())?;
    if !text().contains("tail-cell") || text().len() > studio_markdown_native::inline::PAGE_BYTES {
        return Err("Full cell last page lost content".into());
    }
    let (a, b) = buffer
        .start_iter()
        .forward_search("tail-cell", gtk::TextSearchFlags::TEXT_ONLY, None)
        .unwrap();
    buffer.select_range(&a, &b);
    if crate::content_widgets::selection(view.upcast_ref(), 16384).as_deref() != Some("tail-cell") {
        return Err("Cell tail selection failed".into());
    }
    view.scroll_to_mark(&buffer.get_insert(), 0.0, true, 0.0, 0.5);
    until("cell tail is visible", || {
        let glyph = view.iter_location(&buffer.iter_at_mark(&buffer.get_insert()));
        let visible = view.visible_rect();
        glyph.height() > 0
            && glyph.y() >= visible.y()
            && glyph.y() + glyph.height() <= visible.y() + visible.height()
    })
    .await?;
    glib::timeout_future(Duration::from_millis(80)).await;
    capture(&window, &directory.join(format!("{name}-cell-last.png")))?;
    view.activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    if !text().starts_with("cell-bold 中文😀") {
        return Err("Cell first page did not restore".into());
    }
    window.close();
    until("cell window closes", || full_window().is_none()).await?;
    if !excerpt.is_mapped() {
        return Err("Reading a cell lost the parent table".into());
    }
    open_button(root)
        .ok_or("Cell action lost after closing")?
        .emit_clicked();
    until("reopened cell", || full_window().is_some()).await?;
    let owner = excerpt.parent().ok_or("Cell owner missing")?;
    owner.set_visible(false);
    until("unmapped source closes detached cell", || {
        full_window().is_none()
    })
    .await?;
    owner.set_visible(true);
    until("cell remapped", || open_button(root).is_some()).await?;
    let stale_button = open_button(root).unwrap();
    stale_button.emit_clicked();
    until("cell comments window", || full_window().is_some()).await?;
    let window = full_window().unwrap();
    until("cell comments text", || {
        find(window.upcast_ref(), &|w| w.is::<gtk::TextView>()).is_some()
    })
    .await?;
    let leaf = find(window.upcast_ref(), &|w| w.is::<gtk::TextView>()).unwrap();
    let saved = save_selection(ui, &leaf, "cell-bold 中文😀", name, 3).await?;
    if full_window().is_some() {
        return Err("Cell modal blocked Comments".into());
    }
    // A retained, detached widget must not reopen or capture a recycled row.
    next.emit_clicked();
    until("cell owner paged out", || !stale_button.is_mapped()).await?;
    stale_button.emit_clicked();
    leaf.activate_action(
        &format!(
            "{}-comment.capture",
            if name == "reader" { "reader" } else { "chat" }
        ),
        None,
    )
    .map_err(|e| e.to_string())?;
    if full_window().is_some() || ui.annotations.borrow().is_some() {
        return Err("Detached cell action accepted".into());
    }
    until("previous page enabled", || previous.is_sensitive()).await?;
    previous.emit_clicked();
    until("cell page restored", || open_button(root).is_some()).await?;
    Ok(
        json!({"cellFind":true,"boundedExcerpt":true,"styledPages":true,"unicodeSelection":true,"lastPage":true,"visibleTail":true,"ownerLifetime":true,"tablePaging":true,"readOnly":true,"sourceLineTooltip":excerpt.tooltip_text().map(|v|v.to_string()),"openMs":opened_ms,"comment":saved,"excerptComment":excerpt_record,"boundedAndFullComments":true,"staleSelectionRejected":true}),
    )
}
pub async fn run(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let ticks = Rc::new(Cell::new(0u64));
    let maximum = Rc::new(Cell::new(0u128));
    let last = Rc::new(Cell::new(started));
    let (t, g, l) = (ticks.clone(), maximum.clone(), last.clone());
    let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
        t.set(t.get() + 1);
        g.set(
            g.get()
                .max(l.replace(std::time::Instant::now()).elapsed().as_millis()),
        );
        glib::ControlFlow::Continue
    });
    struct Timer(Option<glib::SourceId>);
    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(id) = self.0.take() {
                id.remove();
            }
        }
    }
    let _timer = Timer(Some(timer));
    until("chat table", || {
        ui.state.borrow().history_verified && open_button(ui.transcript_view.upcast_ref()).is_some()
    })
    .await?;
    let short = gio::spawn_blocking(|| {
        let blocks = studio_markdown_native::parse_text(
            "| Header |\n|---|\n| short **styled 中文😀** tail |\n",
        )?;
        let studio_markdown_native::BlockKind::Table { rows, .. } = &blocks[0].kind else {
            return Err("Short-cell Find fixture has no table".to_string());
        };
        Ok(rows[0].cells[0].clone())
    })
    .await
    .map_err(|_| "Short-cell fixture worker stopped".to_string())??;
    let short_reader = crate::table_cell::ReadCell::new(
        &short,
        ui.transcript_view.upcast_ref(),
        Rc::new(|_| {}),
        None,
        None,
    );
    short_reader.present();
    until("short full-cell reader", || full_window().is_some()).await?;
    crate::table_cell::smoke_find(&full_window().unwrap(), "styled 中文😀", "styled 中文😀")
        .await?;
    short_reader.close();
    until("short cell closes", || full_window().is_none()).await?;
    let chat_open_ms = started.elapsed().as_millis();
    let chat = check(ui, ui.transcript_view.upcast_ref(), directory, "chat").await?;
    ui.composer
        .buffer()
        .set_text("Unsent table-cell draft 中文😀 ");
    let started = std::time::Instant::now();
    ui.reader.open_path(&directory.join("project/report.md"));
    ui.show_tool("document");
    until("reader table", || {
        ui.reader.is_ready() && open_button(ui.reader.widget.upcast_ref()).is_some()
    })
    .await?;
    let reader_open_ms = started.elapsed().as_millis();
    super::smoke::capture(&ui.window, &directory.join("reader-table-cells.png"));
    let reader = check(ui, ui.reader.widget.upcast_ref(), directory, "reader").await?;
    open_button(ui.reader.widget.upcast_ref())
        .ok_or("Reader cell action missing")?
        .emit_clicked();
    until("cell link window", || full_window().is_some()).await?;
    let window = full_window().unwrap();
    until("cell link view", || {
        find(window.upcast_ref(), &|w| w.is::<gtk::TextView>()).is_some()
    })
    .await?;
    let view = find(window.upcast_ref(), &|w| w.is::<gtk::TextView>())
        .unwrap()
        .downcast::<gtk::TextView>()
        .unwrap();
    let buffer = view.buffer();
    let (a, _) = buffer
        .start_iter()
        .forward_search("linked", gtk::TextSearchFlags::TEXT_ONLY, None)
        .ok_or("Cell link missing")?;
    buffer.place_cursor(&a);
    view.activate_action("rich.open-link", None)
        .map_err(|e| e.to_string())?;
    until("cell link changes file and closes old cell", || {
        ui.reader.is_ready()
            && ui
                .reader
                .location()
                .is_some_and(|(_, p)| p.ends_with("nested/note.md"))
            && full_window().is_none()
    })
    .await?;
    ui.reader.open_path(&directory.join("project/code.md"));
    until("reader code block", || {
        ui.reader.is_ready()
            && find(ui.reader.widget.upcast_ref(), &|w| {
                w.downcast_ref::<gtk::TextView>().is_some_and(|v| {
                    let b = v.buffer();
                    b.text(&b.start_iter(), &b.end_iter(), false)
                        .contains("native_code")
                })
            })
            .is_some()
    })
    .await?;
    let code = find(ui.reader.widget.upcast_ref(), &|w| {
        w.downcast_ref::<gtk::TextView>().is_some_and(|v| {
            let b = v.buffer();
            b.text(&b.start_iter(), &b.end_iter(), false)
                .contains("native_code")
        })
    })
    .unwrap();
    let code_record = save_selection(ui, &code, "native_code 中文😀", "reader", 1).await?;
    ui.reader.smoke_saved_selection_revision(&code).await?;
    ui.reader
        .open_path(&directory.join("project/nested/note.md"));
    until("new file after code", || {
        ui.reader.is_ready() && !code.is_mapped()
    })
    .await?;
    code.activate_action("reader-comment.capture", None)
        .map_err(|e| e.to_string())?;
    if ui.annotations.borrow().is_some() {
        return Err("Previous-file code action accepted".into());
    }
    for (record, line) in [
        (&reader["excerptComment"], 3),
        (&reader["comment"], 3),
        (&code_record, 1),
    ] {
        ui.comment_panel(None);
        let panel = ui
            .annotations
            .borrow()
            .clone()
            .ok_or("Reopen library missing")?;
        let id = record["id"].as_str().ok_or("Source record ID missing")?;
        until("saved comment ready to reopen", || {
            panel.smoke_open_record(id).unwrap_or(false)
        })
        .await?;
        until("source quote verified", || {
            ui.annotations.borrow().is_none() && ui.reader.is_ready()
        })
        .await?;
        // These fixtures have no heading; verified quotes intentionally open
        // Source, matching the reader's existing heading navigation policy.
        if ui.reader.smoke_comment_position() != (line, false, true) {
            return Err(format!(
                "Cell/code comment failed quote verification: {}",
                ui.reader.smoke_comment_debug()
            ));
        }
    }
    if !ui
        .composer_text()
        .contains("Unsent table-cell draft 中文😀 ")
    {
        return Err("Reading a cell changed the composer".into());
    }
    Ok(
        json!({"ok":true,"chat":chat,"reader":reader,"chatOpenMs":chat_open_ms,"readerOpenMs":reader_open_ms,"mainLoopTicks":ticks.get(),"maxHeartbeatGapMs":maximum.get(),"draftRetained":true,"relativeLink":true,"staleWindowClosed":true,"codeComment":code_record,"staleDocumentRejected":true,"verifiedSourceReopen":true,"savedRevisionRejected":true}),
    )
}
