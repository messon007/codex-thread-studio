//! Verify a retained reader after actual workspace dispatch/return.
use super::*;
impl Reader {
    pub fn smoke_workspace_offer(
        view: &gtk::TextView,
    ) -> Result<gtk::gdk::ContentProvider, String> {
        let controllers = view.observe_controllers();
        let drag = (0..controllers.n_items())
            .find_map(|i| controllers.item(i).and_downcast::<gtk::DragSource>())
            .ok_or("Retained projected drag controller missing")?;
        let rect = view.iter_location(&view.buffer().start_iter());
        let (x, y) = view.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            rect.x() + 2,
            rect.y() + rect.height() / 2,
        );
        drag.emit_by_name::<Option<gtk::gdk::ContentProvider>>(
            "prepare",
            &[&(x as f64), &(y as f64)],
        )
        .ok_or("Retained drag offer missing".into())
    }
    pub async fn smoke_workspace_stale_offer(
        offer: &gtk::gdk::ContentProvider,
    ) -> Result<(), String> {
        let stream = gio::MemoryOutputStream::new_resizable();
        if offer
            .write_mime_type_future("text/plain;charset=utf-8", &stream, glib::Priority::DEFAULT)
            .await
            .is_ok()
        {
            return Err("An old drag offer revived after remapping the same selection".into());
        }
        if stream.data_size() != 0 {
            return Err("An expired offer wrote data".into());
        }
        Ok(())
    }

    /// Hold real indexed-table replies across hiding/remapping the same view.
    /// No provider turn, comment store mutation or synthetic table payload.
    pub async fn smoke_workspace_transfer_races(self: &Rc<Self>) -> Result<(), String> {
        type Reply = tokio::sync::oneshot::Sender<
            Result<Vec<studio_markdown_native::flow::TableText>, String>,
        >;
        let original = self
            .flow_preview
            .borrow()
            .as_ref()
            .ok_or("No retained flow")?
            .widget
            .clone();
        let source = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .document
            .clone();
        let revision = self.state.borrow().index.as_ref().unwrap().revision.clone();
        let request = source.selection_request(0..source.characters, 16 * 1024)?;
        let mut options = self.flow_options().ok_or("No race fixture options")?;
        let tables = options
            .selection_tables
            .as_mut()
            .ok_or("No table delegate")?;
        let resolved = (tables.resolve)(request.tables, request.remaining).await?;
        let pending = Rc::new(RefCell::new(Vec::<Reply>::new()));
        let queue = pending.clone();
        tables.resolve = Rc::new(move |_, _| {
            let (reply, receive) = tokio::sync::oneshot::channel();
            queue.borrow_mut().push(reply);
            Box::pin(async move { receive.await.map_err(|_| "Held read closed".to_owned())? })
        });
        options.indexed_tables = Some(Rc::new(|_, _| {
            Ok(gtk::Box::new(gtk::Orientation::Vertical, 0).upcast())
        }));
        options.selection_error = None;
        let captured = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let comments = options.comments.as_mut().ok_or("No comment delegate")?;
        comments.configure = None;
        comments.begin = Rc::new(move || {
            let sink = sink.clone();
            Some(Rc::new(move |value| sink.borrow_mut().push(value)))
        });
        let prepared = gio::spawn_blocking(move || {
            crate::text_flow::prepare_projection(
                source,
                revision,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
                false,
            )
        })
        .await
        .map_err(|_| "Workspace fixture preparation stopped")??;
        let flow = crate::text_flow::view_with_options(prepared, options)?;
        self.preview_scroll.set_child(Some(&flow.widget));
        crate::acceptance_wait::until("race fixture mapped", || flow.widget.is_mapped()).await?;
        let buffer = flow.widget.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        for comment in [false, true] {
            let dispatch = || -> Result<(), String> {
                if comment {
                    flow.widget
                        .activate_action("reader-comment.capture", None)
                        .map_err(|e| e.to_string())
                } else {
                    flow.widget.emit_copy_clipboard();
                    Ok(())
                }
            };
            let busy = if comment {
                flow.commenting.clone()
            } else {
                flow.copying.clone()
            };
            dispatch()?;
            crate::acceptance_wait::until("old table read held", || pending.borrow().len() == 1)
                .await?;
            let old = pending.borrow_mut().remove(0);
            if old.is_closed() || !busy.get() {
                return Err("Old transfer was not in flight".into());
            }
            self.preview_scroll
                .set_child(Some(&gtk::Label::new(Some("Private replacement pane"))));
            if flow.widget.is_mapped() || busy.get() {
                return Err("Hiding did not release transfer busy state".into());
            }
            self.preview_scroll.set_child(Some(&flow.widget));
            crate::acceptance_wait::until("same race fixture remapped", || flow.widget.is_mapped())
                .await?;
            dispatch()?;
            crate::acceptance_wait::until("fresh read held and old read cancelled", || {
                old.is_closed() && pending.borrow().len() == 1
            })
            .await?;
            if !busy.get() || flow.copied.borrow().is_some() || !captured.borrow().is_empty() {
                return Err("Old completion cleared or published over the new request".into());
            }
            if old.send(Ok(resolved.clone())).is_ok() {
                return Err("Hidden read receiver survived".into());
            }
            pending
                .borrow_mut()
                .remove(0)
                .send(Ok(resolved.clone()))
                .map_err(|_| "Fresh receiver closed")?;
            crate::acceptance_wait::until("fresh read completes", || !busy.get()).await?;
            if comment {
                let captures = captured.borrow();
                if captures.len() != 1
                    || captures[0].as_ref().map(|s| s.text.as_str()).ok()
                        != Some(expected().as_str())
                {
                    return Err(
                        "Fresh comment did not capture the full indexed selection exactly once"
                            .into(),
                    );
                }
            } else {
                if flow
                    .copied
                    .borrow()
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .map(|s| s.text.clone())
                    != Some(expected())
                {
                    return Err("Fresh Copy did not retain full indexed text".into());
                }
                flow.copied.borrow_mut().take();
            }
        }
        self.preview_scroll.set_child(Some(&original));
        crate::acceptance_wait::until("original view restored after races", || {
            original.is_mapped()
        })
        .await?;
        Ok(())
    }
    pub async fn smoke_workspace_transfer(
        self: &Rc<Self>,
        root: &Path,
        previous: Option<&gtk::TextView>,
    ) -> Result<gtk::TextView, String> {
        crate::acceptance_wait::until("retained indexed document", || {
            self.is_ready()
                && self
                    .flow_preview
                    .borrow()
                    .as_ref()
                    .is_some_and(|v| v.widget.is_mapped())
                && self
                    .flow_tables
                    .borrow()
                    .values()
                    .any(|t| t.rows.n_items() > 0)
        })
        .await?;
        let view = self.flow_preview.borrow().as_ref().unwrap().widget.clone();
        if previous.is_some_and(|p| p != &view) {
            return Err("Workspace return rebuilt the document instead of retaining it".into());
        }
        if !self.window.is_active() {
            crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        }
        view.grab_focus();
        let buffer = view.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        view.clipboard()
            .set_text("Clipboard sentinel before retained reader Copy");
        self.flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .copied
            .borrow_mut()
            .take();
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"copy"})).await?;
        crate::acceptance_wait::until("retained table Copy completion", || {
            self.flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .copied
                .borrow()
                .is_some()
        })
        .await?;
        let result = self
            .flow_preview
            .borrow()
            .as_ref()
            .unwrap()
            .copied
            .borrow()
            .clone()
            .unwrap();
        let text = result
            .map_err(|e| {
                format!(
                    "Retained document Copy failed (after pane return={}): {e}",
                    previous.is_some()
                )
            })?
            .text;
        let expected = expected();
        if text != expected {
            return Err(format!(
                "Retained table Copy changed its full text: {text:?}"
            ));
        }
        let copied =
            crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
        let primary =
            crate::smoke_input::request(root, serde_json::json!({"action":"read-primary"})).await?;
        if copied["text"] != expected || primary["text"] != expected {
            return Err("Retained reader clipboard/primary changed table text".into());
        }
        Ok(view)
    }
}
fn expected() -> String {
    let mut text =
        String::from("Viewer navigation\n\nParent document 中文😀.\n\nImage\n\nName\tValue");
    for i in 0..70 {
        text.push_str(&format!("\nROW-{i}\tVALUE-{i}"));
    }
    text
}
