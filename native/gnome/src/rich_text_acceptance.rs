//! Realized long-paragraph widgets in the actual private Wayland application.
use super::*;
use std::cell::Cell;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(40) {
            return Err(format!("Rich text acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn paragraph(root: &gtk::Widget) -> Option<gtk::Widget> {
    if root.is_mapped() && root.width() > 0 && root.height() > 0 {
        let text = if let Some(label) = root.downcast_ref::<gtk::Label>() {
            Some(label.text().to_string())
        } else {
            root.downcast_ref::<gtk::TextView>()
                .filter(|view| view.iter_location(&view.buffer().start_iter()).height() > 0)
                .map(|view| {
                    let buffer = view.buffer();
                    buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), false)
                        .to_string()
                })
        };
        if text.is_some_and(|text| text.contains("rich-layout-begin")) {
            return Some(root.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(found) = paragraph(&widget) {
            return Some(found);
        }
    }
    None
}
fn verify(widget: &gtk::Widget) -> Result<Value, String> {
    let Some(view) = widget.downcast_ref::<gtk::TextView>() else {
        return Ok(json!({"legacyLabel":true}));
    };
    if view.is_editable() || view.buffer().enables_undo() {
        return Err("Rich paragraph is not read only".into());
    }
    let buffer = view.buffer();
    if buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .len()
        > studio_markdown_native::inline::PAGE_BYTES
    {
        return Err("Rich page exceeds its byte budget".into());
    }
    for (text, name) in [
        ("bold 中文😀", "studio-bold"),
        ("italic", "studio-italic"),
        ("strike", "studio-strike"),
        ("code", "studio-inline-code"),
        ("linked 中文😀", "studio-link"),
    ] {
        let (start, _) = buffer
            .start_iter()
            .forward_search(text, gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Styled Unicode text missing")?;
        if !start
            .tags()
            .iter()
            .any(|tag| tag.name().as_deref() == Some(name))
        {
            return Err(format!("Missing {name} on {text}"));
        }
    }
    let all = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    if !all.contains("<b>literal</b>") {
        return Err("Literal HTML changed".into());
    }
    let (start, end) = buffer
        .start_iter()
        .forward_search("bold 中文😀", gtk::TextSearchFlags::TEXT_ONLY, None)
        .unwrap();
    buffer.select_range(&start, &end);
    let selected = crate::content_widgets::selection(widget, 16384);
    if selected.as_deref() != Some("bold 中文😀") {
        return Err("Rich-text selection changed its Unicode quote".into());
    }
    buffer.place_cursor(&buffer.start_iter());
    Ok(json!({"styles":true,"literalHtml":true,"unicodeSelection":true,"readOnly":true}))
}
async fn navigate(widget: &gtk::Widget) -> Result<Value, String> {
    let Some(view) = widget.downcast_ref::<gtk::TextView>() else {
        return Ok(json!({"legacyLabel":true}));
    };
    let buffer = view.buffer();
    view.activate_action("rich.last-page", None)
        .map_err(|e| e.to_string())?;
    if !buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .contains("rich-layout-tail end")
    {
        return Err("Paragraph last page lost its tail".into());
    }
    let mut end = buffer.end_iter();
    let start = std::time::Instant::now();
    buffer.place_cursor(&end);
    view.scroll_to_iter(&mut end, 0.0, false, 0.0, 1.0);
    until("paragraph tail visible", || {
        let glyph = view.iter_location(&buffer.end_iter());
        let visible = view.visible_rect();
        glyph.height() > 0
            && glyph.y() >= visible.y()
            && glyph.y() + glyph.height() <= visible.y() + visible.height() + 1
    })
    .await?;
    let end_ms = start.elapsed().as_millis();
    view.activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    let mut beginning = buffer.start_iter();
    buffer.place_cursor(&beginning);
    view.scroll_to_iter(&mut beginning, 0.0, false, 0.0, 0.0);
    until("paragraph start visible", || {
        let glyph = view.iter_location(&buffer.start_iter());
        let visible = view.visible_rect();
        glyph.height() > 0
            && glyph.y() >= visible.y()
            && glyph.y() + glyph.height() <= visible.y() + visible.height() + 1
    })
    .await?;
    Ok(json!({"tailVisible":true,"startRestored":true,"tailMs":end_ms}))
}
fn text_view(root: &gtk::Widget, needle: &str) -> Option<gtk::TextView> {
    if let Some(view) = root
        .downcast_ref::<gtk::TextView>()
        .filter(|v| v.is_mapped())
    {
        let buffer = view.buffer();
        if buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .contains(needle)
        {
            return Some(view.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(view) = text_view(&widget, needle) {
            return Some(view);
        }
    }
    None
}
async fn retention(ui: &Rc<Ui>, widget: gtk::Widget) -> Result<gtk::Widget, String> {
    let Some(view) = widget.downcast_ref::<gtk::TextView>() else {
        return Ok(widget);
    };
    view.activate_action("rich.last-page", None)
        .map_err(|e| e.to_string())?;
    let before = ui.transcript_scroll.vadjustment().value();
    let original = ui.state.borrow().items.clone();
    let equal = original
        .iter()
        .map(|item| Arc::new((**item).clone()))
        .collect::<Vec<_>>();
    crate::chat_ui::reconcile(&ui.transcript, &equal);
    until("equal refresh retains paragraph page", || {
        text_view(ui.transcript_view.upcast_ref(), "rich-layout-tail end").is_some()
    })
    .await?;
    crate::chat_ui::reconcile(&ui.transcript, &original);
    until(
        "original callback restoration retains paragraph page",
        || text_view(ui.transcript_view.upcast_ref(), "rich-layout-tail end").is_some(),
    )
    .await?;
    let view = text_view(ui.transcript_view.upcast_ref(), "rich-layout-tail end").unwrap();
    if (before - ui.transcript_scroll.vadjustment().value()).abs() > 1.0 {
        return Err("Paragraph paging moved the outer transcript".into());
    }
    view.activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    Ok(view.upcast())
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let last = Rc::new(Cell::new(started));
    let gap = Rc::new(Cell::new(0u64));
    let ticks = Rc::new(Cell::new(0u64));
    let l = last.clone();
    let g = gap.clone();
    let t = ticks.clone();
    let timer = glib::timeout_add_local(Duration::from_millis(10), move || {
        let now = std::time::Instant::now();
        g.set(
            g.get()
                .max(now.duration_since(l.replace(now)).as_millis() as u64),
        );
        t.set(t.get() + 1);
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
    let mut timer = Timer(Some(timer));
    until("conversation", || {
        ui.state.borrow().history_verified
            && !ui.state.borrow().loading
            && paragraph(ui.transcript_view.upcast_ref()).is_some()
    })
    .await?;
    let conversation = paragraph(ui.transcript_view.upcast_ref()).unwrap();
    let conversation_ms = started.elapsed().as_millis();
    let conversation_gap = gap.get();
    let conversation_height = conversation.height();
    let conversation_navigation = navigate(&conversation).await?;
    let conversation = retention(ui, conversation).await?;
    let (conversation, chat_position) = position_acceptance::chat(
        ui,
        conversation
            .downcast_ref::<gtk::TextView>()
            .ok_or("Scrollable conversation missing")?,
    )
    .await?;
    let conversation: gtk::Widget = conversation.upcast();
    capture("rich-conversation.png");
    ui.composer
        .buffer()
        .set_text("Unsent rich-layout draft 中文😀 ");
    let source = ui.state.borrow().items.clone();
    gap.set(0);
    last.set(std::time::Instant::now());
    let start = std::time::Instant::now();
    ui.reader.open_path(&directory.join("project/report.md"));
    ui.show_tool("document");
    until("reader", || {
        ui.reader.is_ready() && paragraph(ui.reader.widget.upcast_ref()).is_some()
    })
    .await?;
    let reader = paragraph(ui.reader.widget.upcast_ref()).unwrap();
    let reader_ms = start.elapsed().as_millis();
    let reader_gap = gap.get();
    let reader_height = reader.height();
    let reader_navigation = navigate(&reader).await?;
    let (reader, reader_position) = position_acceptance::reader(
        ui,
        reader
            .downcast_ref::<gtk::TextView>()
            .ok_or("Scrollable reader missing")?,
    )
    .await?;
    let reader: gtk::Widget = reader.upcast();
    let conversation_checks = verify(&conversation)?;
    let reader_checks = verify(&reader)?;
    let comment = ui.reader.smoke_rich_comment()?;
    let items = ui.state.borrow().items.clone();
    let hits = gio::spawn_blocking(move || {
        crate::session_search::find(
            &items,
            "rich-layout-tail",
            true,
            crate::session_search::Filter::All,
        )
    })
    .await
    .map_err(|_| "Search fixture worker stopped")??;
    let hit = hits
        .first()
        .ok_or("Rich paragraph search missed the tail")?;
    if hit.paragraph_page.is_none_or(|page| page == 0)
        || crate::chat_ui::locate(&ui.transcript, hit).is_none()
    {
        return Err("Rich paragraph search did not prepare a target page".into());
    }
    until("search page displayed", || {
        crate::chat_ui::target_visible(&ui.transcript, hit)
    })
    .await?;
    let items = ui.state.borrow().items.clone();
    let projection_hits = gio::spawn_blocking(
        move || -> Result<Vec<(crate::session_search::Occurrence, bool)>, String> {
            let item = items
                .iter()
                .find(|item| item.id == "rich-assistant")
                .ok_or("Projection fixture message missing")?;
            let pattern = regex::Regex::new("repeated-source 中文😀 ").unwrap();
            let mut hits = pattern
                .find_iter(&item.body)
                .map(|found| {
                    (
                        crate::session_search::occurrence(
                            item,
                            found,
                            crate::session_search::Filter::Assistant,
                        ),
                        false,
                    )
                })
                .collect::<Vec<_>>();
            if hits.len() != 2 || hits[0].0.paragraph_page == hits[1].0.paragraph_page {
                return Err("Repeated source matches did not resolve to distinct pages".into());
            }
            let phrase = if item.body.contains("字😀abcdef") {
                "字😀abcdef".repeat(11)
            } else {
                "Long paragraph 中文😀 keeps its context and formatting.".into()
            };
            let pattern = regex::Regex::new(&regex::escape(&phrase)).unwrap();
            let content = item.rich.as_ref().unwrap();
            for found in pattern.find_iter(&item.body) {
                let hit = crate::session_search::occurrence(
                    item,
                    found,
                    crate::session_search::Filter::Assistant,
                );
                if let (Some(block), Some(page), Some(range)) =
                    (hit.block, hit.paragraph_page, hit.paragraph_range.as_ref())
                {
                    let studio_markdown_native::BlockKind::Paragraph(text) =
                        &content.rows[block].block.kind
                    else {
                        continue;
                    };
                    let part = &text.attributes.as_ref().unwrap().pages[page];
                    if range.end > part.character + part.characters {
                        hits.push((hit, true));
                        return Ok(hits);
                    }
                }
            }
            Err("Cross-page search fixture did not exercise a split match".into())
        },
    )
    .await
    .map_err(|_| "Projection fixture worker stopped")??;
    for (index, (hit, continued)) in projection_hits.iter().enumerate() {
        let row = crate::chat_ui::locate(&ui.transcript, hit)
            .ok_or("Exact source projection failed to locate its row")?;
        ui.transcript_view
            .scroll_to(row, gtk::ListScrollFlags::NONE, None);
        until("exact projected selection visible", || {
            crate::chat_ui::target_visible(&ui.transcript, hit)
        })
        .await?;
        if *continued {
            fn notice(widget: &gtk::Widget) -> bool {
                if widget
                    .downcast_ref::<gtk::Label>()
                    .is_some_and(|label| label.text().contains("match continues on next page"))
                {
                    return true;
                }
                let mut child = widget.first_child();
                while let Some(widget) = child {
                    child = widget.next_sibling();
                    if notice(&widget) {
                        return true;
                    }
                }
                false
            }
            if !notice(ui.transcript_view.upcast_ref()) {
                return Err("Split search match has no continuation notice".into());
            }
        }
        capture(&format!("rich-projection-{index}.png"));
    }
    let items = ui.state.borrow().items.clone();
    let hidden = gio::spawn_blocking(move || {
        crate::session_search::find(
            &items,
            "nested/note.md:3",
            true,
            crate::session_search::Filter::All,
        )
    })
    .await
    .map_err(|_| "Hidden source fixture worker stopped")??;
    let hidden = hidden
        .first()
        .ok_or("Hidden source fixture missed the link URL")?;
    if hidden.paragraph_range.is_some() {
        return Err("Hidden source query selected visible text".into());
    }
    crate::chat_ui::locate(&ui.transcript, hidden).ok_or("Hidden source context did not open")?;
    until("source-only match notice", || {
        fn notice(widget: &gtk::Widget) -> bool {
            if widget
                .downcast_ref::<gtk::Label>()
                .is_some_and(|label| label.text().contains("match is in Markdown source"))
            {
                return true;
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if notice(&widget) {
                    return true;
                }
            }
            false
        }
        notice(ui.transcript_view.upcast_ref())
            && text_view(ui.transcript_view.upcast_ref(), "rich-layout-begin")
                .is_some_and(|view| !view.buffer().has_selection())
    })
    .await?;
    capture("rich-source-only.png");
    let position = ui.workspace.position();
    let frame = ui
        .window
        .frame_clock()
        .ok_or("Native frame clock missing")?;
    let previous_frame = frame.frame_counter();
    ui.workspace.set_position(320);
    until("narrow source notice layout", || {
        ui.conversation.width() == 320 && frame.frame_counter() >= previous_frame + 2
    })
    .await?;
    fn controls(
        widget: &gtk::Widget,
        relative: &gtk::Widget,
        count: &mut usize,
    ) -> Result<(), String> {
        if let Some(button) = widget.downcast_ref::<gtk::Button>().filter(|button| {
            button.is_mapped()
                && button
                    .action_name()
                    .is_some_and(|name| name.starts_with("rich."))
        }) {
            let bounds = button
                .compute_bounds(relative)
                .ok_or("Rich page control bounds missing")?;
            if bounds.x() < 0.0 || bounds.x() + bounds.width() > relative.width() as f32 + 1.0 {
                return Err("Paragraph page control escaped the narrow pane".into());
            }
            *count += 1;
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            controls(&widget, relative, count)?;
        }
        Ok(())
    }
    let mut count = 0;
    controls(
        ui.transcript_view.upcast_ref(),
        ui.conversation.upcast_ref(),
        &mut count,
    )?;
    if count != 4 {
        return Err("Narrow source notice lost paragraph navigation controls".into());
    }
    capture("rich-source-narrow.png");
    ui.workspace.set_position(position);
    until("normal paragraph pane restored", || {
        ui.conversation.width() == position
    })
    .await?;
    let conversation: gtk::Widget = text_view(ui.transcript_view.upcast_ref(), "rich-layout-begin")
        .ok_or("Current source-context paragraph missing")?
        .upcast();
    conversation
        .activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    // Both modes retain the exact text and source anchors; layout is the only change.
    let unchanged = {
        let state = ui.state.borrow();
        source.len() == state.items.len()
            && source
                .iter()
                .zip(&state.items)
                .all(|(a, b)| Arc::ptr_eq(a, b))
    };
    if ui.composer_text() != "Unsent rich-layout draft 中文😀 " || !unchanged {
        return Err("Reader layout changed the conversation or draft".into());
    }
    timer.0.take().unwrap().remove();
    capture("rich-reader.png");
    let mut link = false;
    if let Some(view) = conversation.downcast_ref::<gtk::TextView>() {
        let buffer = view.buffer();
        let (start, _) = buffer
            .start_iter()
            .forward_search("linked 中文😀", gtk::TextSearchFlags::TEXT_ONLY, None)
            .unwrap();
        buffer.place_cursor(&start);
        view.activate_action("rich.open-link", None)
            .map_err(|e| e.to_string())?;
        until("relative link", || {
            ui.reader.is_ready()
                && ui
                    .reader
                    .location()
                    .is_some_and(|(_, path)| path == directory.join("project/nested/note.md"))
        })
        .await?;
        link = true;
    }
    Ok(
        json!({"ok":true,"conversation":{"mappedMs":conversation_ms,"maximumGapMs":conversation_gap,"type":conversation.type_().name(),"height":conversation_height,"checks":conversation_checks,"navigation":conversation_navigation,"readingPosition":chat_position},"reader":{"mappedMs":reader_ms,"maximumGapMs":reader_gap,"type":reader.type_().name(),"height":reader_height,"checks":reader_checks,"navigation":reader_navigation,"readingPosition":reader_position},"comment":comment,"relativeLink":link,"equalRefreshRetention":true,"searchTargetPage":true,"sourceProjection":{"repeatedOccurrences":2,"splitMatch":true,"visibleSelection":true,"sourceOnlyNotice":true,"trailingSpace":true,"adaptiveNotice":true},"heartbeatTicks":ticks.get(),"draftRetention":true,"transcriptIsolation":true}),
    )
}
