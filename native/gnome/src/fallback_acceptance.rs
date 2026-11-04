//! Real native source paging after a Markdown formatting-budget failure.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(25) {
            return Err(format!("Literal fallback timed out: {step}"));
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
fn view(root: &gtk::Widget) -> Option<gtk::TextView> {
    find(root, &|w| w.widget_name() == "studio-rich-text").and_downcast()
}
fn source_caption(root: &gtk::Widget) -> Option<String> {
    find(root, &|w| {
        w.downcast_ref::<gtk::Label>()
            .is_some_and(|l| l.text().starts_with("Source · lines "))
    })
    .and_downcast::<gtk::Label>()
    .map(|l| l.text().to_string())
}
async fn check(root: &gtk::Widget) -> Result<Value, String> {
    let view = view(root).ok_or("Literal native text view missing")?;
    let b = view.buffer();
    let text = || b.text(&b.start_iter(), &b.end_iter(), false).to_string();
    if view.is_editable()
        || !view.is_monospace()
        || b.enables_undo()
        || text().len() > studio_markdown_native::inline::PAGE_BYTES
    {
        return Err("Literal source is not bounded/read-only/monospace".into());
    }
    if !text().contains("literal-begin **raw 中文😀**")
        || !text().contains("<b>literal</b>")
        || !text().contains("javascript:alert")
    {
        return Err("Literal markup or source text changed".into());
    }
    let (a, _) = b
        .start_iter()
        .forward_search("raw 中文😀", gtk::TextSearchFlags::TEXT_ONLY, None)
        .ok_or("Unicode source missing")?;
    if a.tags().iter().any(|t| {
        matches!(
            t.name().as_deref(),
            Some("studio-bold" | "studio-italic" | "studio-link")
        )
    }) {
        return Err("Literal text acquired Markdown formatting or active links".into());
    }
    let first = source_caption(root).ok_or("Source line caption missing")?;
    if !first.starts_with("Source · lines 1–") {
        return Err(format!("First literal source lines are wrong: {first}"));
    }
    view.activate_action("rich.last-page", None)
        .map_err(|e| e.to_string())?;
    until("full literal tail", || text().contains("literal-tail ")).await?;
    let (a, z) = b
        .start_iter()
        .forward_search("literal-tail ", gtk::TextSearchFlags::TEXT_ONLY, None)
        .unwrap();
    b.select_range(&a, &z);
    view.scroll_to_mark(&b.get_insert(), 0.0, true, 0.0, 0.5);
    until("selected source tail on screen", || {
        let glyph = view.iter_location(&a);
        let visible = view.visible_rect();
        glyph.height() > 0
            && glyph.y() >= visible.y()
            && glyph.y() + glyph.height() <= visible.y() + visible.height()
    })
    .await?;
    if crate::content_widgets::selection(view.upcast_ref(), 16384).as_deref()
        != Some("literal-tail ")
        || text().len() > studio_markdown_native::inline::PAGE_BYTES
    {
        return Err("Literal tail selection lost text or its trailing space".into());
    }
    let last = source_caption(root).unwrap();
    if !last.contains("–3 ·") {
        return Err(format!(
            "Last literal physical source line is wrong: {last}"
        ));
    }
    Ok(
        json!({"boundedPages":true,"literalMarkup":true,"noActiveLinks":true,"unicodeAndTrailingSpace":true,"visibleTail":true,"readOnly":true,"firstCaption":first,"lastCaption":last}),
    )
}
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    let at = std::time::Instant::now();
    let ticks = Rc::new(std::cell::Cell::new(0u64));
    let gap = Rc::new(std::cell::Cell::new(0u128));
    let last = Rc::new(std::cell::Cell::new(at));
    let (t, g, l) = (ticks.clone(), gap.clone(), last.clone());
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
            if let Some(t) = self.0.take() {
                t.remove();
            }
        }
    }
    let _timer = Timer(Some(timer));
    until("fallback history mapped", || {
        ui.state.borrow().history_verified && view(ui.transcript_view.upcast_ref()).is_some()
    })
    .await?;
    let chat_ms = at.elapsed().as_millis();
    let item = ui
        .state
        .borrow()
        .items
        .iter()
        .find(|i| i.id == "literal-assistant")
        .cloned()
        .ok_or("Literal history item missing")?;
    let literal = item.rich.as_ref().is_some_and(|content| {
        if content.rows.len() != 1 {
            return false;
        }
        matches!(
            &content.rows[0].block.kind,
            studio_markdown_native::BlockKind::Paragraph(text)
                if text.source_only
                    && text.attributes.as_ref().is_some_and(|a| a.links.is_empty())
        )
    });
    if item.markup.is_some() || !literal {
        return Err("History did not use the shared literal presentation".into());
    }
    let chat = check(ui.transcript_view.upcast_ref()).await?;
    let items = ui.state.borrow().items.clone();
    let hits = gio::spawn_blocking(move || {
        crate::session_search::find(
            &items,
            "literal-tail ",
            true,
            crate::session_search::Filter::All,
        )
    })
    .await
    .map_err(|_| "Fallback search worker stopped")??;
    let hit = hits.first().ok_or("Literal search missed source text")?;
    if hit.paragraph_page.is_none_or(|p| p == 0) {
        return Err("Literal search did not project its exact source page".into());
    }
    let row = crate::chat_ui::locate(&ui.transcript, hit).ok_or("Literal search row missing")?;
    ui.transcript_view
        .scroll_to(row, gtk::ListScrollFlags::NONE, None);
    until("literal search selection", || {
        crate::chat_ui::target_visible(&ui.transcript, hit)
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent fallback draft 中文😀 ");
    let at = std::time::Instant::now();
    ui.reader.open_path(&root.join("project/report.md"));
    ui.show_tool("document");
    until("reader fallback mapped", || {
        ui.reader.is_ready() && view(ui.reader.widget.upcast_ref()).is_some()
    })
    .await?;
    let reader_ms = at.elapsed().as_millis();
    super::smoke::capture(&ui.window, &root.join("fallback-reader-first.png"));
    let reader = check(ui.reader.widget.upcast_ref()).await?;
    super::smoke::capture(&ui.window, &root.join("fallback-reader-last.png"));
    ui.reader.show_source();
    until("exact Source view remains available", || {
        ui.reader.source_mode() && ui.reader.is_ready()
    })
    .await?;
    let source = find(ui.reader.widget.upcast_ref(), &|w| {
        w.is::<sourceview::View>()
    })
    .ok_or("Original Source widget missing")?
    .downcast::<sourceview::View>()
    .unwrap();
    let b = source.buffer();
    if !b
        .text(&b.start_iter(), &b.end_iter(), false)
        .contains("literal-begin **raw 中文😀**")
    {
        return Err("Original Markdown Source changed".into());
    }
    ui.reader.show_preview();
    until("source-only Preview returns", || {
        ui.reader.is_ready() && view(ui.reader.widget.upcast_ref()).is_some()
    })
    .await?;
    if ui.composer_text() != "Unsent fallback draft 中文😀 " {
        return Err("Reading fallback changed the draft".into());
    }
    Ok(
        json!({"ok":true,"chat":chat,"reader":reader,"exactSearchPage":true,"sourcePreviewSwitch":true,"draftRetained":true,"chatMappedMs":chat_ms,"readerMappedMs":reader_ms,"mainLoopTicks":ticks.get(),"maxHeartbeatGapMs":gap.get()}),
    )
}
