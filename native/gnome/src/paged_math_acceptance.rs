//! Actual conversation and document paging, source targets and original quotes.
use super::*;
use std::sync::Arc;
use studio_markdown_native::rich_page::Source;
struct Heartbeat {
    id: Option<glib::SourceId>,
    gap: Rc<std::cell::Cell<u128>>,
    ticks: Rc<std::cell::Cell<u64>>,
}
impl Heartbeat {
    fn start() -> Self {
        let gap = Rc::new(std::cell::Cell::new(0));
        let ticks = Rc::new(std::cell::Cell::new(0));
        let (g, t) = (gap.clone(), ticks.clone());
        let mut last = std::time::Instant::now();
        let id = glib::timeout_add_local(Duration::from_millis(10), move || {
            let now = std::time::Instant::now();
            g.set(g.get().max(now.duration_since(last).as_millis()));
            t.set(t.get() + 1);
            last = now;
            glib::ControlFlow::Continue
        });
        Self {
            id: Some(id),
            gap,
            ticks,
        }
    }
}
impl Drop for Heartbeat {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            id.remove();
        }
    }
}

fn find(root: &gtk::Widget, expected: &str) -> Option<gtk::TextView> {
    if let Some(v) = root
        .downcast_ref::<gtk::TextView>()
        .filter(|v| v.is_mapped() && v.width() > 0 && v.height() > 0)
    {
        if crate::text_flow::quick_projection(v).is_some_and(|p| {
            p.quick.as_ref().unwrap().select(
                0..p.characters,
                studio_markdown_native::flow::RICH_TEXT_BYTES,
            ) == Some(expected)
        }) {
            return Some(v.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        if let Some(v) = find(&w, expected) {
            return Some(v);
        }
    }
    None
}
async fn page(root: &gtk::Widget, source: &Source, index: usize) -> Result<gtk::TextView, String> {
    let input = source.clone();
    let text = gio::spawn_blocking(move || input.page(index).map(|p| p.plain))
        .await
        .map_err(|_| "Page fixture stopped")??;
    until("rendered math page", || find(root, &text).is_some()).await?;
    Ok(find(root, &text).unwrap())
}
async fn copy_all(root: &std::path::Path, v: &gtk::TextView, expected: &str) -> Result<(), String> {
    let b = v.buffer();
    v.grab_focus();
    b.select_range(&b.start_iter(), &b.end_iter());
    copy_view(root, v, expected).await
}
fn raw_view(root: &gtk::Widget, expected: &str) -> Option<gtk::TextView> {
    if let Some(v) = root
        .downcast_ref::<gtk::TextView>()
        .filter(|v| v.is_mapped() && v.widget_name() == "studio-rich-text")
    {
        let b = v.buffer();
        if b.text(&b.start_iter(), &b.end_iter(), true).as_str() == expected {
            return Some(v.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        if let Some(v) = raw_view(&w, expected) {
            return Some(v);
        }
    }
    None
}
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    let heartbeat = Heartbeat::start();
    let began = std::time::Instant::now();
    until("paged conversation ready", || {
        let s = ui.state.borrow();
        s.ready && s.history_verified && !s.loading
    })
    .await?;
    crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    let item = ui
        .state
        .borrow()
        .items
        .iter()
        .find(|i| i.id == "paged-assistant")
        .cloned()
        .ok_or("No paged fixture message")?;
    let input = item.clone();
    let (source, hit, q) = gio::spawn_blocking(move || {
        let rich = studio_markdown_native::parse_text(&input.body)?
            .into_iter()
            .find_map(|b| match b.kind {
                studio_markdown_native::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .ok_or("Missing long paragraph")?;
        let q = rich.plain[rich.math[0].range.clone()].to_owned();
        let pattern = regex::Regex::new("frac").unwrap();
        let matched = pattern
            .find(&input.body)
            .ok_or("No partial formula match")?;
        let hit = crate::session_search::occurrence(
            &input,
            matched,
            crate::session_search::Filter::Assistant,
        );
        Ok::<_, String>((Arc::new(Source::new(&rich).unwrap()), hit, q))
    })
    .await
    .map_err(|_| "Paged fixture worker stopped")??;
    let count = source.attributes.pages.len();
    if count < 4 {
        return Err("Long fixture did not span enough pages".into());
    }
    ui.composer
        .buffer()
        .set_text("Unsent paged math draft 中文😀 ");
    let first = page(ui.transcript_view.upcast_ref(), &source, 0).await?;
    let chat_first_ms = began.elapsed().as_millis();
    let startup_gap = heartbeat.gap.replace(0);
    copy_all(
        root,
        &first,
        &source.attributes.text[source.attributes.pages[0].start..source.attributes.pages[0].end],
    )
    .await?;
    first
        .activate_action("rich-page.last-page", None)
        .map_err(|e| e.to_string())?;
    let last = page(ui.transcript_view.upcast_ref(), &source, count - 1).await?;
    let last_text = &source.attributes.text[source.attributes.pages[count - 1].start..];
    copy_all(root, &last, last_text).await?;
    // Production conversation search mapping must choose the atomic page and
    // select the complete formula for a match inside its original TeX.
    let row = crate::chat_ui::locate(&ui.transcript, &hit)
        .ok_or("Partial math match has no native row")?;
    ui.transcript_view
        .scroll_to(row, gtk::ListScrollFlags::NONE, None);
    until("partial formula source target", || {
        crate::chat_ui::target_visible(&ui.transcript, &hit)
    })
    .await?;
    let at = hit.paragraph_page.ok_or("Partial math match has no page")?;
    let chat = page(ui.transcript_view.upcast_ref(), &source, at).await?;
    if crate::content_widgets::selection(chat.upcast_ref(), 16384).as_deref() != Some(q.as_str()) {
        return Err("Partial search did not expand the atomic formula".into());
    }
    copy_view(root, &chat, &q).await?;
    let original = ui.state.borrow().items.clone();
    let input = original.clone();
    let equal = gio::spawn_blocking(move || {
        input
            .iter()
            .map(|item| {
                let mut item = (**item).clone();
                if item.rich.is_some() {
                    item.rich = Some(crate::chat_content::Content::parse(&item.body)?);
                }
                Ok::<_, String>(Arc::new(item))
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|_| "Equal math history worker stopped")??;
    for items in [&equal, &original] {
        let old = page(ui.transcript_view.upcast_ref(), &source, at).await?;
        crate::chat_ui::reconcile(&ui.transcript, items);
        let expected = &source.attributes.text
            [source.attributes.pages[at].start..source.attributes.pages[at].end];
        until("equal history retains math page and selection", || {
            find(ui.transcript_view.upcast_ref(), expected).is_some_and(|v| {
                v != old
                    && crate::content_widgets::selection(v.upcast_ref(), 16384).as_deref()
                        == Some(q.as_str())
            })
        })
        .await?;
        old.activate_action("rich-page.last-page", None).ok();
        old.buffer().place_cursor(&old.buffer().end_iter());
        glib::timeout_future(Duration::from_millis(50)).await;
        let current = page(ui.transcript_view.upcast_ref(), &source, at).await?;
        copy_view(root, &current, &q).await?;
    }
    crate::window_capture::capture(&ui.window, &root.join("paged-math-chat.png")).await?;
    ui.reader.open_path(&root.join("project/paged-math.md"));
    let opened = std::time::Instant::now();
    ui.show_tool("document");
    until("paged document ready", || ui.reader.is_ready()).await?;
    let mut reader = page(ui.reader.widget.upcast_ref(), &source, 0).await?;
    let reader_first_ms = opened.elapsed().as_millis();
    let mut page_ms = Vec::new();
    for index in 1..count {
        let changed = std::time::Instant::now();
        reader
            .activate_action("rich-page.next-page", None)
            .map_err(|e| e.to_string())?;
        reader = page(ui.reader.widget.upcast_ref(), &source, index).await?;
        page_ms.push(changed.elapsed().as_millis());
        let part = &source.attributes.pages[index];
        copy_all(root, &reader, &source.attributes.text[part.start..part.end]).await?;
    }
    // One jump supersedes the queued one; detached page actions cannot revive
    // the old request or move the replacement buffer.
    let old = reader.clone();
    let owner = reader
        .ancestor(crate::rich_paragraph::Paragraph::static_type())
        .ok_or("No paged owner")?;
    for action in [
        "rich-page.first-page",
        "rich-page.last-page",
        "rich-page.first-page",
    ] {
        owner
            .activate_action(action, None)
            .map_err(|e| e.to_string())?;
        glib::timeout_future(Duration::from_millis(1)).await;
    }
    page(ui.reader.widget.upcast_ref(), &source, 0).await?;
    old.activate_action("rich-page.last-page", None).ok();
    glib::timeout_future(Duration::from_millis(80)).await;
    reader = page(ui.reader.widget.upcast_ref(), &source, 0).await?;
    for index in 1..=at {
        reader
            .activate_action("rich-page.next-page", None)
            .map_err(|e| e.to_string())?;
        reader = page(ui.reader.widget.upcast_ref(), &source, index).await?;
    }
    let projection = crate::text_flow::quick_projection(&reader).unwrap();
    let math = &projection.objects[0];
    let b = reader.buffer();
    reader.grab_focus();
    b.select_range(
        &b.iter_at_offset(math.characters.start as i32),
        &b.iter_at_offset(math.characters.end as i32),
    );
    copy_view(root, &reader, &q).await?;
    // Actual keyboard navigation interrupts a deferred reading-position restore,
    // just as a real selection/copy does. Do not race a synthetic selection
    // against GTK's pending initial allocation/scroll restoration.
    crate::smoke_input::request(root, json!({"action":"key","key":"copy"})).await?;
    if crate::translation_ui::selection(reader.upcast_ref()).as_deref() != Some(q.as_str()) {
        return Err("Translation received an object placeholder instead of original math".into());
    }
    let comment = ui.reader.smoke_inline_comment_range(&q, 3, 4)?;
    for round in 0..2 {
        ui.reader.show_source();
        until("math source mode ready", || {
            ui.reader.source_mode() && ui.reader.is_ready()
        })
        .await?;
        let foreign = format!("Foreign primary during Source {round}");
        reader.primary_clipboard().set_text(&foreign);
        ui.reader.show_preview();
        reader = page(ui.reader.widget.upcast_ref(), &source, at).await?;
        until("math selection survives source preview", || {
            crate::content_widgets::selection(reader.upcast_ref(), 16384).as_deref()
                == Some(q.as_str())
        })
        .await?;
        let primary = crate::smoke_input::request(root, json!({"action":"read-primary"})).await?;
        if primary["text"].as_str() != Some(foreign.as_str()) {
            return Err("Source/Preview reclaimed foreign primary selection".into());
        }
        reader.grab_focus();
        crate::smoke_input::request(root, json!({"action":"key","key":"copy"})).await?;
        copy_view(root, &reader, &q).await?;
    }
    // Nonzero page offsets must survive replacement in both directions.
    let initial_dark = adw::StyleManager::default().is_dark();
    for dark in [!initial_dark, initial_dark] {
        let old = reader.clone();
        adw::StyleManager::default().set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        until("themed nonzero math page", || {
            find(
                ui.reader.widget.upcast_ref(),
                &source.attributes.text
                    [source.attributes.pages[at].start..source.attributes.pages[at].end],
            )
            .is_some_and(|v| {
                v != old
                    && crate::content_widgets::selection(v.upcast_ref(), 16384).as_deref()
                        == Some(q.as_str())
            })
        })
        .await
        .map_err(|e| {
            format!(
                "{e}: selection={:?}",
                find(
                    ui.reader.widget.upcast_ref(),
                    &source.attributes.text
                        [source.attributes.pages[at].start..source.attributes.pages[at].end]
                )
                .map(|v| (
                    v == old,
                    crate::content_widgets::selection(v.upcast_ref(), 16384),
                    v.buffer().cursor_position()
                ))
            )
        })?;
        reader = page(ui.reader.widget.upcast_ref(), &source, at).await?;
        copy_view(root, &reader, &q).await?;
    }
    crate::window_capture::capture(&ui.window, &root.join("paged-math-reader.png")).await?;
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if label(
        ui.reader.widget.upcast_ref(),
        &format!("第 {} / {} 页", at + 1, count),
    )
    .is_none()
    {
        return Err("Paged controls did not switch language".into());
    }
    if crate::content_widgets::selection(reader.upcast_ref(), 16384).as_deref() != Some(q.as_str())
    {
        return Err("Language switch changed source selection".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("paged-math-chinese.png")).await?;
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    if label(
        ui.reader.widget.upcast_ref(),
        &format!("Page {}/{}", at + 1, count),
    )
    .is_none()
    {
        return Err("Paged controls did not restore English".into());
    }
    let projection = crate::text_flow::quick_projection(&reader).unwrap();
    let link = projection.links.first().ok_or("Paged link disappeared")?;
    reader.buffer().place_cursor(
        &reader
            .buffer()
            .iter_at_offset(link.characters.start as i32 + 1),
    );
    reader
        .activate_action("rich.open-link", None)
        .map_err(|e| e.to_string())?;
    until("link after rebased formula", || {
        ui.reader.is_ready()
            && ui
                .reader
                .location()
                .is_some_and(|(_, path)| path.ends_with("nested/note.md"))
    })
    .await?;
    let interactive_gap = heartbeat.gap.replace(0);
    let oversized = root.join("project/oversize-math.md");
    let file = oversized.clone();
    let (prefix, formula) = gio::spawn_blocking(move || {
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let rich = studio_markdown_native::parse_text(&text)?
            .into_iter()
            .find_map(|b| match b.kind {
                studio_markdown_native::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .ok_or("No oversized paragraph")?;
        let source = Source::new(&rich).unwrap();
        Ok::<_, String>((source.page(0)?.plain, source.page(1)?.plain))
    })
    .await
    .map_err(|_| "Oversize fixture stopped")??;
    if formula.len() <= studio_markdown_native::inline::PAGE_BYTES {
        return Err("Fixture did not exercise an atomic oversize formula".into());
    }
    ui.reader.open_path(&oversized);
    until("oversize source page", || {
        ui.reader.is_ready() && find(ui.reader.widget.upcast_ref(), &prefix).is_some()
    })
    .await?;
    let prefix = find(ui.reader.widget.upcast_ref(), &prefix).unwrap();
    prefix
        .activate_action("rich-page.next-page", None)
        .map_err(|e| e.to_string())?;
    until("failed formula remains exact readable source", || {
        raw_view(ui.reader.widget.upcast_ref(), &formula).is_some_and(|v| {
            v.ancestor(crate::rich_paragraph::Paragraph::static_type())
                .and_then(|r| r.last_child())
                .and_then(|w| w.tooltip_text())
                .is_some()
        })
    })
    .await?;
    let raw = raw_view(ui.reader.widget.upcast_ref(), &formula).unwrap();
    if raw.wrap_mode() != gtk::WrapMode::None {
        return Err("Oversize literal formula still uses expensive wrapping".into());
    }
    copy_all(root, &raw, &formula).await?;
    crate::window_capture::capture(&ui.window, &root.join("paged-math-budget-source.png")).await?;
    let source_gap = heartbeat.gap.replace(0);
    let dense = root.join("project/dense-math.md");
    let file = dense.clone();
    let plain = gio::spawn_blocking(move || {
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let rich = studio_markdown_native::parse_text(&text)?
            .into_iter()
            .find_map(|b| match b.kind {
                studio_markdown_native::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .ok_or("No dense paragraph")?;
        if rich
            .attributes
            .as_ref()
            .is_none_or(|a| a.spans.len() < 1000)
            || rich.plain.len() > studio_markdown_native::inline::PAGE_BYTES
        {
            return Err("Fixture did not exercise markup-heavy short text".into());
        }
        Ok::<_, String>(rich.plain)
    })
    .await
    .map_err(|_| "Dense fixture stopped")??;
    ui.reader.open_path(&dense);
    until("markup-heavy math rendered with bounded tags", || {
        ui.reader.is_ready() && find(ui.reader.widget.upcast_ref(), &plain).is_some()
    })
    .await?;
    let dense_view = find(ui.reader.widget.upcast_ref(), &plain).unwrap();
    if dense_view.height() < 300 {
        return Err("Markup-heavy text was clipped to a short viewport".into());
    }
    let tags = dense_view.buffer().tag_table().size();
    if tags > 16 {
        return Err(format!(
            "Inline styles allocated {tags} native tags for repeated emphasis"
        ));
    }
    copy_all(root, &dense_view, &plain).await?;
    crate::window_capture::capture(&ui.window, &root.join("paged-math-dense.png")).await?;
    let dense_gap = heartbeat.gap.get();
    if ui
        .composer
        .buffer()
        .text(
            &ui.composer.buffer().start_iter(),
            &ui.composer.buffer().end_iter(),
            true,
        )
        .as_str()
        != "Unsent paged math draft 中文😀 "
    {
        return Err("Paging changed the composer draft".into());
    }
    Ok(
        json!({"ok":true,"pages":count,"chatPaging":true,"readerPaging":true,"exactCopy":true,"partialFormulaSearch":true,"sourceComment":comment,"equalHistoryRetention":true,"sourcePreviewRoundTrip":true,"themesRetainNonzeroPage":true,"stalePageActionRejected":true,"rapidPageChanges":true,"draftRetained":true,"language":true,"translationSource":true,"rebasedLink":true,"boundedFailureReadable":true,"reusedStyleTags":tags,"measurements":{"chatFirstAllocatedMs":chat_first_ms,"readerFirstAllocatedMs":reader_first_ms,"readerPageAllocationMs":page_ms,"maximumMainLoopGapMs":startup_gap.max(interactive_gap).max(source_gap).max(dense_gap),"startupGapMs":startup_gap,"interactiveGapMs":interactive_gap,"oversizeSourceGapMs":source_gap,"denseStyleGapMs":dense_gap,"ticks":heartbeat.ticks.get()}}),
    )
}
