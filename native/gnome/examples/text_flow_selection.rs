//! Standalone architectural probe. The application renderer is unchanged.
#![allow(dead_code)]
#[cfg(target_os = "linux")]
#[path = "../src/smoke_input.rs"]
mod smoke_input;
#[cfg(target_os = "linux")]
use codex_thread_studio_gnome::text_flow;
#[cfg(target_os = "linux")]
#[path = "../src/window_capture.rs"]
mod window_capture;

#[cfg(target_os = "linux")]
mod gui {
    use super::*;
    use adw::prelude::*;
    use gtk::{gio, glib};
    use serde_json::{json, Value};
    use std::{
        cell::{Cell, RefCell},
        path::{Path, PathBuf},
        rc::Rc,
        time::{Duration, Instant},
    };
    use studio_markdown_native::{Fragment, FragmentMetadata, SourceWindow};
    const MIXED:&str="# Native text flow\n\nBegin cross-widget 中文😀 **bold** and $x^2$ with [link 中文😀](https://example.invalid).\n\n```rust\nlet value = \"Source 中文😀\";\n```\n\n| Name | Value |\n|---|---|\n| first | 中文😀 |\n| second | $y+1$ |\n\nEnd range 中文😀.\n\n```mermaid\nflowchart LR\nA[Start]-->B[Done]\n```\n\n```plantuml\n@startuml\nAlice -> Bob: flow\n@enduml\n```\n\n```d2\na -> b\n```\n\n```math\nx=\\frac{a}{b}\n```\n\nFinal paragraph.\n";
    fn fragment(text: String) -> Fragment {
        Fragment {
            page: 0,
            document_id: "flow-probe".into(),
            revision: "immutable-probe".into(),
            metadata: FragmentMetadata {
                start: 0,
                end: text.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: SourceWindow {
                start: 0,
                end: text.len(),
                line: 1,
                end_line: text.lines().count(),
                text,
                projection: vec![],
                references: vec![],
            },
            header: None,
        }
    }
    async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
        let at = Instant::now();
        while !test() {
            if at.elapsed() > Duration::from_secs(15) {
                return Err(format!("Text-flow probe timed out: {step}"));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    fn point(
        view: &gtk::TextView,
        window: &adw::ApplicationWindow,
        index: usize,
    ) -> Result<(f64, f64), String> {
        character_point(view, window, index, false)
    }
    fn character_point(
        view: &gtk::TextView,
        window: &adw::ApplicationWindow,
        index: usize,
        right_edge: bool,
    ) -> Result<(f64, f64), String> {
        let rect = view.iter_location(&view.buffer().iter_at_offset(index as i32));
        let (x, y) = view.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            rect.x()
                + if right_edge {
                    rect.width().saturating_sub(1)
                } else {
                    2
                },
            rect.y() + rect.height() / 2,
        );
        let p = view
            .compute_point(window, &gtk::graphene::Point::new(x as f32, y as f32))
            .ok_or("No mapped text coordinate")?;
        if p.x() < 0.0 || p.y() < 0.0 || p.x() >= 1600.0 || p.y() >= 1000.0 {
            return Err("Text-flow drag point is outside the owned monitor".into());
        }
        Ok((p.x() as f64, p.y() as f64))
    }
    async fn copy(flow: &text_flow::View, root: &Path) -> Result<Value, String> {
        flow.copied.borrow_mut().take();
        smoke_input::request(root, json!({"action":"key","key":"copy"})).await?;
        until("worker clipboard result", || {
            flow.copied.borrow().is_some() && !flow.copying.get()
        })
        .await?;
        let picked = flow
            .copied
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .map_err(Clone::clone)?
            .clone();
        let clipboard = smoke_input::request(root, json!({"action":"read"})).await?;
        if clipboard["text"].as_str() != Some(&picked.text) {
            return Err(
                "Independent clipboard differs from the immutable selection projection".into(),
            );
        }
        Ok(
            json!({"characters":[picked.characters.start,picked.characters.end],"bytes":picked.text.len(),"startLine":picked.source.as_ref().map(|s|s.line),"endLine":picked.source.as_ref().map(|s|s.end_line)}),
        )
    }
    fn find(root: &gtk::Widget, predicate: &dyn Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
        if predicate(root) {
            return Some(root.clone());
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            if let Some(found) = find(&widget, predicate) {
                return Some(found);
            }
            child = widget.next_sibling();
        }
        None
    }
    fn first_table_line(view: &gtk::ColumnView) -> Option<usize> {
        let value = view
            .model()?
            .item(0)?
            .downcast::<glib::BoxedAnyObject>()
            .ok()?;
        let line = value.borrow::<studio_markdown_native::Row>().source.line;
        Some(line)
    }
    fn cell_window() -> Option<gtk::Window> {
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.widget_name() == "studio-selection-window" && w.is_mapped())
    }
    fn resident_kib() -> usize {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|text| {
                text.lines().find_map(|line| {
                    line.strip_prefix("VmRSS:")
                        .and_then(|v| v.split_whitespace().next())
                        .and_then(|v| v.parse().ok())
                })
            })
            .unwrap_or(0)
    }
    fn actual_foreground(view: &gtk::TextView, offset: usize) -> Option<gtk::gdk::RGBA> {
        view.buffer()
            .iter_at_offset(offset as i32)
            .tags()
            .into_iter()
            .filter(|tag| {
                tag.name()
                    .is_some_and(|name| name.starts_with("studio-flow-syntax-"))
            })
            .max_by_key(|tag| tag.priority())
            .and_then(|tag| tag.foreground_rgba())
    }
    fn matches_paint(actual: Option<gtk::gdk::RGBA>, expected: [u8; 4]) -> bool {
        actual.is_some_and(|color| {
            [color.red(), color.green(), color.blue(), color.alpha()]
                .into_iter()
                .zip(expected)
                .all(|(a, b)| (a - b as f32 / 255.0).abs() < 0.002)
        })
    }
    fn syntax_foreground(
        flow: &text_flow::View,
        dark: bool,
        offset: usize,
    ) -> Result<[u8; 4], String> {
        let code = flow
            .syntax
            .blocks
            .first()
            .ok_or("Prepared code colors missing")?;
        let colors = if dark { &code.dark } else { &code.light };
        let colors = colors.as_ref().ok_or("Expected code was not highlighted")?;
        Ok(colors
            .runs
            .iter()
            .find(|r| r.characters.contains(&offset))
            .map_or(colors.base.foreground, |r| r.paint.foreground))
    }
    async fn syntax_theme(
        flow: &text_flow::View,
        root: &Path,
        window: &adw::ApplicationWindow,
        name: &str,
    ) -> Result<Value, String> {
        let manager = adw::StyleManager::default();
        manager.set_color_scheme(adw::ColorScheme::ForceLight);
        let buffer = flow.widget.buffer();
        let code = flow
            .syntax
            .blocks
            .first()
            .ok_or("Synthetic code block missing")?;
        let keyword = flow
            .document
            .buffer
            .find("let value")
            .or_else(|| flow.document.buffer.find("const value"))
            .ok_or("Code keyword missing")?;
        let keyword = flow.document.buffer[..keyword].chars().count();
        let light = syntax_foreground(flow, false, keyword)?;
        let dark = syntax_foreground(flow, true, keyword)?;
        if light == dark {
            return Err("Test keyword needs different theme colors".into());
        }
        until("actual light code colors", || {
            matches_paint(actual_foreground(&flow.widget, keyword), light)
        })
        .await?;
        buffer.select_range(
            &buffer.iter_at_offset(code.characters.start as i32),
            &buffer.iter_at_offset(code.characters.end as i32),
        );
        buffer.set_modified(false);
        let before = copy(flow, root).await?;
        let original = buffer
            .slice(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        let selection = buffer
            .selection_bounds()
            .map(|(a, b)| (a.offset(), b.offset()));
        let started = Instant::now();
        manager.set_color_scheme(adw::ColorScheme::ForceDark);
        until("actual dark code colors", || {
            matches_paint(actual_foreground(&flow.widget, keyword), dark)
        })
        .await?;
        let dark_ms = started.elapsed().as_millis();
        glib::timeout_future(Duration::from_millis(100)).await;
        let after = copy(flow, root).await?;
        if before != after
            || original != buffer.slice(&buffer.start_iter(), &buffer.end_iter(), true)
            || buffer.is_modified()
            || selection
                != buffer
                    .selection_bounds()
                    .map(|(a, b)| (a.offset(), b.offset()))
            || !flow.syntax_current()
        {
            return Err("Syntax theme replay changed text, source selection or dirty state".into());
        }
        window_capture::capture(window, &root.join(format!("{name}-syntax-dark.png"))).await?;
        // Observe unselected code paint as well as selection preservation.
        buffer.place_cursor(&buffer.iter_at_offset(code.characters.start as i32));
        glib::timeout_future(Duration::from_millis(100)).await;
        window_capture::capture(
            window,
            &root.join(format!("{name}-syntax-dark-reading.png")),
        )
        .await?;
        let mut peak_tags = 0;
        for dark_now in [false, true, false, true, false] {
            manager.set_color_scheme(if dark_now {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
            let expected = if dark_now { dark } else { light };
            until("cached theme replay", || {
                matches_paint(actual_foreground(&flow.widget, keyword), expected)
            })
            .await?;
            let count = Cell::new(0);
            buffer.tag_table().foreach(|tag| {
                if tag
                    .name()
                    .is_some_and(|n| n.starts_with("studio-flow-syntax-"))
                {
                    count.set(count.get() + 1);
                }
            });
            peak_tags = peak_tags.max(count.get());
            if count.get() > studio_markdown_native::syntax::PALETTE {
                return Err("Syntax tags accumulate across theme changes".into());
            }
        }
        manager.set_color_scheme(adw::ColorScheme::ForceLight);
        until("light code restored", || {
            matches_paint(actual_foreground(&flow.widget, keyword), light)
        })
        .await?;
        Ok(
            json!({"workerRanges":true,"actualLightDarkPaint":true,"darkObservedMs":dark_ms,"textSelectionSourceUnchanged":true,"notAnEdit":true,"exactCopy":before,"boundedThemeTags":peak_tags}),
        )
    }
    async fn primary_selection(
        window: &adw::ApplicationWindow,
        root: &Path,
    ) -> Result<Value, String> {
        let text = "# Primary selection\n\nBefore 中文😀 literal \u{fffc} and $x^2$ after.\n\n| Name | Value |\n|---|---|\n| first | 中文😀 $y+1$ |\n\nEnd range.\n";
        let prepared = gio::spawn_blocking(move || text_flow::prepare(&fragment(text.into())))
            .await
            .map_err(|_| "Primary fixture preparation stopped")??;
        let flow = text_flow::view(prepared, Rc::new(|_| {}))?;
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .child(&flow.widget)
            .build();
        let sink = gtk::TextView::builder()
            .height_request(160)
            .editable(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();
        content.append(&scroll);
        content.append(&sink);
        window.set_content(Some(&content));
        window.fullscreen();
        until("primary fixture fullscreen", || {
            window.width() == 1600 && window.height() == 1000 && flow.widget.is_mapped()
        })
        .await?;
        smoke_input::request(root, json!({"action":"focus"})).await?;
        let (x, y) = point(&flow.widget, window, 0)?;
        smoke_input::request(root, json!({"action":"pointer","x":x,"y":y})).await?;
        let buffer = flow.widget.buffer();
        let expected = flow
            .document
            .select(0..buffer.char_count() as usize, 512 * 1024)?
            .text;
        if expected.matches('\u{fffc}').count() != 1
            || !expected.contains("x^2")
            || !expected.contains("y+1")
        {
            return Err("Fixture lacks literal placeholder/formula/table alternatives".into());
        }
        flow.widget
            .clipboard()
            .set_text("ordinary clipboard sentinel 中文😀");
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        let reply = smoke_input::request(root, json!({"action":"read-primary"})).await?;
        if reply["text"].as_str() != Some(expected.as_str()) {
            return Err(format!("Independent primary read differs: {reply}"));
        }
        if !buffer.has_selection() {
            return Err("Publishing primary cleared native selection".into());
        }
        let sink_bounds = sink
            .compute_bounds(window)
            .ok_or("No paste target allocation")?;
        // GNOME disables primary paste by default. Enable only this private
        // process's GtkSettings; production follows the desktop preference.
        sink.settings()
            .set_property("gtk-enable-primary-paste", true);
        smoke_input::request(root,json!({"action":"pointer","x":sink_bounds.x()+24.0,"y":sink_bounds.y()+24.0,"button":274})).await?;
        until("native middle-button paste", || {
            sink.buffer().char_count() > 0
        })
        .await?;
        let pasted =
            sink.buffer()
                .text(&sink.buffer().start_iter(), &sink.buffer().end_iter(), true);
        if pasted.as_str() != expected {
            return Err(format!("Middle paste differs: {pasted}"));
        }
        if flow
            .widget
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some("ordinary clipboard sentinel 中文😀")
        {
            return Err("Primary selection changed ordinary clipboard".into());
        }
        window_capture::capture(window, &root.join("primary-flow.png")).await?;
        let primary = flow.widget.primary_clipboard();
        primary.set_text("foreign while mapped");
        buffer.create_mark(
            Some("primary-fixture-layout-mark"),
            &buffer.end_iter(),
            true,
        );
        if primary
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some("foreign while mapped")
        {
            return Err("An unrelated mark reclaimed another owner's primary selection".into());
        }
        window.set_content(None::<&gtk::Widget>);
        if primary
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some("foreign while mapped")
        {
            return Err("Unmapping the source changed another owner's primary selection".into());
        }
        window.set_content(Some(&content));
        until("primary source remap", || flow.widget.is_mapped()).await?;
        let old = primary.content().ok_or("Primary offer missing")?;
        buffer.place_cursor(&buffer.start_iter());
        if primary.content().as_ref() == Some(&old) {
            return Err("Selection clear retained old offer".into());
        }
        let rejected = gtk::gio::MemoryOutputStream::new_resizable();
        if old
            .write_mime_type_future(
                "text/plain;charset=utf-8",
                &rejected,
                glib::Priority::DEFAULT,
            )
            .await
            .is_ok()
            || rejected.data_size() != 0
        {
            return Err("A withdrawn provider exported stale selection bytes".into());
        }
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        let immutable = primary.content().ok_or("No replacement offer")?;
        let mut at = buffer.end_iter();
        buffer.insert(&mut at, " changed");
        if primary.content().as_ref() == Some(&immutable) || flow.syntax_current() {
            return Err("Mutation retained projected offer".into());
        }
        let replacement = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
        replacement.set_text("replacement");
        flow.widget.set_buffer(Some(&replacement));
        primary.set_text("foreign primary sentinel");
        window.set_content(None::<&gtk::Widget>);
        glib::timeout_future(Duration::from_millis(40)).await;
        if primary
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some("foreign primary sentinel")
        {
            return Err("Closing old view changed another owner's primary selection".into());
        }
        // An individually bounded table can take the aggregate selection over
        // 512 KiB. Build that synthetic alternative on the preparation worker.
        let prepared = gio::spawn_blocking(move || {
            let mut prepared = text_flow::prepare(&fragment(text.into()))?;
            let document = std::sync::Arc::make_mut(&mut prepared.document);
            let table = document
                .objects
                .iter_mut()
                .find(|o| {
                    matches!(
                        o.kind,
                        studio_markdown_native::flow::ObjectKind::Table { .. }
                    )
                })
                .ok_or("No budget fixture table")?;
            table.alternative = "x".repeat(512 * 1024);
            Ok::<_, String>(prepared)
        })
        .await
        .map_err(|_| "Budget fixture worker stopped")??;
        let bounded = text_flow::view(prepared, Rc::new(|_| {}))?;
        window.set_content(Some(&bounded.widget));
        until("bounded primary fixture", || bounded.widget.is_mapped()).await?;
        let bounded_buffer = bounded.widget.buffer();
        bounded_buffer.select_range(&bounded_buffer.start_iter(), &bounded_buffer.end_iter());
        let budget_offer = primary.content().ok_or("No bounded primary offer")?;
        let stream = gio::MemoryOutputStream::new_resizable();
        let failed = budget_offer
            .write_mime_type_future("text/plain;charset=utf-8", &stream, glib::Priority::DEFAULT)
            .await;
        if !failed.is_err_and(|e| e.message().contains("byte budget")) || stream.data_size() != 0 {
            return Err("Oversized selection exported partial or unbounded text".into());
        }
        window.set_content(None::<&gtk::Widget>);
        if primary.content().as_ref() == Some(&budget_offer) {
            return Err("Unmap retained the current primary offer".into());
        }
        Ok(
            json!({"ok":true,"experimental":true,"productionEnabled":false,"primarySelection":{"independentWaylandClient":true,"nativeMiddlePaste":true,"literalPlaceholderPreserved":true,"mathAndTableExpanded":true,"ordinaryClipboardPreserved":true,"selectionClear":true,"mutationWithdraws":true,"replacementBalanced":true,"foreignOwnerPreserved":true,"unrelatedMarkPreservesOwner":true,"staleOfferRejected":true,"aggregateBudgetRejectsWithoutPartialBytes":true,"unmapWithdraws":true,"bytes":expected.len()}}),
        )
    }
    async fn run(window: &adw::ApplicationWindow, root: &Path) -> Result<Value, String> {
        if std::env::var_os("STUDIO_FLOW_PRIMARY_ONLY").is_some() {
            return primary_selection(window, root).await;
        }
        #[cfg(debug_assertions)]
        if std::env::var_os("STUDIO_FLOW_COLUMNS_ONLY").is_some() {
            let columns = text_flow::column_acceptance::run(window, root).await?;
            return Ok(
                json!({"ok":true,"experimental":true,"productionEnabled":false,"columnPicker":columns}),
            );
        }
        #[cfg(debug_assertions)]
        if std::env::var_os("STUDIO_FLOW_IMAGES_ONLY").is_some() {
            let images = text_flow::image_acceptance::run(window, root).await?;
            return Ok(
                json!({"ok":true,"experimental":true,"productionEnabled":false,"imageAnchors":images}),
            );
        }
        let ticks = Rc::new(Cell::new(0usize));
        let previous_tick = Rc::new(Cell::new(Instant::now()));
        let longest_gap = Rc::new(Cell::new(Duration::ZERO));
        let pulse = ticks.clone();
        let last_tick = previous_tick.clone();
        let max_gap = longest_gap.clone();
        let timer = glib::timeout_add_local(Duration::from_millis(10), move || {
            pulse.set(pulse.get() + 1);
            max_gap.set(
                max_gap
                    .get()
                    .max(last_tick.replace(Instant::now()).elapsed()),
            );
            glib::ControlFlow::Continue
        });
        let mut results = Vec::new();
        let mixed = MIXED.replace(
            "$y+1$",
            &format!(
                "$y+1$ **styled** {} tail-cell 中文😀",
                "context 中文😀 ".repeat(90)
            ),
        );
        let mut cases = vec![("mixed".to_string(), mixed.clone(), mixed.len() as u64)];
        for (name, text) in [
            (
                "syntax-typescript",
                "Before 中文😀.\n\n```ts\nconst value: string = \"中文😀\";\n```\n",
            ),
            (
                "syntax-tsx",
                "Before 中文😀.\n\n```tsx\nconst value = <p>中文😀</p>;\n```\n",
            ),
        ] {
            cases.push((name.into(), text.into(), text.len() as u64));
            cases.push((format!("{name}-warm"), text.into(), text.len() as u64));
        }
        if let Some(path) = std::env::var_os("STUDIO_FLOW_REFERENCE") {
            let (text, total) = gio::spawn_blocking(move || {
                use std::io::Read;
                let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
                let total = file.metadata().map_err(|e| e.to_string())?.len();
                let mut bytes = Vec::new();
                file.take(64 * 1024)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if let Some(end) = bytes.iter().rposition(|b| *b == b'\n') {
                    bytes.truncate(end + 1);
                }
                Ok::<_, String>((String::from_utf8(bytes).map_err(|e| e.to_string())?, total))
            })
            .await
            .map_err(|_| "Reference read worker stopped")??;
            cases.push(("reference".into(), text.clone(), total));
            cases.push(("reference-warm".into(), text, total));
        }
        for name in ["syntax-mutate", "syntax-replace"] {
            let text = "Before 中文😀.\n\n```rust\nlet value=123;\n```\n".to_string();
            cases.push((name.into(), text.clone(), text.len() as u64));
        }
        for (name, text) in [
            (
                "syntax-unknown",
                "```unknown-native-syntax\n<script>literal 中文😀</script>\n```\n".to_owned(),
            ),
            (
                "syntax-long-line",
                format!(
                    "```rust\n{} tail-marker 中文😀\n```\n",
                    "x".repeat(studio_markdown_native::syntax::LINE_BYTES + 1)
                ),
            ),
        ] {
            cases.push((name.into(), text.clone(), text.len() as u64));
        }
        let previous_view = glib::WeakRef::<gtk::TextView>::new();
        for (name, text, total_bytes) in cases {
            let first = Instant::now();
            previous_tick.set(first);
            longest_gap.set(Duration::ZERO);
            let before = ticks.get();
            let prepared = gio::spawn_blocking(move || {
                let resident_before = resident_kib();
                let at = Instant::now();
                text_flow::prepare(&fragment(text)).map(|value| {
                    (
                        value,
                        at.elapsed().as_micros(),
                        resident_before,
                        resident_kib(),
                    )
                })
            })
            .await
            .map_err(|_| "Projection worker stopped")??;
            let (prepared, worker_us, worker_rss_before, worker_rss_after) = prepared;
            let tables: Vec<_> = prepared
                .document
                .objects
                .iter()
                .filter_map(|object| {
                    if let studio_markdown_native::flow::ObjectKind::Table {
                        header, rows, ..
                    } = &object.kind
                    {
                        Some(json!({"columns":header.len(),"rows":rows.len()}))
                    } else {
                        None
                    }
                })
                .collect();
            let raster_bytes = prepared.raster_bytes;
            let syntax_us = prepared.syntax_us;
            let syntax_bytes = prepared.syntax.heap_bytes();
            let construct = Instant::now();
            let opened = Rc::new(RefCell::new(Vec::<String>::new()));
            let observed = opened.clone();
            let flow = text_flow::view(
                prepared,
                Rc::new(move |value| observed.borrow_mut().push(value.to_owned())),
            )?;
            let construct_us = construct.elapsed().as_micros();
            let scroll = gtk::ScrolledWindow::builder()
                .child(&flow.widget)
                .hexpand(true)
                .vexpand(true)
                .build();
            window.set_content(Some(&scroll));
            until("old text-flow widget released", || {
                previous_view.upgrade().is_none()
            })
            .await?;
            previous_view.set(Some(&flow.widget));
            window.fullscreen();
            window.present();
            until("painted text and child anchors", || {
                window.width() == 1600
                    && window.height() == 1000
                    && flow.widget.width() > 500
                    && flow.objects.iter().all(|w| w.width() > 0 && w.height() > 0)
            })
            .await?;
            let allocated_ms = first.elapsed().as_millis();
            glib::timeout_future(Duration::from_millis(120)).await;
            let open_ms = first.elapsed().as_millis();
            let opening_ticks = ticks.get() - before;
            let opening_gap_ms = longest_gap.get().as_millis();
            let mapped_rss = gio::spawn_blocking(resident_kib)
                .await
                .map_err(|_| "RSS observation stopped")?;
            smoke_input::request(root, json!({"action":"focus"})).await?;
            flow.widget.grab_focus();
            let buffer = flow.widget.buffer();
            buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
            let full = copy(&flow, root).await?;
            let expected = flow.document.select(
                0..flow.document.characters,
                studio_markdown_native::flow::BUFFER_BYTES,
            )?;
            if full["bytes"] != expected.text.len() {
                return Err("Full text-flow copy dropped data".into());
            }
            let mut drag = Value::Null;
            let mut paging = Value::Null;
            let mut links = Value::Null;
            let mut full_cell = Value::Null;
            let mut syntax_test = Value::Null;
            if ["mixed", "syntax-typescript", "syntax-tsx"].contains(&name.as_str()) {
                syntax_test = syntax_theme(&flow, root, window, &name).await?;
            }
            if name == "syntax-typescript-warm" || name == "syntax-tsx-warm" {
                let keyword = flow
                    .document
                    .buffer
                    .find("const value")
                    .ok_or("Warm code marker missing")?;
                let offset = flow.document.buffer[..keyword].chars().count();
                let expected = syntax_foreground(&flow, false, offset)?;
                until("warm syntax paint", || {
                    matches_paint(actual_foreground(&flow.widget, offset), expected)
                })
                .await?;
                syntax_test = json!({"warmPaintReady":true});
            }
            if name == "mixed" {
                let link = flow
                    .document
                    .links
                    .first()
                    .ok_or("Synthetic link missing")?;
                buffer.place_cursor(&buffer.iter_at_offset(link.characters.start as i32));
                smoke_input::request(root, json!({"action":"key","key":"open-link"})).await?;
                until("real Ctrl+Enter link", || opened.borrow().len() == 1).await?;
                glib::timeout_future(Duration::from_millis(100)).await;
                let (x, y) = point(&flow.widget, window, link.characters.start + 1)?;
                window_capture::capture(window, &root.join("before-link.png")).await?;
                smoke_input::request(root, json!({"action":"pointer","x":x,"y":y})).await?;
                glib::timeout_future(Duration::from_millis(100)).await;
                if opened.borrow().len() != 1 {
                    return Err("Plain selection opened a link".into());
                }
                // These are two distinct clicks, not a native word-selection
                // double-click. Keep the user's GTK timing unchanged.
                glib::timeout_future(Duration::from_millis(
                    flow.widget.settings().gtk_double_click_time().max(0) as u64 + 100,
                ))
                .await;
                smoke_input::request(root, json!({"action":"pointer","x":x,"y":y,"control":true}))
                    .await?;
                until("real Ctrl+click link", || opened.borrow().len() == 2).await?;
                if opened
                    .borrow()
                    .iter()
                    .any(|value| value != &link.destination)
                {
                    return Err("Link input changed the original destination".into());
                }
                let (edge_x, edge_y) =
                    character_point(&flow.widget, window, link.characters.end - 1, true)?;
                glib::timeout_future(Duration::from_millis(
                    flow.widget.settings().gtk_double_click_time().max(0) as u64 + 100,
                ))
                .await;
                smoke_input::request(
                    root,
                    json!({"action":"pointer","x":edge_x,"y":edge_y,"control":true}),
                )
                .await?;
                until("Unicode link trailing edge", || opened.borrow().len() == 3).await?;
                glib::timeout_future(Duration::from_millis(
                    flow.widget.settings().gtk_double_click_time().max(0) as u64 + 100,
                ))
                .await;
                smoke_input::request(
                    root,
                    json!({"action":"pointer","x":edge_x+24.0,"y":edge_y,"control":true}),
                )
                .await?;
                glib::timeout_future(Duration::from_millis(100)).await;
                if opened.borrow().len() != 3 {
                    return Err("Blank space activated an adjacent link".into());
                }
                links = json!({"ctrlEnter":true,"ctrlClick":true,"plainClickSelects":true,"originalDestination":true,"unicodeTrailingEdge":true,"blankSpaceIgnored":true});
                let begin = flow
                    .document
                    .buffer
                    .find("Begin cross-widget")
                    .ok_or("Missing drag marker")?;
                let end = flow
                    .document
                    .buffer
                    .find("End range 中文😀.")
                    .ok_or("Missing drag end")?
                    + "End range 中文😀.".len();
                let begin = flow.document.buffer[..begin].chars().count();
                let end = flow.document.buffer[..end].chars().count();
                buffer.place_cursor(&buffer.iter_at_offset(begin as i32));
                scroll.vadjustment().set_value(0.0);
                glib::timeout_future(Duration::from_millis(100)).await;
                let (x, y) = point(&flow.widget, window, begin)?;
                let (last_x, last_y) = point(&flow.widget, window, end)?;
                window_capture::capture(window, &root.join("before-drag.png")).await?;
                smoke_input::request(
                    root,
                    json!({"action":"pointer","x":x,"y":y,"dx":last_x-x,"dy":last_y-y}),
                )
                .await?;
                glib::timeout_future(Duration::from_millis(100)).await;
                window_capture::capture(window, &root.join("after-drag.png")).await?;
                until("cross-object pointer selection", || buffer.has_selection())
                    .await
                    .map_err(|e| format!("{e}; from ({x},{y}) to ({last_x},{last_y})"))?;
                drag = copy(&flow, root).await?;
                let selected = flow
                    .copied
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .map_err(Clone::clone)?
                    .text
                    .clone();
                if !selected.contains("$x^2$")
                    || !selected.contains("let value")
                    || !selected.contains("first\t中文😀")
                {
                    return Err(
                        "Physical drag did not cross paragraph, code, formula and table".into(),
                    );
                }
                for object in &flow.document.objects {
                    buffer.select_range(
                        &buffer.iter_at_offset(object.characters.start as i32),
                        &buffer.iter_at_offset(object.characters.start as i32 + 1),
                    );
                    copy(&flow, root).await?;
                    if flow
                        .copied
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .map_err(Clone::clone)?
                        .text
                        != object.alternative
                    {
                        return Err("Object-only copy lost its original text".into());
                    }
                }
                let table = flow
                    .objects
                    .iter()
                    .find(|object| {
                        find(object, &|w| w.widget_name() == "studio-cell-open").is_some()
                    })
                    .ok_or("Full-cell control missing")?;
                let read = find(table, &|w| w.widget_name() == "studio-cell-open")
                    .and_downcast::<gtk::Button>()
                    .ok_or("Full-cell button missing")?;
                if let Some(inner) = find(table, &|w| w.is::<gtk::ColumnView>())
                    .and_then(|view| view.ancestor(gtk::ScrolledWindow::static_type()))
                    .and_downcast::<gtk::ScrolledWindow>()
                {
                    inner.vadjustment().set_value(inner.vadjustment().upper());
                    glib::timeout_future(Duration::from_millis(120)).await;
                }
                let position = read
                    .compute_point(
                        window,
                        &gtk::graphene::Point::new(
                            read.width() as f32 / 2.0,
                            read.height() as f32 / 2.0,
                        ),
                    )
                    .ok_or("Full-cell control is not mapped")?;
                window_capture::capture(window, &root.join("before-full-cell.png")).await?;
                smoke_input::request(
                    root,
                    json!({"action":"pointer","x":position.x(),"y":position.y()}),
                )
                .await?;
                until("shared full-cell viewer", || cell_window().is_some()).await?;
                let dialog = cell_window().unwrap();
                until("active shared full-cell viewer", || dialog.is_active()).await?;
                let label = find(dialog.upcast_ref(), &|w| {
                    w.downcast_ref::<gtk::Label>()
                        .is_some_and(|l| l.text().ends_with("tail-cell 中文😀"))
                })
                .and_downcast::<gtk::Label>()
                .ok_or("Complete cell is not readable")?;
                if !label.is_selectable() || !label.label().contains("<b>styled</b>") {
                    return Err("Full-cell viewer lost native selection or formatting".into());
                }
                let expected_cell = label.text().to_string();
                let copy_button = find(dialog.upcast_ref(), &|w| {
                    w.is::<gtk::Button>()
                        && w.tooltip_text().as_deref() == Some("Copy complete cell text")
                })
                .and_downcast::<gtk::Button>()
                .ok_or("Complete cell Copy missing")?;
                copy_button.grab_focus();
                smoke_input::request(root, json!({"action":"key","key":"space"})).await?;
                let value = smoke_input::request(root, json!({"action":"read"})).await?;
                if value["text"].as_str() != Some(expected_cell.as_str()) {
                    return Err("Complete cell Copy truncated or changed the original text".into());
                }
                window_capture::capture(&dialog, &root.join("full-cell.png")).await?;
                smoke_input::request(root, json!({"action":"key","key":"close-window"})).await?;
                until("native full-cell close", || !dialog.is_mapped()).await?;
                smoke_input::request(
                    root,
                    json!({"action":"pointer","x":position.x(),"y":position.y()}),
                )
                .await?;
                until("shared full-cell reopen", || cell_window().is_some()).await?;
                let reopened = cell_window().unwrap();
                let opened_before = opened.borrow().len();
                window.set_content(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
                until("owner-unmap full-cell cleanup", || !reopened.is_mapped()).await?;
                buffer.place_cursor(&buffer.iter_at_offset(link.characters.start as i32));
                flow.widget
                    .activate_action("rich.open-link", None)
                    .map_err(|e| e.to_string())?;
                if opened.borrow().len() != opened_before {
                    return Err("Unmapped source opened a stale link".into());
                }
                window.set_content(Some(&scroll));
                until("reader remap", || flow.widget.is_mapped()).await?;
                smoke_input::request(root, json!({"action":"focus"})).await?;
                flow.widget.grab_focus();
                full_cell = json!({"sharedProductionWidget":true,"styledSelectable":true,"exactCopyBytes":expected_cell.len(),"nativeClose":true,"ownerUnmapCleanup":true,"unmappedLinkIgnored":true});
            }
            if name == "reference" {
                let table = flow.objects.first().ok_or("Reference table missing")?;
                let button = find(table, &|w| w.widget_name() == "probe-table-next")
                    .and_downcast::<gtk::Button>()
                    .ok_or("Reference table Next missing")?;
                let column = find(table, &|w| w.is::<gtk::ColumnView>())
                    .and_downcast::<gtk::ColumnView>()
                    .ok_or("Reference ColumnView missing")?;
                let before_line = first_table_line(&column).ok_or("First table row missing")?;
                buffer.place_cursor(&buffer.start_iter());
                scroll.vadjustment().set_value(0.0);
                glib::timeout_future(Duration::from_millis(100)).await;
                let before_scroll = scroll.vadjustment().value();
                let position = button
                    .compute_point(
                        window,
                        &gtk::graphene::Point::new(
                            button.width() as f32 / 2.0,
                            button.height() as f32 / 2.0,
                        ),
                    )
                    .ok_or("No mapped paging control")?;
                smoke_input::request(
                    root,
                    json!({"action":"pointer","x":position.x(),"y":position.y()}),
                )
                .await?;
                until("real table paging control", || {
                    first_table_line(&column).is_some_and(|line| line > before_line)
                })
                .await?;
                glib::timeout_future(Duration::from_millis(100)).await;
                let after_scroll = scroll.vadjustment().value();
                if (before_scroll - after_scroll).abs() > 1.0
                    || column.model().unwrap().n_items() > 16
                {
                    return Err(
                        "Table paging moved the outer document or exceeded its row budget".into(),
                    );
                }
                flow.widget.grab_focus();
                buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
                let after_copy = copy(&flow, root).await?;
                if after_copy["bytes"] != full["bytes"] {
                    return Err("Table paging lost original object text".into());
                }
                paging = json!({"actualPointer":true,"beforeLine":before_line,"afterLine":first_table_line(&column),"mountedRows":column.model().unwrap().n_items(),"outerScrollStable":true,"originalCopyRetained":true});
            }
            buffer.place_cursor(&buffer.start_iter());
            scroll.vadjustment().set_value(0.0);
            glib::timeout_future(Duration::from_millis(100)).await;
            window_capture::capture(window, &root.join(format!("{name}-flow.png"))).await?;
            if name == "syntax-mutate" || name == "syntax-replace" {
                if name == "syntax-mutate" {
                    buffer.set_text("Replacement 中文😀");
                } else {
                    let replacement = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
                    replacement.set_text("Replacement 中文😀");
                    flow.widget.set_buffer(Some(&replacement));
                }
                if flow.syntax_current() {
                    return Err("Syntax remained attached to changed/replaced source".into());
                }
                adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
                glib::timeout_future(Duration::from_millis(100)).await;
                let replacement = flow.widget.buffer();
                if (0..replacement.char_count())
                    .any(|i| actual_foreground(&flow.widget, i as usize).is_some())
                    || replacement.text(&replacement.start_iter(), &replacement.end_iter(), true)
                        != "Replacement 中文😀"
                {
                    return Err("A stale theme callback styled replacement data".into());
                }
                replacement.select_range(&replacement.start_iter(), &replacement.end_iter());
                flow.widget.clipboard().set_text("clipboard sentinel");
                flow.copied.borrow_mut().take();
                smoke_input::request(root, json!({"action":"key","key":"copy"})).await?;
                until("stale source copy rejection", || {
                    flow.copied.borrow().is_some()
                })
                .await?;
                if flow.copied.borrow().as_ref().unwrap().is_ok() {
                    return Err("Stale source copied old text".into());
                }
                if smoke_input::request(root, json!({"action":"read"})).await?["text"]
                    != "clipboard sentinel"
                {
                    return Err("Stale source replaced the clipboard".into());
                }
                adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
                syntax_test = json!({"sourceInvalidated":true,"staleThemeIgnored":true,"oldClipboardRejected":true});
            }
            if name == "syntax-unknown" || name == "syntax-long-line" {
                let block = flow
                    .syntax
                    .blocks
                    .first()
                    .ok_or("Plain code block missing")?;
                let expected = if name == "syntax-unknown" {
                    studio_markdown_native::syntax::Reason::UnknownLanguage
                } else {
                    studio_markdown_native::syntax::Reason::LongLine
                };
                if block.status != studio_markdown_native::syntax::Status::Plain(expected)
                    || buffer.slice(&buffer.start_iter(), &buffer.end_iter(), true)
                        != flow.document.buffer
                    || block.light.is_some()
                    || block.dark.is_some()
                    || (0..buffer.char_count()).any(|i| {
                        let actual = actual_foreground(&flow.widget, i as usize);
                        if block.characters.contains(&(i as usize)) {
                            let base = if adw::StyleManager::default().is_dark() {
                                flow.syntax.base_dark
                            } else {
                                flow.syntax.base_light
                            };
                            !base.is_some_and(|p| matches_paint(actual, p.foreground))
                        } else {
                            actual.is_some()
                        }
                    })
                {
                    return Err(
                        "Limited/unknown highlighting hid, rewrote or token-colored source".into(),
                    );
                }
                syntax_test = json!({"plainReadable":true,"themedPlainText":true,"completeCopy":full["bytes"],"reason":format!("{expected:?}")});
            }
            let after_actions_rss = gio::spawn_blocking(resident_kib)
                .await
                .map_err(|_| "RSS observation stopped")?;
            results.push(json!({"case":name,"fileBytes":total_bytes,"bufferBytes":flow.document.buffer.len(),"objects":flow.document.objects.len(),"tables":tables,"segments":flow.document.segments.len(),"rasterBytes":raster_bytes,"workerUs":worker_us,"syntaxWorkerUs":syntax_us,"syntaxHeapBytes":syntax_bytes,"syntaxTest":syntax_test,"residentKiB":{"beforeWorker":worker_rss_before,"afterWorker":worker_rss_after,"mapped":mapped_rss,"afterActions":after_actions_rss},"widgetConstructionUs":construct_us,"allocatedMs":allocated_ms,"settledOpenMs":open_ms,"openingTicks":opening_ticks,"openingLongestTickGapMs":opening_gap_ms,"copy":full,"drag":drag,"paging":paging,"links":links,"fullCell":full_cell}));
        }
        #[cfg(debug_assertions)]
        let image_anchors = text_flow::image_acceptance::run(window, root).await?;
        #[cfg(not(debug_assertions))]
        let image_anchors = json!({"skipped":"Debug fixture only"});
        #[cfg(debug_assertions)]
        let column_picker = text_flow::column_acceptance::run(window, root).await?;
        #[cfg(not(debug_assertions))]
        let column_picker = json!({"skipped":"Debug fixture only"});
        timer.remove();
        Ok(
            json!({"ok":true,"experimental":true,"productionEnabled":false,"cases":results,"imageAnchors":image_anchors,"columnPicker":column_picker}),
        )
    }
    pub fn launch() -> glib::ExitCode {
        codex_thread_studio_gnome::run_math_worker_if_requested();
        let root = PathBuf::from(
            std::env::args_os()
                .nth(1)
                .expect("Owned output directory required"),
        );
        let app = adw::Application::builder()
            .application_id(format!(
                "io.github.messon007.codexthreadstudio.FlowProbe{}",
                std::process::id()
            ))
            .build();
        app.connect_activate(move |app| {
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Native text-flow selection experiment")
                .default_width(1200)
                .default_height(850)
                .build();
            window.present();
            let root = root.clone();
            let app = app.clone();
            glib::spawn_future_local(async move {
                let result = run(&window, &root)
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                let data = serde_json::to_vec_pretty(&result).unwrap();
                let path = root.join("result.json");
                let _ = gio::spawn_blocking(move || std::fs::write(path, data)).await;
                window.close();
                app.quit();
            });
        });
        app.run_with_args::<&str>(&[])
    }
}
#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    gui::launch()
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Native text-flow probe requires GTK4/Linux");
    std::process::exit(2);
}
