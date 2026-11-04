//! Real document requests and reader navigation, without replacing its widget.
use super::*;
impl Reader {
    pub(crate) async fn smoke_flow_pipeline(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(reader: &Reader, step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let at = std::time::Instant::now();
            while !test() {
                if at.elapsed() > Duration::from_secs(25) {
                    return Err(format!(
                        "Selectable preview {step}: {}",
                        reader.status.text()
                    ));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        async fn loaded(reader: &Rc<Reader>, path: &Path) -> Result<(), String> {
            reader.open_path(path);
            until(reader, "loaded through document actor", || {
                reader.is_ready()
                    && reader.state.borrow().path.as_deref() == Some(path)
                    && reader.flow_preview.borrow().is_some()
            })
            .await?;
            crate::window_capture::next_frame(&reader.widget).await
        }
        fn choice(widget: &gtk::Widget) -> Option<gtk::CheckButton> {
            if let Some(check) = widget
                .downcast_ref::<gtk::CheckButton>()
                .filter(|c| c.is_mapped() && c.label().as_deref() == Some("Column 0"))
            {
                return Some(check.clone());
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(check) = choice(&widget) {
                    return Some(check);
                }
            }
            None
        }
        fn code_copy(widget: &gtk::Widget) -> Option<gtk::Button> {
            if widget.widget_name() == "studio-code-copy" {
                return widget.clone().downcast().ok();
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(button) = code_copy(&widget) {
                    return Some(button);
                }
            }
            None
        }
        async fn clipboard(root: &Path, expected: &str) -> Result<(), String> {
            let started = std::time::Instant::now();
            loop {
                let value =
                    crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
                if value["text"].as_str() == Some(expected) {
                    return Ok(());
                }
                if started.elapsed() > Duration::from_secs(5) {
                    return Err(format!("Code control changed clipboard text: {value:?}"));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
        }
        if !self.continuous_preview.get() {
            return Err("Continuous-preview validation switch is not enabled".into());
        }
        let repeated = self.smoke_repeated_math(root).await?;
        let code_surfaces = self.smoke_code_surfaces(root).await?;
        let small = root.join("project/pipeline.md");
        loaded(self, &small).await?;
        if self.blocks.n_items() != 0 {
            return Err("Selectable request mounted the ordinary block model".into());
        }
        let (view, span) = {
            let value = self.flow_preview.borrow();
            let view = value.as_ref().unwrap();
            if self.preview_scroll.child().as_ref() != Some(view.widget.upcast_ref()) {
                return Err("Document actor did not mount its selectable view".into());
            }
            let span = view
                .document
                .spans
                .iter()
                .find(|s| matches!(s.style, studio_markdown_native::flow::Style::Code(_)))
                .ok_or("Code source span missing")?
                .characters
                .clone();
            (view.widget.clone(), span)
        };
        let buffer = view.buffer();
        buffer.select_range(
            &buffer.start_iter(),
            &buffer.iter_at_offset(span.end as i32),
        );
        view.emit_copy_clipboard();
        until(self, "native projected Copy", || {
            self.flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .copied
                .borrow()
                .is_some()
        })
        .await?;
        let copied = view
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Native clipboard empty")?;
        let expected = "Pipeline\n\nFirst Unicode 中文😀.\n\nSecond paragraph.\n\nlet value = 7;\n";
        if copied.as_str() != expected {
            return Err(format!("Cross-block Copy changed its source: {copied:?}"));
        }
        let old_code = code_copy(view.upcast_ref()).ok_or("Native code Copy control missing")?;
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        if !old_code.grab_focus() {
            return Err("Code Copy control could not receive keyboard focus".into());
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"space"}))
            .await?;
        clipboard(root, "let value = 7;\n").await?;
        if buffer
            .selection_bounds()
            .map(|(a, b)| (a.offset(), b.offset()))
            != Some((0, span.end as i32))
        {
            return Err("Code Copy changed the projected selection".into());
        }
        crate::window_capture::next_frame(&view).await?;
        let mut header = old_code.parent().ok_or("Code header parent missing")?;
        while header.widget_name() != "studio-code-header" {
            header = header.parent().ok_or("Code header widget missing")?;
        }
        let anchor = buffer.iter_at_offset(span.start.saturating_sub(1) as i32);
        let mark = buffer.create_mark(None, &anchor, true);
        view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.2);
        let visible = until(self, "visible code header", || {
            header.compute_bounds(&view).is_some_and(|r| {
                r.y() >= 0.0 && r.height() > 0.0 && r.y() + r.height() <= view.height() as f32
            })
        })
        .await;
        buffer.delete_mark(&mark);
        if let Err(error) = visible {
            crate::window_capture::capture(&self.window, &root.join("code-header-failed.png"))
                .await?;
            return Err(format!(
                "{error}; header={:?}, view={}x{}, visible={:?}, childMatches={}, scroll=({},{},{}), textScroll={:?}, blockScroll={:?}",
                header.compute_bounds(&view),
                view.width(),
                view.height(),
                view.visible_rect(),
                self.preview_scroll.child().as_ref()==Some(view.upcast_ref()),
                self.preview_scroll.vadjustment().value(),
                self.preview_scroll.vadjustment().upper(),
                self.preview_scroll.vadjustment().page_size(),
                view.vadjustment().map(|a|(a.value(),a.upper(),a.page_size(),a==self.preview_scroll.vadjustment())),
                self.preview.vadjustment().map(|a|(a.value(),a.upper(),a.page_size(),a==self.preview_scroll.vadjustment()))
            ));
        }
        crate::window_capture::next_frame(&view).await?;
        let bounds = header
            .compute_bounds(&view)
            .ok_or("Code header has no text-view bounds")?;
        let first = view.iter_location(&buffer.iter_at_offset(span.start as i32));
        let (_, glyph_y) =
            view.buffer_to_window_coords(gtk::TextWindowType::Widget, first.x(), first.y());
        if glyph_y as f32 + 1.0 < bounds.y() + bounds.height() {
            return Err("Native code header overlaps the selectable code body".into());
        }
        crate::window_capture::capture(&self.window, &root.join("pipeline-code.png")).await?;
        let old_scope = self.flow_options().unwrap().comments.unwrap().current;
        let target = self
            .state
            .borrow()
            .index
            .as_ref()
            .unwrap()
            .toc
            .iter()
            .find(|h| h.label == "Target section")
            .ok_or("Target heading missing")?
            .line;
        self.navigate(Navigation::Line(target));
        until(self, "contents jump", || {
            self.is_ready()
                && self
                    .flow_preview
                    .borrow()
                    .as_ref()
                    .is_some_and(|v| v.widget != view)
        })
        .await?;
        if old_scope() {
            return Err("A replaced page retained comment ownership".into());
        }
        view.clipboard()
            .set_text("New clipboard survives expired code control 中文😀");
        old_code.emit_clicked();
        clipboard(root, "New clipboard survives expired code control 中文😀").await?;
        if self.current_line() != target {
            return Err(format!(
                "Preview cursor mapped to {} instead of heading {target}",
                self.current_line()
            ));
        }
        self.set_mode(Mode::Source);
        until(self, "Source switch", || {
            self.is_ready() && self.state.borrow().mode == Mode::Source
        })
        .await?;
        self.set_mode(Mode::Edit);
        until(self, "Edit switch", || {
            self.is_ready() && self.state.borrow().editor.is_some()
        })
        .await?;
        let editor = self.edit_view.buffer();
        editor.begin_user_action();
        editor.insert(&mut editor.end_iter(), "\nUnsaved pipeline edit 中文😀\n");
        editor.end_user_action();
        until(self, "dirty edit", || self.state.borrow().dirty).await?;
        self.set_mode(Mode::Preview);
        until(self, "saved Preview with retained dirty Edit", || {
            self.is_ready() && self.flow_preview.borrow().is_some()
        })
        .await?;
        if !self.state.borrow().dirty {
            return Err("Preview switch lost the dirty page".into());
        }
        let saved_code = {
            let flow = self.flow_preview.borrow();
            code_copy(flow.as_ref().unwrap().widget.upcast_ref())
                .ok_or("Saved Preview lost code controls while Edit is dirty")?
        };
        saved_code.emit_clicked();
        clipboard(root, "let value = 7;\n").await?;
        self.set_mode(Mode::Edit);
        until(self, "retained editor", || {
            self.is_ready() && self.state.borrow().mode == Mode::Edit
        })
        .await?;
        if !editor
            .text(&editor.start_iter(), &editor.end_iter(), true)
            .contains("Unsaved pipeline edit")
        {
            return Err("Edit round trip replaced the buffer".into());
        }
        // Undo is explicit; this fixture never saves modifications to the source.
        editor.undo();
        until(self, "undo restores saved page", || {
            !self.state.borrow().dirty && !editor.is_modified()
        })
        .await?;
        self.set_mode(Mode::Preview);
        until(self, "clean Preview", || {
            self.is_ready() && self.flow_preview.borrow().is_some()
        })
        .await?;
        crate::window_capture::capture(&self.window, &root.join("pipeline-small.png")).await?;
        loaded(self, &root.join("project/empty-code.md")).await?;
        let empty = {
            let flow = self.flow_preview.borrow();
            code_copy(flow.as_ref().unwrap().widget.upcast_ref())
                .ok_or("Empty code block lost its controls")?
        };
        if !empty.grab_focus() {
            return Err("Empty code Copy could not receive focus".into());
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"space"}))
            .await?;
        clipboard(root, "").await?;
        crate::window_capture::capture(&self.window, &root.join("pipeline-empty-code.png")).await?;
        let empty_view = self.flow_preview.borrow().as_ref().unwrap().widget.clone();
        let replacement = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
        replacement.set_text("A different native text buffer 中文😀");
        empty_view.set_buffer(Some(&replacement));
        empty_view
            .clipboard()
            .set_text("New buffer clipboard survives old control 中文😀");
        empty.emit_clicked();
        clipboard(root, "New buffer clipboard survives old control 中文😀").await?;
        let wide = root.join("project/wide.md");
        loaded(self, &wide).await?;
        until(self, "wide indexed table", || {
            self.flow_tables
                .borrow()
                .get(&0)
                .is_some_and(|t| t.rows.n_items() > 0)
        })
        .await?;
        let table = self.flow_tables.borrow().get(&0).cloned().unwrap();
        if self.state.borrow().index.as_ref().unwrap().tables[0]
            .columns
            .len()
            != 256
        {
            return Err("Wide table lost source columns".into());
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        if !table.menu.grab_focus() {
            return Err("Column menu could not receive focus".into());
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"space"}))
            .await?;
        let popup = table.menu.popover().ok_or("Wide table picker missing")?;
        until(self, "mapped wide column choice", || {
            choice(popup.upcast_ref()).is_some()
        })
        .await?;
        if !choice(popup.upcast_ref()).unwrap().grab_focus() {
            return Err("Column checkbox could not receive focus".into());
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"space"}))
            .await?;
        until(self, "original-index column filter", || {
            table.rows.n_items() > 0
                && table.view.columns().n_items() == 255
                && table
                    .view
                    .columns()
                    .item(0)
                    .and_downcast::<gtk::ColumnViewColumn>()
                    .is_some_and(|c| c.title().as_deref() == Some("Column 1"))
                && table.menu.label().as_deref() == Some("Columns 255/256")
                && table
                    .rows
                    .item(0)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .is_some_and(|r| {
                        r.borrow::<Vec<studio_markdown_native::Cell>>()
                            .first()
                            .is_some_and(|c| c.title() == "Value 1")
                    })
        })
        .await?;
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"escape"}))
            .await?;
        crate::window_capture::capture(&self.window, &root.join("pipeline-wide.png")).await?;
        let report = root.join("project/large.md");
        let ticks = Rc::new(Cell::new(0u64));
        let gap = Rc::new(Cell::new(0u128));
        let (t, g) = (ticks.clone(), gap.clone());
        let mut last = std::time::Instant::now();
        let heartbeat = glib::timeout_add_local(Duration::from_millis(10), move || {
            let now = std::time::Instant::now();
            g.set(g.get().max(now.duration_since(last).as_millis()));
            t.set(t.get() + 1);
            last = now;
            glib::ControlFlow::Continue
        });
        let start = std::time::Instant::now();
        let result = loaded(self, &report).await;
        let open_ms = start.elapsed().as_millis();
        heartbeat.remove();
        result?;
        let first_table_page = self
            .state
            .borrow()
            .index
            .as_ref()
            .unwrap()
            .tables
            .first()
            .ok_or("Real report contains no indexed tables")?
            .first_page;
        self.navigate(Navigation::Page(first_table_page));
        until(self, "real report table", || {
            self.is_ready()
                && !self.flow_tables.borrow().is_empty()
                && self
                    .flow_tables
                    .borrow()
                    .values()
                    .any(|t| t.rows.n_items() > 0)
        })
        .await?;
        let (bytes, pages) = {
            let s = self.state.borrow();
            let i = s.index.as_ref().unwrap();
            (i.source_bytes, i.pages.len())
        };
        self.next.emit_clicked();
        until(self, "next bounded context", || {
            self.is_ready() && self.flow_preview.borrow().is_some()
        })
        .await?;
        until(self, "next context tables settled", || {
            let tables = self.flow_tables.borrow();
            !tables.is_empty() && tables.values().all(|t| t.rows.n_items() > 0)
        })
        .await?;
        crate::window_capture::capture(&self.window, &root.join("pipeline-report.png")).await?;
        let mixed = self.smoke_mixed_preview(root).await?;
        let dense = self.smoke_dense_preview(root).await?;
        self.finish_close();
        Ok(
            serde_json::json!({"ok":true,"realDocumentRequests":true,"densePreview":dense,"repeatedMath":repeated,"mixedPreview":mixed,"boundedContexts":true,"crossBlockCopy":true,"nativeCodeCopy":true,"codeSurfaces":code_surfaces,"independentCodeClipboard":true,"codeCopyRetainsSelection":true,"codeHeaderNoOverlap":true,"codeHeaderVisible":true,"emptyCodeCopy":true,"replacedCodeBufferRejected":true,"expiredCodeControlRejected":true,"savedCodeCopyWhileEditDirty":true,"contentsJump":true,"pageEpochRejected":true,"sourceEditPreview":true,"dirtyEditorRetained":true,"wideColumns":256,"originalIndexFilter":true,"sourceBytes":bytes,"indexPages":pages,"realReportOpenMs":open_ms,"openingMainLoopTicks":ticks.get(),"openingMaximumMainLoopGapMs":gap.get()}),
        )
    }
}
