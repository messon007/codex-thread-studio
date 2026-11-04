//! Dense deferred content through the ordinary document actor and native queues.
use super::*;
struct Pulse {
    source: Option<glib::SourceId>,
    ticks: Rc<Cell<u64>>,
    gap: Rc<Cell<u128>>,
    last: Rc<Cell<std::time::Instant>>,
    active: Rc<Cell<usize>>,
}
impl Pulse {
    fn new(reader: &Rc<Reader>) -> Self {
        let (ticks, gap, active) = (
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        );
        let (t, g, a, reader) = (
            ticks.clone(),
            gap.clone(),
            active.clone(),
            Rc::downgrade(reader),
        );
        let last = Rc::new(Cell::new(std::time::Instant::now()));
        let previous = last.clone();
        let source = glib::timeout_add_local(Duration::from_millis(10), move || {
            let now = std::time::Instant::now();
            g.set(
                g.get()
                    .max(now.duration_since(previous.replace(now)).as_millis()),
            );
            t.set(t.get() + 1);
            if let Some(reader) = reader.upgrade() {
                a.set(a.get().max(reader.flow_diagrams.smoke_work_counts().0));
            }
            glib::ControlFlow::Continue
        });
        Self {
            source: Some(source),
            ticks,
            gap,
            last,
            active,
        }
    }
    fn maximum_gap(&self) -> u128 {
        // A blocking paint can finish before the overdue timer is dispatched.
        // Include the unfinished interval instead of reporting zero at that boundary.
        self.gap.get().max(self.last.get().elapsed().as_millis())
    }
}
impl Drop for Pulse {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
    }
}
impl Reader {
    pub(crate) async fn smoke_dense_preview(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(reader: &Reader, step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let at = std::time::Instant::now();
            while !test() {
                if at.elapsed() > Duration::from_secs(25) {
                    return Err(format!("Dense preview {step}: {}", reader.status.text()));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        fn loaded(widget: &gtk::Widget) -> bool {
            crate::images_ui::loaded_count(widget, false) > 0
        }
        fn widest_loaded_picture(widget: &gtk::Widget) -> i32 {
            let mut width = widget
                .downcast_ref::<crate::scaled_picture::ScaledPicture>()
                .filter(|p| p.paintable().is_some())
                .map_or(0, |p| p.width());
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                width = width.max(widest_loaded_picture(&widget));
            }
            width
        }
        fn fully_visible(widget: &gtk::Widget, view: &gtk::TextView) -> bool {
            widget
                .compute_bounds(view)
                .is_some_and(|r| r.y() >= 0.0 && r.y() + r.height() <= view.height() as f32)
        }
        fn retained(widget: &gtk::Widget) -> bool {
            if widget
                .downcast_ref::<crate::scaled_picture::ScaledPicture>()
                .is_some_and(|p| p.paintable().is_some() || p.vector().is_some())
            {
                return true;
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if retained(&widget) {
                    return true;
                }
            }
            false
        }
        let mut rows = Vec::new();
        for case in ["images", "diagrams"] {
            let path = root.join(format!("project/dense-{case}.md"));
            let pulse = Pulse::new(self);
            let start = std::time::Instant::now();
            self.open_path(&path);
            until(self, "document actor", || {
                self.is_ready()
                    && self.state.borrow().path.as_ref() == Some(&path)
                    && self.flow_preview.borrow().is_some()
            })
            .await?;
            let (view, objects, last_offset) = {
                let flow = self.flow_preview.borrow();
                let flow = flow.as_ref().unwrap();
                (
                    flow.widget.clone(),
                    flow.objects.clone(),
                    flow.document
                        .objects
                        .last()
                        .ok_or("Dense projection empty")?
                        .characters
                        .start,
                )
            };
            if objects.len() != 64 {
                return Err(format!(
                    "Dense {case} retained {} instead of 64 objects",
                    objects.len()
                ));
            }
            until(self, "first visible native texture", || {
                objects.iter().any(|w| {
                    loaded(w) && crate::viewport::intersects(w, &crate::viewport::scopes(w))
                })
            })
            .await?;
            let first_ms = start.elapsed().as_millis();
            crate::window_capture::next_frame(&self.widget).await?;
            let first_loaded = objects.iter().filter(|w| loaded(w)).count();
            if first_loaded >= 64 {
                return Err(format!(
                    "Dense {case} eagerly loaded every offscreen object"
                ));
            }
            if case == "images"
                && objects.iter().map(widest_loaded_picture).max().unwrap_or(0)
                    < 320.min(view.width() - 48)
            {
                return Err("Dense image collapsed below its intrinsic width".into());
            }
            crate::window_capture::capture(
                &self.window,
                &root.join(format!("dense-{case}-first.png")),
            )
            .await?;
            let at = std::time::Instant::now();
            let buffer = view.buffer();
            if !view.grab_focus() {
                return Err("Dense text view cannot receive keyboard focus".into());
            }
            crate::smoke_input::request(
                root,
                serde_json::json!({"action":"key","key":"document-end"}),
            )
            .await?;
            until(self, "native Ctrl+End cursor", || {
                buffer.iter_at_mark(&buffer.get_insert()).offset() as usize >= last_offset
            })
            .await?;
            let last = objects.last().unwrap();
            let settled = until(self, "last texture and prior texture release", || {
                loaded(last) && !retained(&objects[0]) && fully_visible(last, &view)
            })
            .await;
            if let Err(error) = settled {
                crate::window_capture::capture(
                    &self.window,
                    &root.join(format!("dense-{case}-failed.png")),
                )
                .await?;
                return Err(format!("{error}; case={case}, firstRetained={}, lastLoaded={}, lastBounds={:?}, view={:?}, scroll=({},{},{}), work={:?}",retained(&objects[0]),loaded(last),last.compute_bounds(&view),view.visible_rect(),self.preview_scroll.vadjustment().value(),self.preview_scroll.vadjustment().upper(),self.preview_scroll.vadjustment().page_size(),self.flow_diagrams.smoke_work_counts()));
            }
            let last_ms = at.elapsed().as_millis();
            crate::window_capture::capture(
                &self.window,
                &root.join(format!("dense-{case}-last.png")),
            )
            .await?;
            let adjustment = self.preview_scroll.vadjustment();
            let end_value = adjustment.value();
            crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"page-up"}))
                .await?;
            until(self, "Page Up supersedes end hold", || {
                adjustment.value() < end_value - adjustment.page_size() * 0.2
                    && buffer.cursor_position() < buffer.char_count()
            })
            .await?;
            crate::smoke_input::request(
                root,
                serde_json::json!({"action":"key","key":"document-start"}),
            )
            .await?;
            until(self, "native Ctrl+Home restores first texture", || {
                buffer.cursor_position() == 0
                    && loaded(&objects[0])
                    && !retained(last)
                    && adjustment.value() <= adjustment.lower() + 1.0
            })
            .await?;
            crate::smoke_input::request(
                root,
                serde_json::json!({"action":"key","key":"document-end"}),
            )
            .await?;
            until(self, "second native Ctrl+End", || {
                buffer.cursor_position() == buffer.char_count()
                    && loaded(last)
                    && !retained(&objects[0])
                    && fully_visible(last, &view)
            })
            .await?;
            self.set_mode(Mode::Source);
            until(self, "Source cleanup", || {
                self.is_ready()
                    && self.state.borrow().mode == Mode::Source
                    && objects.iter().all(|w| !retained(w))
            })
            .await?;
            if self.flow_diagrams.smoke_work_counts() != (0, 0) || pulse.active.get() > 2 {
                return Err("Dense diagram work exceeded its slots or survived Source".into());
            }
            rows.push(serde_json::json!({"kind":case,"objects":64,"firstTextureReadyMs":first_ms,"initialLoaded":first_loaded,"lastObjectScrollMs":last_ms,"mainLoopTicks":pulse.ticks.get(),"maximumWorkflowGapIncludingScreenshotsMs":pulse.maximum_gap(),"maximumObservedActiveDiagrams":pulse.active.get(),"offscreenDeferred":true,"oldPixelsReleased":true,"nativeEndKey":true,"nativeHomeKey":true,"pageUpOverridesEnd":true,"repeatedEnd":true,"sourceCleanup":true}));
            let path = root.join("dense-progress.json");
            let bytes = serde_json::to_vec_pretty(&rows).unwrap();
            gio::spawn_blocking(move || std::fs::write(path, bytes))
                .await
                .map_err(|_| "Dense progress writer stopped")?
                .map_err(|e| e.to_string())?;
            self.set_mode(Mode::Preview);
            until(self, "Preview restoration", || {
                self.is_ready() && self.flow_preview.borrow().is_some()
            })
            .await?;
        }
        Ok(
            serde_json::json!({"ok":true,"cases":rows,"realEnginesAndImages":true,"normalDocumentActor":true}),
        )
    }
}
impl Reader {
    pub(crate) async fn smoke_mixed_preview(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        let pulse = Pulse::new(self);
        let started = std::time::Instant::now();
        let path = root.join("project/dense-mixed.md");
        self.open_path(&path);
        crate::acceptance_wait::until("mixed formula/table document", || {
            self.is_ready()
                && self.state.borrow().path.as_ref() == Some(&path)
                && self.flow_preview.borrow().is_some()
        })
        .await?;
        let (view, document, preparation, objects) = {
            let flow = self.flow_preview.borrow();
            let flow = flow.as_ref().unwrap();
            (
                flow.widget.clone(),
                flow.document.clone(),
                flow.preparation,
                flow.objects.clone(),
            )
        };
        let formulas = document
            .objects
            .iter()
            .filter(|o| {
                matches!(
                    o.kind,
                    studio_markdown_native::flow::ObjectKind::Math { .. }
                )
            })
            .count();
        if formulas != 16 {
            return Err(format!(
                "Mixed fixture lost its inline formulas: {formulas}"
            ));
        }
        crate::acceptance_wait::until("mixed indexed table pages", || {
            let tables = self.flow_tables.borrow();
            tables.len() == 8
                && tables
                    .values()
                    .all(|t| t.rows.n_items() > 0 && t.rows.n_items() <= 50)
        })
        .await?;
        let ready_ms = started.elapsed().as_millis();
        let ticks = pulse.ticks.get();
        let gap = pulse.maximum_gap();
        view.buffer().set_modified(false);
        let checkpoint = serde_json::to_vec_pretty(&serde_json::json!({"readyMs":ready_ms,"openingMainLoopTicks":ticks,"openingMaximumGapMs":gap,"renderUs":preparation.0,"renderedObjects":preparation.1,"reusedObjects":preparation.2})).unwrap();
        let checkpoint_path = root.join("mixed-progress.json");
        gio::spawn_blocking(move || std::fs::write(checkpoint_path, checkpoint))
            .await
            .map_err(|_| "Mixed progress writer stopped")?
            .map_err(|e| e.to_string())?;
        crate::acceptance_wait::until("first mixed table formulas painted", || {
            let (visible, ready) =
                crate::inline_math::smoke_visible_formulas(view.upcast_ref(), "$z^2$");
            visible > 0 && visible == ready
        })
        .await?;
        let first_paint_ms = started.elapsed().as_millis();
        let paint_gap = pulse.maximum_gap();
        let (active_tables, untouched_tables, initial_bindings) = {
            let tables = self.flow_tables.borrow();
            (
                tables.values().filter(|t| t.view.model().is_some()).count(),
                tables.values().filter(|t| t.bindings.get() == 0).count(),
                tables.values().map(|t| t.bindings.get()).sum::<usize>(),
            )
        };
        if active_tables == 0 || active_tables >= 8 || untouched_tables == 0 {
            return Err(format!("Mixed tables eagerly bound offscreen models: active={active_tables}, untouched={untouched_tables}"));
        }
        crate::window_capture::capture(&self.window, &root.join("dense-mixed-first.png")).await?;
        let first = self
            .flow_tables
            .borrow()
            .get(&0)
            .cloned()
            .ok_or("First mixed table missing")?;
        first.next.emit_clicked();
        crate::acceptance_wait::until("mixed first table second page", || {
            first.start.get() == 50 && first.rows.n_items() == 10
        })
        .await?;
        let column = first
            .view
            .columns()
            .item(0)
            .and_downcast::<gtk::ColumnViewColumn>()
            .ok_or("Mixed first column missing")?;
        column.set_fixed_width(306);
        crate::window_capture::next_frame(&first.widget).await?;
        first.scroll.hadjustment().set_value(40.);
        first.scroll.vadjustment().set_value(40.);
        let remembered = (
            first.scroll.hadjustment().value(),
            first.scroll.vadjustment().value(),
        );
        view.grab_focus();
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"key","key":"document-end"}),
        )
        .await?;
        crate::acceptance_wait::until("mixed native End", || {
            view.buffer().cursor_position() == view.buffer().char_count()
        })
        .await?;
        crate::window_capture::next_frame(&view).await?;
        crate::acceptance_wait::until("last mixed table formulas painted", || {
            let (visible, ready) =
                crate::inline_math::smoke_visible_formulas(view.upcast_ref(), "$z^2$");
            visible > 0 && visible == ready
        })
        .await?;
        crate::acceptance_wait::until("first mixed table factories detached", || {
            first.view.model().is_none()
        })
        .await?;
        crate::window_capture::capture(&self.window, &root.join("dense-mixed-last.png")).await?;
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"key","key":"document-start"}),
        )
        .await?;
        crate::acceptance_wait::until("mixed table model and scroll restored", || {
            first.view.model().is_some()
                && first.restore.borrow().is_none()
                && (first.scroll.hadjustment().value() - remembered.0).abs() < 1.
                && (first.scroll.vadjustment().value() - remembered.1).abs() < 1.
        })
        .await?;
        if first.start.get() != 50 || column.fixed_width() != 306 || first.rows.n_items() != 10 {
            return Err("Table viewport admission lost page, rows or column width".into());
        }
        crate::acceptance_wait::until("table viewport admission idle", || {
            self.flow_tables.borrow().values().all(|t| {
                t.restore.borrow().is_none()
                    && t.visibility.borrow().as_ref().is_none_or(|v| !v.pending())
            })
        })
        .await?;
        let bindings: usize = self
            .flow_tables
            .borrow()
            .values()
            .map(|t| t.bindings.get())
            .sum();
        glib::timeout_future(Duration::from_millis(200)).await;
        if self
            .flow_tables
            .borrow()
            .values()
            .map(|t| t.bindings.get())
            .sum::<usize>()
            != bindings
        {
            return Err("Table row factories rebound without viewport or data changes".into());
        }
        crate::window_capture::capture(&self.window, &root.join("dense-mixed-restored.png"))
            .await?;
        let buffer = view.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        self.flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .copied
            .replace(None);
        view.emit_copy_clipboard();
        crate::acceptance_wait::until("mixed projected Copy", || {
            self.flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .copied
                .borrow()
                .is_some()
        })
        .await?;
        let copied = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .copied
            .borrow()
            .clone()
            .unwrap()?;
        if copied.text.matches("$\\frac{x^2+1}{1+y}$").count() != 16
            || !copied.text.contains("MIXED-7-59")
        {
            return Err("Mixed formula/table Copy lost original text or full indexed rows".into());
        }
        if buffer.is_modified() {
            return Err("Mixed read-only view became modified".into());
        }
        self.set_mode(Mode::Source);
        crate::acceptance_wait::until("mixed Source retirement", || {
            self.is_ready()
                && self.state.borrow().mode == Mode::Source
                && self.flow_preview.borrow().is_none()
        })
        .await?;
        if objects.iter().any(|w| w.is_mapped()) {
            return Err("Mixed object survived Source switch".into());
        }
        Ok(
            serde_json::json!({"ok":true,"normalDocumentActor":true,"formulas":formulas,"tables":8,"maximumTableRows":50,"readyMs":ready_ms,"firstPaintMs":first_paint_ms,"maximumGapThroughFirstPaintMs":paint_gap,"initialActiveTables":active_tables,"initialUntouchedTables":untouched_tables,"initialCellBindings":initial_bindings,"openingMainLoopTicks":ticks,"openingMaximumGapMs":gap,"renderUs":preparation.0,"renderedObjects":preparation.1,"reusedObjects":preparation.2,"visibleCellFormulasPainted":true,"tableFactoriesReleased":true,"tablePageAndWidthRetained":true,"tableScrollRetained":true,"idleBindingsStable":true,"exactFormulaCopy":true,"fullTableCopy":true,"nativeEnd":true,"sourceRetirement":true}),
        )
    }
}

impl Reader {
    pub(crate) async fn smoke_repeated_math(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        let shell_pulse = Pulse::new(self);
        let shell_started = std::time::Instant::now();
        crate::window_capture::next_frame(&self.window).await?;
        let shell_ms = shell_started.elapsed().as_millis();
        let shell_gap = shell_pulse.maximum_gap();
        drop(shell_pulse);
        let pulse = Pulse::new(self);
        let started = std::time::Instant::now();
        let path = root.join("project/repeated-math.md");
        self.open_path(&path);
        let open_call_us = started.elapsed().as_micros();
        crate::acceptance_wait::until("repeated math document", || {
            self.is_ready() && self.state.borrow().path.as_ref() == Some(&path)
        })
        .await?;
        if self.flow_preview.borrow().is_none() {
            crate::window_capture::capture(&self.window, &root.join("repeated-math-rejected.png"))
                .await?;
            return Err(format!(
                "Repeated math context rejected: {}",
                self.status.text()
            ));
        }
        let before_frame_ms = started.elapsed().as_millis();
        let (view, objects, document, preparation, construction, mount_us) = {
            let flow = self.flow_preview.borrow();
            let flow = flow.as_ref().unwrap();
            (
                flow.widget.clone(),
                flow.objects.clone(),
                flow.document.clone(),
                flow.preparation,
                flow.construction_us.clone(),
                flow.mount_us.get(),
            )
        };
        if objects.len() != 64
            || document.objects.iter().any(|o| {
                !matches!(
                    o.kind,
                    studio_markdown_native::flow::ObjectKind::Math { .. }
                )
            })
        {
            return Err("Repeated formula projection is incomplete".into());
        }
        if objects.iter().any(|w| {
            w.downcast_ref::<gtk::Picture>()
                .is_none_or(|p| p.paintable().is_none())
        }) {
            return Err("Repeated formula did not mount a real native raster".into());
        }
        crate::window_capture::next_frame(&view).await?;
        let ready_ms = started.elapsed().as_millis();
        let gap = pulse.maximum_gap();
        let layout_us = view
            .downcast_ref::<crate::anchored_text_view::AnchoredTextView>()
            .unwrap()
            .layout_timings();
        crate::window_capture::capture(&self.window, &root.join("repeated-math-first.png")).await?;
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        crate::acceptance_wait::until("repeated formula test window active", || {
            self.window.is_active()
        })
        .await?;
        if !view.grab_focus() {
            return Err("Repeated formula view cannot receive keyboard focus".into());
        }
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"key","key":"document-end"}),
        )
        .await?;
        crate::acceptance_wait::until("last repeated formula visible", || {
            view.buffer().cursor_position() == view.buffer().char_count()
                && objects
                    .last()
                    .unwrap()
                    .compute_bounds(&view)
                    .is_some_and(|r| r.y() >= 0. && r.y() + r.height() <= view.height() as f32)
        })
        .await?;
        crate::window_capture::capture(&self.window, &root.join("repeated-math-last.png")).await?;
        let buffer = view.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        view.emit_copy_clipboard();
        crate::acceptance_wait::until("repeated formula Copy", || {
            self.flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .copied
                .borrow()
                .is_some()
        })
        .await?;
        let copied = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .copied
            .borrow()
            .clone()
            .unwrap()?;
        let formula = "$\\frac{x^2+1}{1+y}$";
        if copied.text.matches(formula).count() != 64 || !copied.text.contains("REPEAT-63") {
            return Err("Repeated formula Copy lost source or its last passage".into());
        }
        let copied_at = std::time::Instant::now();
        loop {
            let actual =
                crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
            if actual["text"].as_str() == Some(&copied.text) {
                break;
            }
            if copied_at.elapsed() > Duration::from_secs(3) {
                return Err(
                    "Repeated formula clipboard differs from original projected source".into(),
                );
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        self.set_mode(Mode::Source);
        crate::acceptance_wait::until("repeated formulas Source", || {
            self.is_ready() && self.state.borrow().mode == Mode::Source
        })
        .await?;
        let warm_pulse = Pulse::new(self);
        let warm_started = std::time::Instant::now();
        self.set_mode(Mode::Preview);
        crate::acceptance_wait::until("repeated formulas Preview return", || {
            self.is_ready() && self.flow_preview.borrow().is_some()
        })
        .await?;
        let (restored_view, warm_construction, warm_mount_us) = {
            let restored = self.flow_preview.borrow();
            let restored = restored.as_ref().unwrap();
            if restored.document.objects.len() != 64 || objects.iter().any(|w| w.is_mapped()) {
                return Err(
                    "Repeated formula view replacement retained old widgets or lost formulas"
                        .into(),
                );
            }
            (
                restored.widget.clone(),
                restored.construction_us.clone(),
                restored.mount_us.get(),
            )
        };
        crate::window_capture::next_frame(&restored_view).await?;
        let warm_ms = warm_started.elapsed().as_millis();
        let warm_gap = warm_pulse.maximum_gap();
        let warm_layout_us = restored_view
            .downcast_ref::<crate::anchored_text_view::AnchoredTextView>()
            .unwrap()
            .layout_timings();
        drop(warm_pulse);
        let distinct = root.join("project/distinct-math.md");
        self.open_path(&distinct);
        crate::acceptance_wait::until("distinct math budget notice", || {
            self.is_ready()
                && self.state.borrow().path.as_ref() == Some(&distinct)
                && self.flow_preview.borrow().is_none()
        })
        .await?;
        self.update();
        if !self.notice.is_visible()
            || !self.notice.text().contains("32 renders")
            || !self.modes[1].is_sensitive()
        {
            return Err(
                "Rejected Preview did not retain a useful notice and enabled Source action".into(),
            );
        }
        crate::window_capture::capture(&self.window, &root.join("distinct-math-rejected.png"))
            .await?;
        self.set_mode(Mode::Source);
        crate::acceptance_wait::until("distinct formulas remain readable in Source", || {
            self.is_ready() && self.state.borrow().mode == Mode::Source
        })
        .await?;
        if self.notice.is_visible()
            || !self
                .state
                .borrow()
                .source
                .as_ref()
                .is_some_and(|s| s.text.contains("x_{32}"))
        {
            return Err(
                "Preview rejection hid Source text or leaked its notice into Source".into(),
            );
        }
        self.set_mode(Mode::Preview);
        crate::acceptance_wait::until("distinct formula rejection after Source", || {
            self.is_ready() && self.state.borrow().mode == Mode::Preview
        })
        .await?;
        self.open_path(&path);
        crate::acceptance_wait::until("new document clears Preview rejection", || {
            self.is_ready()
                && self.state.borrow().path.as_ref() == Some(&path)
                && self.flow_preview.borrow().is_some()
        })
        .await?;
        if self.notice.is_visible() {
            return Err("A previous document's rendering error survived a new Preview".into());
        }
        Ok(
            serde_json::json!({"ok":true,"normalDocumentActor":true,"shellFirstFrameMs":shell_ms,"shellMaximumGapMs":shell_gap,"formulas":64,"mountedRasters":64,"renderUs":preparation.0,"renderedObjects":preparation.1,"reusedObjects":preparation.2,"readyMs":ready_ms,"beforeFrameMs":before_frame_ms,"openCallUs":open_call_us,"constructionUs":construction,"mountUs":mount_us,"layoutUs":layout_us,"openingMaximumGapMs":gap,"warmPreviewMs":warm_ms,"warmMaximumGapMs":warm_gap,"warmConstructionUs":warm_construction,"warmMountUs":warm_mount_us,"warmLayoutUs":warm_layout_us,"nativeEnd":true,"exactIndependentCopy":true,"sourcePreviewRoundTrip":true,"retiredObjectsUnmapped":true,"overBudgetNoticePersisted":true,"overBudgetSourceReadable":true,"noticeClearsAfterNewFile":true}),
        )
    }
}
