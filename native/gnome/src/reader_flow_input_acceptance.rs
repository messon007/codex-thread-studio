//! Actual document actor, Wayland input and a private native drop destination.
use super::*;
#[allow(deprecated)]
fn outline_pixel(widget: &gtk::Widget) -> Result<([u8; 4], [u8; 4]), String> {
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(widget)).snapshot(
        &snapshot,
        widget.width() as f64,
        widget.height() as f64,
    );
    let node = snapshot
        .to_node()
        .ok_or("Selected object has no painted node")?;
    let texture = widget
        .native()
        .and_then(|n| n.renderer())
        .ok_or("Selected object has no renderer")?
        .render_texture(
            &node,
            Some(&gtk::graphene::Rect::new(
                0.,
                0.,
                widget.width() as f32,
                widget.height() as f32,
            )),
        );
    let mut download = gtk::gdk::TextureDownloader::new(&texture);
    download.set_format(gtk::gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = download.download_bytes();
    let offset = (texture.height() / 2) as usize * stride + 4;
    let actual = bytes.as_ref()[offset..offset + 4].try_into().unwrap();
    let color = widget
        .style_context()
        .lookup_color("accent_bg_color")
        .ok_or("Adwaita accent color missing")?;
    let expected = [color.red(), color.green(), color.blue(), color.alpha()]
        .map(|n| (n * 255.0).round() as u8);
    Ok((actual, expected))
}
fn same_color(a: [u8; 4], b: [u8; 4]) -> bool {
    a.into_iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 4)
}
impl Reader {
    pub async fn smoke_flow_input(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            crate::acceptance_wait::until(step, test).await
        }
        let expected =
            "Cross blocks\n\nFirst bold 中文😀 $x^2$ tail.\n\nSecond paragraph.\n\nplain code\n";
        crate::window_capture::ready(&self.window).await?;
        self.window.fullscreen();
        until("owned fullscreen reader", || {
            self.window.is_fullscreen()
                && self.window.width() == 1600
                && self.window.height() == 1000
        })
        .await?;
        let sink = gtk::TextView::builder()
            .editable(true)
            .height_request(110)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();
        sink.set_widget_name("studio-flow-input-sink");
        let destination = gtk::ScrolledWindow::builder()
            .height_request(110)
            .vexpand(false)
            .child(&sink)
            .build();
        self.widget.append(&destination);
        self.open_path(&root.join("project/input.md"));
        until("continuous input document", || {
            self.is_ready() && self.flow_preview.borrow().is_some()
        })
        .await?;
        let view = self.flow_preview.borrow().as_ref().unwrap().widget.clone();
        let document = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .document
            .clone();
        if document.quick.is_some() || document.objects.is_empty() {
            return Err(
                "Input fixture did not use a full document projection with child objects".into(),
            );
        }
        crate::window_capture::ready(&view).await?;
        let accessible = crate::smoke_input::request(
            root,
            serde_json::json!({"action":"reader-accessibility","pid":std::process::id(),"offsets":[document.objects[0].characters.start,document.characters-2]}),
        )
        .await?;
        if !accessible["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().any(|n| {
                n["showing"] == true
                    && n["role"] == "image"
                    && (n["name"] == "$x^2$" || n["description"] == "$x^2$")
            })
        }) {
            return Err(
                "Rendered formula has no accessible alternative in the owned native tree".into(),
            );
        }
        let text_node = accessible["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["showing"] == true && n["role"] == "text" && n["name"] == "Preview")
            .ok_or("Named accessible Preview missing")?;
        if text_node["characterCount"] != document.characters
            || text_node["text"] != document.buffer
            || text_node["fullText"] != document.buffer
        {
            return Err(format!(
                "Accessible counts/contents disagree with toolkit offsets: {text_node}"
            ));
        }
        for probe in text_node["probes"]
            .as_array()
            .ok_or("Accessible text granularity probes missing")?
        {
            let a = probe["start"]
                .as_u64()
                .ok_or("Accessible start offset missing")? as i32;
            let b = probe["end"]
                .as_u64()
                .ok_or("Accessible end offset missing")? as i32;
            let raw = view.buffer().slice(
                &view.buffer().iter_at_offset(a),
                &view.buffer().iter_at_offset(b),
                true,
            );
            if probe["text"] != raw.as_str() {
                return Err(format!(
                    "Accessible boundary text lost anchor slots: {probe}"
                ));
            }
        }
        let bounds = sink
            .compute_bounds(&self.window)
            .ok_or("Drop destination is unmapped")?;
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"pointer","x":bounds.x()+20.0,"y":bounds.y()+20.0}),
        )
        .await?;
        view.grab_focus();
        let buffer = view.buffer();
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"key","key":"document-start"}),
        )
        .await?;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"key","key":"select-document-end"}),
        )
        .await?;
        until("keyboard selection across all blocks", || {
            buffer
                .selection_bounds()
                .is_some_and(|(a, b)| a.offset() == 0 && b.offset() == buffer.char_count())
        })
        .await?;
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"copy"})).await?;
        let selected_accessible=crate::smoke_input::request(root,serde_json::json!({"action":"reader-accessibility","pid":std::process::id(),"phase":"selected"})).await?;
        let selected_text = selected_accessible["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["showing"] == true && n["role"] == "text" && n["name"] == "Preview")
            .ok_or("Selected accessible Preview missing")?;
        if selected_text["caret"] != document.characters
            || selected_text["selection"]
                != serde_json::json!([{"start":0,"end":document.characters}])
        {
            return Err(format!(
                "Accessible caret/selection lost anchor offsets: {selected_text}"
            ));
        }
        until("selected embedded objects", || {
            self.flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .objects
                .last()
                .is_some_and(|w| w.has_css_class("studio-flow-object-selected"))
        })
        .await?;
        if self.flow_preview.borrow().as_ref().unwrap().objects[0]
            .has_css_class("studio-flow-object-selected")
        {
            return Err("Inline formula received an inset selection border over its glyphs".into());
        }
        let copied =
            crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
        if copied["text"] != expected {
            return Err(format!("Cross-block copy differs: {copied}"));
        }
        let primary =
            crate::smoke_input::request(root, serde_json::json!({"action":"read-primary"})).await?;
        if primary["text"] != expected {
            return Err(format!("Cross-block primary differs: {primary}"));
        }
        let first = view.iter_location(&buffer.start_iter());
        let (x, y) = view.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            first.x() + 2,
            first.y() + first.height() / 2,
        );
        let start = view
            .compute_point(&self.window, &gtk::graphene::Point::new(x as f32, y as f32))
            .ok_or("Drag start is unmapped")?;
        crate::window_capture::capture(&self.window, &root.join("flow-input-before-drag.png"))
            .await?;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"pointer","x":start.x(),"y":start.y(),
            "dx":bounds.x()+20.0-start.x(),"dy":bounds.y()+20.0-start.y(),"hold_ms":160}),
        )
        .await?;
        until("native projected drop", || sink.buffer().char_count() > 0).await?;
        let dropped = sink
            .buffer()
            .text(&sink.buffer().start_iter(), &sink.buffer().end_iter(), true)
            .to_string();
        crate::window_capture::capture(&self.window, &root.join("flow-input-drop.png")).await?;
        if dropped != expected {
            return Err(format!(
                "Native document drag lost projected text: {dropped:?}"
            ));
        }
        // Let native click counting settle after DnD, then exercise ordinary
        // click/word/line selection and a new drag selection across blocks.
        glib::timeout_future(Duration::from_millis(450)).await;
        view.grab_focus();
        let offset = |needle: &str| -> Result<i32, String> {
            let at = document
                .buffer
                .find(needle)
                .ok_or("Input paragraph missing")?;
            Ok(document.buffer[..at].chars().count() as i32)
        };
        fn point(
            view: &gtk::TextView,
            window: &adw::ApplicationWindow,
            at: i32,
        ) -> Result<gtk::graphene::Point, String> {
            let rect = view.iter_location(&view.buffer().iter_at_offset(at));
            let (x, y) = view.buffer_to_window_coords(
                gtk::TextWindowType::Widget,
                rect.x() + 1,
                rect.y() + rect.height() / 2,
            );
            view.compute_point(window, &gtk::graphene::Point::new(x as f32, y as f32))
                .ok_or("Input text point missing".into())
        }
        let tail = point(&view, &self.window, offset("tail")? + 1)?;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"pointer","x":tail.x(),"y":tail.y()}),
        )
        .await?;
        until("document click clears selection", || {
            buffer.selection_bounds().is_none()
        })
        .await?;
        until("cleared embedded selection", || {
            self.flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .objects
                .iter()
                .all(|w| !w.has_css_class("studio-flow-object-selected"))
        })
        .await?;
        glib::timeout_future(Duration::from_millis(450)).await;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"pointer","x":tail.x(),"y":tail.y(),"clicks":2}),
        )
        .await?;
        until("document double click selects word", || {
            buffer
                .selection_bounds()
                .is_some_and(|(a, b)| buffer.text(&a, &b, false) == "tail")
        })
        .await?;
        glib::timeout_future(Duration::from_millis(450)).await;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"pointer","x":tail.x(),"y":tail.y(),"clicks":3}),
        )
        .await?;
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"copy"})).await?;
        let line = crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
        if line["text"].as_str().map(|s| s.trim_end_matches('\n'))
            != Some("First bold 中文😀 $x^2$ tail.")
        {
            return Err(format!("Document triple click lost its formula: {line}"));
        }
        glib::timeout_future(Duration::from_millis(450)).await;
        buffer.place_cursor(&buffer.start_iter());
        let begin = offset("First")?;
        let end = offset("Second paragraph.")? + "Second paragraph.".chars().count() as i32;
        let from = point(&view, &self.window, begin)?;
        let to = point(&view, &self.window, end)?;
        crate::smoke_input::request(root,serde_json::json!({"action":"pointer","x":from.x(),"y":from.y(),"dx":to.x()-from.x(),"dy":to.y()-from.y()})).await?;
        until("pointer selection across paragraphs", || {
            buffer
                .selection_bounds()
                .is_some_and(|(a, b)| a.offset() == begin && b.offset() == end)
        })
        .await?;
        let prefix = "First bold 中文😀 $x^2$ tail.\n\nSecond paragraph.";
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"copy"})).await?;
        let selected =
            crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
        if selected["text"] != prefix {
            return Err(format!(
                "Pointer selection changed rendered source: {selected}"
            ));
        }
        // Freeze the actual controller's transfer offer. A drag is independent
        // of primary clipboard ownership, but not of selection/source lifetime.
        let controller = view.observe_controllers();
        let drag = (0..controller.n_items())
            .find_map(|i| controller.item(i).and_downcast::<gtk::DragSource>())
            .ok_or("Projected drag controller absent")?;
        let rect = view.iter_location(&buffer.iter_at_offset(begin));
        let (px, py) = view.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            rect.x() + 2,
            rect.y() + rect.height() / 2,
        );
        let offer = || {
            drag.emit_by_name::<Option<gtk::gdk::ContentProvider>>(
                "prepare",
                &[&(px as f64), &(py as f64)],
            )
            .ok_or("Projected drag offer missing".to_owned())
        };
        async fn read(provider: &gtk::gdk::ContentProvider) -> Result<String, String> {
            use gtk::gdk::prelude::*;
            let stream = gio::MemoryOutputStream::new_resizable();
            provider
                .write_mime_type_future(
                    "text/plain;charset=utf-8",
                    &stream,
                    glib::Priority::DEFAULT,
                )
                .await
                .map_err(|e| e.to_string())?;
            stream
                .close_future(glib::Priority::DEFAULT)
                .await
                .map_err(|e| e.to_string())?;
            String::from_utf8(stream.steal_as_bytes().as_ref().to_vec()).map_err(|e| e.to_string())
        }
        let stale = offer().map_err(|e| format!("Initial offer: {e}"))?;
        view.primary_clipboard()
            .set_text("Foreign primary survives drag read");
        if read(&stale).await? != prefix {
            return Err("Drag offer depended on primary clipboard ownership".into());
        }
        let foreign =
            crate::smoke_input::request(root, serde_json::json!({"action":"read-primary"})).await?;
        if foreign["text"] != "Foreign primary survives drag read" {
            return Err("Drag read replaced another primary owner".into());
        }
        buffer.place_cursor(&buffer.start_iter());
        if read(&stale).await.is_ok() {
            return Err("Changed selection retained drag data".into());
        }
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        let retired = offer().map_err(|e| format!("Retirement offer: {e}"))?;
        let retired_objects = self.flow_preview.borrow().as_ref().unwrap().objects.clone();
        until("objects selected before retirement", || {
            retired_objects
                .iter()
                .any(|w| w.has_css_class("studio-flow-object-selected"))
        })
        .await?;
        self.set_mode(Mode::Source);
        until("Source retires drag view", || {
            self.is_ready() && self.state.borrow().mode == Mode::Source && !view.is_mapped()
        })
        .await?;
        if read(&retired).await.is_ok() {
            return Err("Retired reading view retained drag data".into());
        }
        until("retired object outline cleared", || {
            retired_objects
                .iter()
                .all(|w| !w.has_css_class("studio-flow-object-selected"))
        })
        .await?;
        self.set_mode(Mode::Preview);
        until("new Preview snapshot", || {
            self.is_ready()
                && self
                    .flow_preview
                    .borrow()
                    .as_ref()
                    .is_some_and(|v| v.widget != view)
        })
        .await?;
        let fresh = self.flow_preview.borrow().as_ref().unwrap().widget.clone();
        crate::window_capture::ready(&fresh).await?;
        let fresh_buffer = fresh.buffer();
        fresh_buffer.select_range(&fresh_buffer.start_iter(), &fresh_buffer.end_iter());
        let replaced_objects = self.flow_preview.borrow().as_ref().unwrap().objects.clone();
        until("objects selected before replacement", || {
            replaced_objects
                .last()
                .is_some_and(|w| w.has_css_class("studio-flow-object-selected"))
        })
        .await?;
        let rect = fresh.iter_location(&fresh_buffer.start_iter());
        let (x, y) = fresh.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            rect.x() + 2,
            rect.y() + rect.height() / 2,
        );
        let controls = fresh.observe_controllers();
        let controller = (0..controls.n_items())
            .find_map(|i| controls.item(i).and_downcast::<gtk::DragSource>())
            .ok_or("Fresh drag controller missing")?;
        let replaced = controller
            .emit_by_name::<Option<gtk::gdk::ContentProvider>>(
                "prepare",
                &[&(x as f64), &(y as f64)],
            )
            .ok_or("Fresh buffer drag offer missing")?;
        let replacement = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
        replacement.set_text("Private replacement buffer");
        fresh.set_buffer(Some(&replacement));
        if read(&replaced).await.is_ok() {
            return Err("Replaced buffer retained drag data".into());
        }
        until("replaced object outline cleared", || {
            replaced_objects
                .iter()
                .all(|w| !w.has_css_class("studio-flow-object-selected"))
        })
        .await?;
        sink.buffer().set_text("");
        self.open_path(&root.join("project/table.md"));
        until("indexed-table drag document", || {
            self.is_ready()
                && self.flow_preview.borrow().is_some()
                && self
                    .flow_tables
                    .borrow()
                    .values()
                    .any(|t| t.rows.n_items() > 0)
        })
        .await?;
        let visible_rows = self
            .flow_tables
            .borrow()
            .values()
            .next()
            .ok_or("Indexed table missing")?
            .rows
            .n_items();
        if visible_rows == 0 || visible_rows >= 70 {
            return Err(format!(
                "Table fixture did not hide off-page rows: {visible_rows}"
            ));
        }
        let table = self.flow_preview.borrow().as_ref().unwrap().widget.clone();
        let table_buffer = table.buffer();
        table.grab_focus();
        table_buffer.select_range(&table_buffer.start_iter(), &table_buffer.end_iter());
        let mut first = table_buffer.start_iter();
        table.scroll_to_iter(&mut first, 0.0, true, 0.0, 0.0);
        crate::window_capture::ready(&table).await?;
        let rect = table.iter_location(&table_buffer.start_iter());
        let (x, y) = table.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            rect.x() + 2,
            rect.y() + rect.height() / 2,
        );
        let at = table
            .compute_point(&self.window, &gtk::graphene::Point::new(x as f32, y as f32))
            .ok_or("Table drag text point missing")?;
        let target = sink
            .compute_bounds(&self.window)
            .ok_or("Table drag destination missing")?;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"pointer","x":at.x(),"y":at.y(),
            "dx":target.x()+20.0-at.x(),"dy":target.y()+20.0-at.y(),"hold_ms":160}),
        )
        .await?;
        until("indexed-table native drop", || {
            sink.buffer().char_count() > 0
        })
        .await?;
        let table_text =
            sink.buffer()
                .text(&sink.buffer().start_iter(), &sink.buffer().end_iter(), true);
        let mut expected_table = String::from("Table drag\n\nName\tValue");
        for i in 0..70 {
            expected_table.push_str(&format!("\nROW-{i}\tVALUE-{i}"));
        }
        if table_text != expected_table {
            return Err(format!(
                "Indexed-table drag omitted hidden rows or changed text: {table_text:?}"
            ));
        }
        crate::window_capture::capture(&self.window, &root.join("flow-input-table-drop.png"))
            .await?;
        let table_accessible=crate::smoke_input::request(root,serde_json::json!({"action":"reader-accessibility","pid":std::process::id(),"phase":"table"})).await?;
        let nodes = table_accessible["nodes"]
            .as_array()
            .ok_or("Native table accessibility tree missing")?;
        if !nodes
            .iter()
            .any(|n| n["showing"] == true && (n["name"] == "Name" || n["text"] == "Name"))
            || !nodes
                .iter()
                .any(|n| n["showing"] == true && (n["name"] == "ROW-0" || n["text"] == "ROW-0"))
        {
            return Err("Native table header/first row are not accessible".into());
        }
        let table_object = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .objects
            .iter()
            .find(|w| w.widget_name() == "studio-indexed-table")
            .cloned()
            .or_else(|| {
                self.flow_tables
                    .borrow()
                    .values()
                    .next()
                    .map(|t| t.widget.clone().upcast())
            })
            .ok_or("Selected table object missing")?;
        let style = adw::StyleManager::default();
        let scheme = style.color_scheme();
        let mut selection_pixels = Vec::new();
        for dark in [false, true] {
            style.set_color_scheme(if dark {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
            until("object selection theme", || style.is_dark() == dark).await?;
            table_buffer.select_range(&table_buffer.start_iter(), &table_buffer.end_iter());
            until("table object selected", || {
                table_object.has_css_class("studio-flow-object-selected")
            })
            .await?;
            crate::window_capture::ready(&table_object).await?;
            let before = (table_object.width(), table_object.height());
            let (selected, expected) = outline_pixel(&table_object)?;
            if !same_color(selected, expected) {
                return Err(format!("Selected object outline did not paint the native accent: dark={dark} {selected:?} vs {expected:?}"));
            }
            crate::window_capture::capture(
                &self.window,
                &root.join(if dark {
                    "flow-selected-dark.png"
                } else {
                    "flow-selected-light.png"
                }),
            )
            .await?;
            table_buffer.place_cursor(&table_buffer.start_iter());
            until("table selection outline cleared", || {
                !table_object.has_css_class("studio-flow-object-selected")
            })
            .await?;
            crate::window_capture::ready(&table_object).await?;
            let (cleared, _) = outline_pixel(&table_object)?;
            if same_color(cleared, selected)
                || before != (table_object.width(), table_object.height())
            {
                return Err("Object outline failed to clear or changed layout size".into());
            }
            selection_pixels.push(serde_json::json!({"dark":dark,"selected":selected,"cleared":cleared,"expected":expected,"unchangedSize":true}));
        }
        style.set_color_scheme(scheme);
        self.widget.remove(&destination);
        self.finish_close();
        Ok(
            serde_json::json!({"ok":true,"normalDocumentActor":true,"projectedCopy":true,
            "projectedPrimary":true,"nativeProjectedDrop":true,"keyboardCrossBlockSelection":true,
            "clickClearsSelection":true,"doubleClickWord":true,"tripleClickFormula":true,
            "pointerCrossBlockSelection":true,"foreignPrimaryRetained":true,
            "changedSelectionRejected":true,"replacedBufferRejected":true,"retiredViewRejected":true,"selectionPixels":selection_pixels,"accessibleTable":true,"retiredOutlinesCleared":true,"replacedOutlinesCleared":true,"inlineMathUnboxed":true,"accessibleFormula":true,"accessibleOffsets":true,"accessibleSelection":true,"selectedObjects":true,"clearedObjects":true,"indexedTableOffscreenRows":true,"visibleTableRows":visible_rows,"exportedTableRows":70}),
        )
    }
}
