//! Unsaved editing must guard comment opens and retain the pending quote on Save.
use super::*;
impl Reader {
    pub async fn smoke_flow_comments(
        self: &Rc<Self>,
        directory: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let at = std::time::Instant::now();
            while !test() {
                if at.elapsed() > Duration::from_secs(20) {
                    return Err(format!("Flow comment timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        fn cell(widget: &gtk::Widget) -> Option<gtk::Label> {
            if let Some(label) = widget
                .downcast_ref::<gtk::Label>()
                .filter(|l| l.text() == "one")
            {
                return Some(label.clone());
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(label) = cell(&widget) {
                    return Some(label);
                }
            }
            None
        }
        fn comment_button(widget: &gtk::Widget) -> Option<gtk::Button> {
            if widget.widget_name() == "studio-reader-comment" {
                return widget.clone().downcast().ok();
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(button) = comment_button(&widget) {
                    return Some(button);
                }
            }
            None
        }
        let text="# Flow comments\n\nBefore **中文😀** $x^2$.\n\n| A | B |\n|---|---|\n| one | two |\n\nAfter tail.\n".to_owned();
        let path = directory.join("project/flow-comments.md");
        let write = path.clone();
        let data = text.clone();
        gtk::gio::spawn_blocking(move || std::fs::write(write, data))
            .await
            .map_err(|_| "Flow fixture writer stopped")?
            .map_err(|e| e.to_string())?;
        self.open_path(&path);
        until("source index", || {
            self.is_ready() && self.state.borrow().path.as_ref() == Some(&path)
        })
        .await?;
        let revision = self.state.borrow().index.as_ref().unwrap().revision.clone();
        let captured = Rc::new(RefCell::new(None));
        let result = captured.clone();
        let previous = self.on_comment.replace(Some(Box::new(move |capture| {
            result.replace(Some(capture));
        })));
        let owner = Rc::new(RefCell::new("flow-owner".to_owned()));
        let previous_target =
            self.comments
                .replace(Some(Rc::new(crate::annotations_ui::CaptureTarget {
                    owner: Box::new({
                        let owner = owner.clone();
                        move || Some(owner.borrow().clone())
                    }),
                    capture: Box::new({
                        let (owner, captured) = (owner.clone(), captured.clone());
                        move |expected, capture| {
                            if *owner.borrow() != expected {
                                return Err(
                                "Conversation changed; select the passage again before commenting"
                                    .into(),
                            );
                            }
                            captured.replace(Some(capture));
                            Ok(())
                        }
                    }),
                })));
        let fragment = studio_markdown_native::Fragment {
            page: 0,
            document_id: "flow-comment-fixture".into(),
            revision: revision.clone(),
            metadata: studio_markdown_native::FragmentMetadata {
                start: 0,
                end: text.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: studio_markdown_native::SourceWindow {
                start: 0,
                end: text.len(),
                line: 1,
                end_line: text.lines().count(),
                text,
                projection: vec![],
                references: vec![],
            },
            header: None,
        };
        let remount = fragment.clone();
        let prepared = gtk::gio::spawn_blocking(move || crate::text_flow::prepare(&fragment))
            .await
            .map_err(|_| "Flow comment worker stopped")??;
        let flow = self.flow_view(prepared)?;
        // Exercise the application's real reader header and scroll container;
        // the default block renderer is restored before normal quote reopening.
        self.preview_scroll.set_child(Some(&flow.widget));
        until("mapped source", || {
            flow.widget.is_mapped() && flow.widget.width() > 0
        })
        .await?;
        let buffer = flow.widget.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        let expected = flow
            .document
            .select(0..flow.document.characters, 16384)?
            .text;
        // A mode switch after dispatch invalidates even an already admitted job.
        flow.widget
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        self.set_mode(Mode::Source);
        until("late selection discarded", || {
            !flow.commenting.get() && self.is_ready()
        })
        .await?;
        if captured.borrow().is_some() {
            return Err("Comment escaped a mode switch during extraction".into());
        }
        self.set_mode(Mode::Preview);
        until("preview restored", || self.is_ready()).await?;
        // Even the same revision's old page must remain expired after a mode
        // switch. Remapping its widget must not renew its action ownership.
        self.preview_scroll.set_child(Some(&flow.widget));
        until("expired flow remounted", || flow.widget.is_mapped()).await?;
        flow.widget
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        if flow.commenting.get() || captured.borrow().is_some() {
            return Err("A remapped old page renewed comment ownership".into());
        }
        // A fresh view captures the current mode epoch while retaining the
        // immutable source revision. Destination-switch checks use this owner.
        let prepared = gtk::gio::spawn_blocking(move || crate::text_flow::prepare(&remount))
            .await
            .map_err(|_| "Flow remount worker stopped")??;
        let flow = self.flow_view(prepared)?;
        self.preview_scroll.set_child(Some(&flow.widget));
        until("fresh flow mounted", || flow.widget.is_mapped()).await?;
        let buffer = flow.widget.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        flow.widget
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        *owner.borrow_mut() = "different-owner".into();
        until("destination switch rejected", || !flow.commenting.get()).await?;
        if captured.borrow().is_some() {
            return Err("Flow selection reached a different conversation".into());
        }
        *owner.borrow_mut() = "flow-owner".into();
        flow.widget
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        until("captured quote", || {
            captured.borrow().is_some() && !flow.commenting.get()
        })
        .await?;
        let capture = captured.borrow_mut().take().unwrap();
        let crate::annotations::Reopen::Document {
            root,
            path: target,
            anchor,
        } = crate::annotations::reopen(&capture.source)?
        else {
            return Err("Flow comment did not use the document provider".into());
        };
        if capture.excerpt != expected
            || anchor.text_kind != studio_markdown_native::quote::Kind::Flow
            || anchor.base_hash != revision
            || anchor.start_line != Some(1)
            || anchor.end_line != Some(9)
        {
            return Err(format!(
                "Flow comment changed its quote/source: {:?}",
                capture.source
            ));
        }
        let button =
            comment_button(self.widget.upcast_ref()).ok_or("Reader comment button missing")?;
        button.emit_clicked();
        until("toolbar projected capture", || {
            captured.borrow().is_some() && !flow.commenting.get()
        })
        .await?;
        let from_toolbar = captured.borrow_mut().take().unwrap();
        if from_toolbar.excerpt != capture.excerpt || from_toolbar.source != capture.source {
            return Err("Toolbar captured GTK placeholders instead of the projected quote".into());
        }
        let label = cell(flow.widget.upcast_ref()).ok_or("Flow cell comment leaf missing")?;
        label.grab_focus();
        label.select_region(0, -1);
        button.emit_clicked();
        let from_cell = captured
            .borrow_mut()
            .take()
            .ok_or("Toolbar missed the newest cell selection")?;
        if from_cell.excerpt != "one"
            || from_cell
                .source
                .pointer("/anchor/textKind")
                .and_then(serde_json::Value::as_str)
                != Some("preview")
            || from_cell
                .source
                .pointer("/anchor/startLine")
                .and_then(serde_json::Value::as_u64)
                != Some(7)
        {
            return Err("Toolbar reused an earlier projected selection for a cell".into());
        }
        flow.widget.grab_focus();
        buffer.select_range(&buffer.start_iter(), &buffer.start_iter());
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        button.emit_clicked();
        until("toolbar selection restored", || {
            captured.borrow().is_some() && !flow.commenting.get()
        })
        .await?;
        let restored = captured.borrow_mut().take().unwrap();
        if restored.excerpt != capture.excerpt || restored.source != capture.source {
            return Err(
                "Older cell selection overrode the newly selected projected passage".into(),
            );
        }
        if self.status.text().contains("Conversation changed") {
            return Err("Successful comment capture retained the previous rejection status".into());
        }
        crate::window_capture::capture_widget(
            &self.window,
            &self.widget,
            &directory.join("flow-comment.png"),
        )
        .await?;
        self.set_mode(Mode::Edit);
        until("edit loaded", || {
            self.is_ready() && self.state.borrow().editor.is_some()
        })
        .await?;
        self.edit_view.buffer().insert(
            &mut self.edit_view.buffer().end_iter(),
            "\nChanged source.\n",
        );
        self.save_page();
        until("new immutable revision", || {
            self.is_ready()
                && !self.state.borrow().dirty
                && self
                    .state
                    .borrow()
                    .index
                    .as_ref()
                    .is_some_and(|i| i.revision != revision)
        })
        .await?;
        self.set_mode(Mode::Preview);
        until("fresh preview", || self.is_ready()).await?;
        // Deliberately mount the stale projection: guards must reject its
        // actions even when a caller keeps the old GTK object visible.
        self.preview_scroll.set_child(Some(&flow.widget));
        until("old projection deliberately mapped", || {
            flow.widget.is_mapped()
        })
        .await?;
        flow.widget
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        label.select_region(0, -1);
        label
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        button.emit_clicked();
        if flow.commenting.get() || captured.borrow().is_some() {
            return Err("A mapped old revision created a new comment".into());
        }
        self.preview_scroll.set_child(Some(&self.preview));
        self.on_comment.replace(previous);
        self.comments.replace(previous_target);
        self.open_comment(
            Path::new(&root),
            Path::new(&target),
            anchor.clone(),
            &capture.excerpt,
        )?;
        until("flow quote verified", || self.is_ready()).await?;
        if self.state.borrow().comment_notice.is_some()
            || self.state.borrow().line != 1
            || self.state.borrow().mode != Mode::Preview
        {
            return Err(format!(
                "Flow quote failed immutable verification: {}",
                self.smoke_comment_debug()
            ));
        }
        self.finish_close();
        Ok(
            serde_json::json!({"crossBlockQuote":true,"tableAndMath":true,"exactSourceLines":true,"toolbarProjectedText":true,"toolbarLatestSelection":true,"realReaderContainer":true,"modeSwitchDuringWorkerRejected":true,"oldMappedPageRejected":true,"conversationSwitchRejected":true,"oldMappedRevisionRejected":true,"relocatedAfterSave":true,"documentProvider":true,"quoteBytes":expected.len(),"_capture":{"excerpt":capture.excerpt,"source":capture.source}}),
        )
    }
    pub async fn smoke_saved_selection_revision(
        self: &Rc<Self>,
        old: &gtk::Widget,
    ) -> Result<(), String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let started = std::time::Instant::now();
            while !test() {
                if started.elapsed() > Duration::from_secs(15) {
                    return Err("Saved selection revision timed out".into());
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        let quote = crate::content_widgets::selection(old, 16384)
            .ok_or("Old revision selection missing")?;
        let (generation, revision) = {
            let state = self.state.borrow();
            (
                state.generation,
                state
                    .index
                    .as_ref()
                    .ok_or("Revision fixture missing")?
                    .revision
                    .clone(),
            )
        };
        self.set_mode(Mode::Edit);
        until(|| self.is_ready() && self.state.borrow().editor.is_some()).await?;
        let buffer = self.edit_view.buffer();
        buffer.insert(
            &mut buffer.end_iter(),
            "\nSaved selection revision guard.\n",
        );
        self.save_page();
        until(|| self.is_ready() && !self.state.borrow().dirty).await?;
        {
            let state = self.state.borrow();
            if state.generation != generation
                || state
                    .index
                    .as_ref()
                    .is_none_or(|index| index.revision == revision)
            {
                return Err("Save fixture did not refresh the same document's revision".into());
            }
        }
        self.set_mode(Mode::Preview);
        until(|| self.is_ready()).await?;
        // Keep an old read-only leaf mounted, so rejection must depend on the
        // immutable revision rather than only GTK's mapped-widget check.
        if let Some(parent) = old.parent() {
            let parent = parent
                .downcast::<gtk::ScrolledWindow>()
                .map_err(|_| "Old code parent changed")?;
            parent.set_child(None::<&gtk::Widget>);
        }
        let window = adw::Window::builder()
            .content(old)
            .default_width(400)
            .default_height(200)
            .transient_for(&self.window)
            .build();
        window.present();
        until(|| old.is_mapped()).await?;
        if self.state.borrow().mode != Mode::Preview {
            window.close();
            return Err("Old revision fixture must remain in Preview".into());
        }
        let buffer = old
            .downcast_ref::<gtk::TextView>()
            .ok_or("Old code text view missing")?
            .buffer();
        let (a, z) = buffer
            .start_iter()
            .forward_search(&quote, gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Old code quote changed")?;
        buffer.select_range(&a, &z);
        if crate::content_widgets::selection(old, 16384).as_deref() != Some(&quote) {
            window.close();
            return Err("Remounted old revision lost its literal selection".into());
        }
        let captured = Rc::new(Cell::new(false));
        let result = captured.clone();
        let previous = self
            .on_comment
            .replace(Some(Box::new(move |_| result.set(true))));
        let activated = old.activate_action("reader-comment.capture", None);
        self.on_comment.replace(previous);
        window.close();
        activated.map_err(|e| e.to_string())?;
        if captured.get() {
            return Err("Old rendered selection acquired a new file revision".into());
        }
        Ok(())
    }
    pub async fn smoke_comment_view_switch(self: &Rc<Self>, line: usize) -> Result<(), String> {
        async fn ready(reader: &Reader) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !reader.is_ready() {
                if start.elapsed() > Duration::from_secs(15) {
                    return Err("Comment view switch timed out".into());
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        if self.current_line() != line {
            return Err("Verified Preview lost the quoted block line".into());
        }
        self.set_mode(Mode::Source);
        ready(self).await?;
        if self.current_line() != line {
            return Err("Preview→Source lost the verified quote location".into());
        }
        let heading = self
            .state
            .borrow()
            .index
            .as_ref()
            .unwrap()
            .toc
            .iter()
            .rev()
            .find(|h| h.line <= line)
            .map_or(line, |h| h.line);
        self.set_mode(Mode::Preview);
        ready(self).await?;
        if self.state.borrow().anchor_line != heading {
            return Err("Manual Preview switch bypassed the shared heading policy".into());
        }
        Ok(())
    }
    pub async fn smoke_comment_open_guard(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > Duration::from_secs(20) {
                    return Err("Comment edit guard timed out".into());
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        fn click(widget: &gtk::Widget, label: &str) -> bool {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if button.label().as_deref() == Some(label) {
                    button.emit_clicked();
                    return true;
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if click(&widget, label) {
                    return true;
                }
            }
            false
        }
        let dialog = || {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
                .find(|d| {
                    d.heading().as_deref() == Some("Save changes before opening another document?")
                        && d.is_visible()
                })
        };
        let path = root.join("guard.md");
        self.open_project_path(root, &path, None, None);
        until(|| self.is_ready()).await?;
        self.set_mode(Mode::Edit);
        until(|| self.is_ready() && self.state.borrow().editor.is_some()).await?;
        let buffer = self.edit_view.buffer();
        buffer.insert(
            &mut buffer.end_iter(),
            "\nExplicitly saved native comment guard.\n",
        );
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
        let anchor = studio_markdown_native::quote::Anchor {
            base_hash: "old-snapshot".into(),
            start_line: Some(3),
            end_line: Some(3),
            text_kind: studio_markdown_native::quote::Kind::Source,
        };
        let target = root.join("moved-source.md");
        self.open_comment(
            root,
            &target,
            anchor.clone(),
            "Target **中文😀** and &amp; code.  ",
        )?;
        until(|| dialog().is_some()).await?;
        if !click(dialog().unwrap().upcast_ref(), "Cancel") {
            return Err("Comment guard Cancel button missing".into());
        }
        if !self.state.borrow().dirty
            || self.state.borrow().path.as_ref() != Some(&path)
            || buffer.text(&buffer.start_iter(), &buffer.end_iter(), true) != text
        {
            return Err("Cancelling a comment open lost unsaved edits".into());
        }
        self.open_comment(root, &target, anchor, "Target **中文😀** and &amp; code.  ")?;
        until(|| dialog().is_some()).await?;
        if !click(dialog().unwrap().upcast_ref(), "Save") {
            return Err("Comment guard Save button missing".into());
        }
        until(|| self.is_ready() && self.state.borrow().path.as_ref() == Some(&target)).await?;
        if self.smoke_comment_position() != (8, false, true) {
            return Err("Saving edits lost the pending comment quote".into());
        }
        let saved =
            gio::spawn_blocking(move || std::fs::read_to_string(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Comment fixture worker stopped")??;
        if saved != text {
            return Err("Comment guard did not explicitly save exact edit text".into());
        }
        self.smoke_close();
        Ok(
            serde_json::json!({"cancelRetainedEdit":true,"explicitSave":true,"pendingQuoteRetained":true}),
        )
    }
}
