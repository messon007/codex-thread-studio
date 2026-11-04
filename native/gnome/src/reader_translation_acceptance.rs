//! Actual reader actor, projected selection and private translation provider.
use super::*;
impl Reader {
    pub async fn smoke_projected_translation(
        self: &Rc<Self>,
        translation: &Rc<crate::translation_ui::Runtime>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            crate::acceptance_wait::until(step, test).await
        }
        let old = self.continuous_preview.replace(true);
        struct Restore<'a>(&'a Cell<bool>, bool);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(&self.continuous_preview, old);
        self.open_path(&root.join("project/flow-translation.md"));
        until("projected translation reader", || {
            self.is_ready() && self.flow_preview.borrow().is_some()
        })
        .await?;
        let source_bytes = self.state.borrow().index.as_ref().unwrap().source_bytes;
        if source_bytes < 32 * 1024 * 1024 {
            return Err("Translation fixture is not a large table".into());
        }
        let view = self.flow_preview.borrow().as_ref().unwrap().widget.clone();
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        if !view.grab_focus() {
            return Err("Projected reader did not receive keyboard focus".into());
        }
        let buffer = view.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"translate"}))
            .await?;
        until("projected translation result", || {
            translation.smoke_idle()
                && translation
                    .smoke_view()
                    .is_some_and(|(_, _, _, _, _, ready)| ready)
        })
        .await?;
        let (dialog, text, _, _, _, _) = translation
            .smoke_view()
            .ok_or("Projected translation dialog absent")?;
        let expected_path = root.join("expected-flow-translation.txt");
        let expected = gio::spawn_blocking(move || std::fs::read_to_string(expected_path))
            .await
            .map_err(|_| "Translation fixture read stopped")?
            .map_err(|e| e.to_string())?;
        if text != expected
            || text.chars().count() != crate::translation_text::MAX_SOURCE
            || text.contains(studio_markdown_native::flow::ANCHOR)
        {
            return Err(format!(
                "Projected translation lost its exact Unicode/table/math prefix: {} characters",
                text.chars().count()
            ));
        }
        crate::window_capture::capture_dialog(
            &self.window,
            &dialog,
            &root.join("translation-flow.png"),
        )
        .await?;
        translation.smoke_close();
        until("projected translation closed", || {
            translation.smoke_view().is_none()
        })
        .await?;
        view.activate_action("translation.translate", None)
            .map_err(|e| e.to_string())?;
        until("projected translation cached", || {
            translation
                .smoke_view()
                .is_some_and(|(_, selected, _, _, status, ready)| {
                    selected == expected && ready && status.contains("cache")
                })
        })
        .await?;
        translation.smoke_close();
        until("projected cached dialog closed", || {
            translation.smoke_view().is_none()
        })
        .await?;
        let document = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .document
            .clone();
        let revision = self.state.borrow().index.as_ref().unwrap().revision.clone();
        let prepared = gio::spawn_blocking(move || {
            crate::flow_prepare::prepare_projection(
                document,
                revision,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
                true,
            )
        })
        .await
        .map_err(|_| "Translation preparation stopped")??;
        self.clear_flow_preview();
        self.state.borrow_mut().mode = Mode::Preview;
        let mut options = self
            .flow_options()
            .ok_or("Translation source options absent")?;
        let tables = options.selection_tables.as_mut().unwrap();
        let delegate = tables.prefix.take().ok_or("Table prefix callback absent")?;
        let (selection_release, selection_receive) = tokio::sync::oneshot::channel::<()>();
        let (mode_release, mode_receive) = tokio::sync::oneshot::channel::<()>();
        let held = Rc::new(RefCell::new(std::collections::VecDeque::from([
            selection_receive,
            mode_receive,
        ])));
        let started = Rc::new(Cell::new(0));
        let dropped = Rc::new(Cell::new(0));
        let flag = started.clone();
        let finished = dropped.clone();
        tables.prefix = Some(Rc::new(move |id, characters| {
            let call = flag.get();
            let receive = (call != 1).then(|| held.borrow_mut().pop_front()).flatten();
            let delegate = delegate.clone();
            let finished = finished.clone();
            flag.set(call + 1);
            Box::pin(async move {
                if let Some(receive) = receive {
                    struct Retired(Rc<Cell<usize>>);
                    impl Drop for Retired {
                        fn drop(&mut self) {
                            self.0.set(self.0.get() + 1);
                        }
                    }
                    let _retired = Retired(finished);
                    receive
                        .await
                        .map_err(|_| "Held prefix cancelled".to_owned())?;
                }
                delegate(id, characters).await
            })
        }));
        let replacement = crate::text_flow::view_with_options(prepared, options)?;
        let stale = replacement.widget.clone();
        self.preview_scroll.set_child(Some(&stale));
        self.flow_preview.replace(Some(replacement));
        until("held prefix view mapped", || stale.is_mapped()).await?;
        let buffer = stale.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        stale
            .activate_action("translation.translate", None)
            .map_err(|e| e.to_string())?;
        until("table prefix paused", || started.get() == 1).await?;
        if !stale.grab_focus() {
            return Err("Held selection did not receive keyboard focus".into());
        }
        crate::smoke_input::request(
            root,
            serde_json::json!({"action":"key","key":"document-start"}),
        )
        .await?;
        until("changed selection dropped held prefix", || {
            dropped.get() == 1
        })
        .await?;
        if selection_release.send(()).is_ok() || translation.smoke_view().is_some() {
            return Err("Changed selection retained its pending prefix".into());
        }
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        stale
            .activate_action("translation.translate", None)
            .map_err(|e| e.to_string())?;
        until("new selection can translate immediately", || {
            translation
                .smoke_view()
                .is_some_and(|(_, selected, _, _, status, ready)| {
                    selected == expected && ready && status.contains("cache")
                })
        })
        .await?;
        translation.smoke_close();
        until("new selection dialog closed", || {
            translation.smoke_view().is_none()
        })
        .await?;
        stale
            .activate_action("translation.translate", None)
            .map_err(|e| e.to_string())?;
        until("mode prefix paused", || started.get() == 3).await?;
        self.modes[1].set_active(true);
        until("held prefix source switch", || {
            self.state.borrow().mode == Mode::Source && !stale.is_mapped()
        })
        .await?;
        until("source switch dropped held prefix", || dropped.get() == 2).await?;
        if mode_release.send(()).is_ok()
            || translation.smoke_view().is_some()
            || !translation.smoke_idle()
        {
            return Err("Retired projected selection started translation".into());
        }
        self.smoke_close();
        Ok(
            serde_json::json!({"ok":true,"sourceBytes":source_bytes,"prefixCharacters":text.chars().count(),"shortcutAndContextAction":true,"unicodeMathTableSource":true,"cache":true,"selectionReadCancelled":true,"selectionRetry":true,"staleModeRejected":true,"modeReadCancelled":true}),
        )
    }
}
