//! Exact stale-save probe around a real gateway exit; never writes the source.
use super::*;
impl Reader {
    pub(crate) async fn smoke_before_gateway_loss(self: &Rc<Self>) -> Result<Response, String> {
        self.set_mode(Mode::Edit);
        let start = std::time::Instant::now();
        while !self.is_ready() || self.state.borrow().editor.is_none() {
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Gateway-loss Edit page did not load".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        if std::env::var("STUDIO_GNOME_SMOKE_DISCOVERY").as_deref() == Ok("runtime-paged") {
            let line = self.state.borrow().index.as_ref().unwrap().source_lines / 2;
            self.navigate(Navigation::Line(line.max(2)));
            let start = std::time::Instant::now();
            while !self.is_ready()
                || self
                    .state
                    .borrow()
                    .editor
                    .as_ref()
                    .is_none_or(|p| p.window.start == 0)
            {
                if start.elapsed() > Duration::from_secs(15) {
                    return Err("Paged recovery did not reach a nonzero source window".into());
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
        }
        let buffer = self.edit_view.buffer();
        buffer.insert(
            &mut buffer.end_iter(),
            "\nUnsaved before gateway exit 中文😀\n",
        );
        let mut state = self.state.borrow_mut();
        let page = state.editor.clone().unwrap();
        state.serial += 1;
        let serial = state.serial;
        state.latest.insert("save", serial);
        state.saving = true;
        state.busy = true;
        let response = Response {
            key: self.key,
            generation: state.generation,
            serial,
            kind: "save",
            result: Ok(Reply::Saved(page)),
        };
        drop(state);
        self.update();
        Ok(response)
    }
    pub(crate) fn smoke_after_gateway_loss(
        self: &Rc<Self>,
        stale: Response,
    ) -> Result<serde_json::Value, String> {
        let buffer = self.edit_view.buffer();
        if self.state.borrow().saving || !self.state.borrow().dirty || !self.edit_view.is_editable()
        {
            return Err("Gateway loss did not preserve dirty editing".into());
        }
        buffer.insert(&mut buffer.end_iter(), "Typed after gateway exit 中文😀\n");
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if !text.contains("Typed after gateway exit 中文😀") || self.save.is_sensitive() {
            return Err("Offline buffer rejected typing or enabled an unavailable Save".into());
        }
        self.event(stale);
        if !self.state.borrow().dirty
            || !buffer.is_modified()
            || buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != text
        {
            return Err("Old Save acknowledgement cleared newer edits".into());
        }
        Ok(
            serde_json::json!({"dirtyEditRetained":true,"typingAfterLossRetained":true,"staleSaveRejected":true}),
        )
    }
    pub(crate) fn smoke_discard_gateway_loss_edits(&self) {
        self.state.borrow_mut().dirty = false;
        self.edit_view.buffer().set_modified(false);
    }

    pub(crate) async fn smoke_gateway_resumed(
        self: &Rc<Self>,
        conflict: bool,
    ) -> Result<serde_json::Value, String> {
        let start = std::time::Instant::now();
        while self.state.borrow().busy {
            if start.elapsed() > Duration::from_secs(20) {
                return Err("Document revalidation did not settle".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        let buffer = self.edit_view.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        if !self.state.borrow().dirty
            || !buffer.can_undo()
            || !text.contains("Typed after gateway exit 中文😀")
        {
            return Err("Reconnection replaced the edit buffer or its undo history".into());
        }
        if conflict {
            if !self.notice.is_visible() || !self.notice.text().contains("Document recovery failed")
            {
                return Err("Document conflict has no persistent visible explanation".into());
            }
            if self.session_valid.get() || self.save.is_sensitive() {
                return Err("Changed file enabled Save".into());
            }
            buffer.insert(&mut buffer.end_iter(), "Typed after conflict\n");
            return Ok(
                serde_json::json!({"conflictProtected":true,"dirtyEditRetained":true,"undoRetained":true}),
            );
        }
        if !self.session_valid.get() || !self.save.is_sensitive() {
            return Err("Identical snapshot did not restore Save".into());
        }
        let (start_offset, end_offset) = {
            let state = self.state.borrow();
            let page = state.editor.as_ref().unwrap();
            (page.window.start, page.window.end)
        };
        buffer.undo();
        if buffer.text(&buffer.start_iter(), &buffer.end_iter(), true) == text || !buffer.can_redo()
        {
            return Err("Recovered editor Undo did not change the retained text".into());
        }
        buffer.redo();
        if buffer.text(&buffer.start_iter(), &buffer.end_iter(), true) != text {
            return Err("Recovered editor Redo did not restore the exact text".into());
        }
        self.save.emit_clicked();
        let start = std::time::Instant::now();
        while self.state.borrow().dirty || self.state.borrow().busy {
            if start.elapsed() > Duration::from_secs(20) {
                return Err(format!(
                    "Explicit recovered Save failed: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(
            serde_json::json!({"snapshotRevalidated":true,"dirtyEditRetained":true,"undoRetained":true,"undoRedoRoundTrip":true,"explicitSave":true,"expectedText":text,"start":start_offset,"end":end_offset}),
        )
    }
}
