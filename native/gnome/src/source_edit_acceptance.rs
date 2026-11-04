use super::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
impl Source {
    pub(crate) async fn smoke_edit(
        &self,
        root: &Path,
        capture: impl Fn(&str),
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let start = Instant::now();
            while !test() {
                if start.elapsed() > Duration::from_secs(20) {
                    return Err(format!("Source editor timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(10)).await;
            }
            Ok(())
        }
        async fn read(path: PathBuf) -> Result<Vec<u8>, String> {
            gtk::gio::spawn_blocking(move || std::fs::read(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Fixture read stopped")?
        }
        async fn write(path: PathBuf, bytes: Vec<u8>) -> Result<(), String> {
            gtk::gio::spawn_blocking(move || std::fs::write(path, bytes).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Fixture write stopped")?
        }
        let project = root.join("project");
        let path = project.join("edit.rs");
        let original = read(path.clone()).await?;
        self.open(project.to_str().unwrap(), "edit.rs");
        until("identified source", || {
            self.text.borrow().is_some() && self.widget.is_mapped()
        })
        .await?;
        if !self.editor.modes.is_visible() {
            return Err("Writable file has no native edit controls".into());
        }
        self.editor.edit.set_active(true);
        until("edit page", || {
            self.editor.page.borrow().is_some() && !self.editor.busy.get()
        })
        .await?;
        self.edit_insert("// first 中文😀\n");
        if !self.editor.dirty.get() || !self.editor.save.is_sensitive() {
            return Err("Editing did not mark the page dirty".into());
        }
        self.editor.source.set_active(true);
        if self.visible_text().starts_with("// first")
            || !self.editor.notice.is_visible()
            || read(path.clone()).await? != original
        {
            return Err("View switch wrote the file or replaced the saved Source snapshot".into());
        }
        self.editor.edit.set_active(true);
        if !self.edit_text().starts_with("// first 中文😀\n") {
            return Err("Returning to Edit lost its in-memory changes".into());
        }
        until("edit cursor visible", || {
            let buffer = self.editor.view.buffer();
            let at = self
                .editor
                .view
                .iter_location(&buffer.iter_at_mark(&buffer.get_insert()));
            let visible = self.editor.view.visible_rect();
            at.y() >= visible.y() && at.y() + at.height() <= visible.y() + visible.height()
        })
        .await?;
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if self.editor.edit.label().as_deref() != Some("编辑")
            || self.editor.save.tooltip_text().as_deref() != Some("保存当前页（Ctrl+S）")
            || !self.status.text().contains("未保存")
            || !self.edit_text().starts_with("// first 中文😀\n")
        {
            return Err(
                "Live editor translation changed source text or missed native controls".into(),
            );
        }
        capture("source-edit-chinese.png");
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        capture("source-edit-dirty.png");
        let ticks = Rc::new(Cell::new(0));
        let gap = Rc::new(Cell::new(0u128));
        let clock = Rc::new(Cell::new(Instant::now()));
        let timer = glib::timeout_add_local(Duration::from_millis(10), {
            let ticks = ticks.clone();
            let gap = gap.clone();
            let clock = clock.clone();
            move || {
                let now = Instant::now();
                gap.set(
                    gap.get()
                        .max(now.duration_since(clock.replace(now)).as_millis()),
                );
                ticks.set(ticks.get() + 1);
                glib::ControlFlow::Continue
            }
        });
        let started = Instant::now();
        self.editor.save.emit_clicked();
        if self.allow_window_close() {
            timer.remove();
            return Err("An in-flight save allowed window close".into());
        }
        let saved_result = until("explicit save", || {
            !self.editor.busy.get() && !self.editor.dirty.get()
        })
        .await;
        let save_ms = started.elapsed().as_millis();
        timer.remove();
        saved_result.map_err(|error| {
            format!(
                "{error}; status={} busy={} dirty={}",
                self.status.text(),
                self.editor.busy.get(),
                self.editor.dirty.get()
            )
        })?;
        let saved = read(path.clone()).await?;
        let mut expected = b"// first ".to_vec();
        expected.extend_from_slice("中文😀\r\n".as_bytes());
        expected.extend_from_slice(&original);
        if saved != expected {
            return Err("Page save rewrote unrelated bytes or CRLF line breaks".into());
        }
        self.editor.undo.emit_clicked();
        if !self.editor.dirty.get() || self.edit_text().starts_with("// first") {
            return Err("Undo did not cross the save point".into());
        }
        self.editor.save.emit_clicked();
        until("save undo", || {
            !self.editor.busy.get() && !self.editor.dirty.get()
        })
        .await?;
        if read(path.clone()).await? != original {
            return Err("Saving Undo did not restore exact original bytes".into());
        }
        self.editor.redo.emit_clicked();
        self.next.emit_clicked();
        until("page-change save", || {
            !self.editor.busy.get()
                && !self.editor.dirty.get()
                && self
                    .editor
                    .page
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.window.line == 202)
        })
        .await?;
        if read(path.clone()).await? != expected {
            return Err("Changing edit pages did not save the previous page".into());
        }
        self.edit_insert("// second 😀\n");
        self.open(project.to_str().unwrap(), "other.py");
        self.edit_response("cancel").await?;
        if !self.editor.dirty.get()
            || !self
                .text
                .borrow()
                .as_ref()
                .unwrap()
                .path
                .ends_with("edit.rs")
            || read(path.clone()).await? != expected
        {
            return Err("Cancelled file replacement lost or saved edits".into());
        }
        self.open(project.to_str().unwrap(), "other.py");
        self.edit_response("save").await?;
        until("replacement after save", || {
            !self.editor.busy.get()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.path.ends_with("other.py"))
        })
        .await?;
        let previous = read(path.clone()).await?;
        if !previous
            .windows("// second 😀\r\n".len())
            .any(|w| w == "// second 😀\r\n".as_bytes())
        {
            return Err("Saving before replacement lost the second page".into());
        }
        let other = project.join("other.py");
        let other_original = read(other.clone()).await?;
        self.editor.edit.set_active(true);
        until("other edit", || {
            !self.editor.busy.get() && self.editor.page.borrow().is_some()
        })
        .await?;
        self.edit_insert("# local 中文😀\n");
        let external = b"# external writer\n".to_vec();
        write(other.clone(), external.clone()).await?;
        self.editor.save.emit_clicked();
        self.edit_response("cancel").await?;
        if !self.editor.dirty.get()
            || read(other.clone()).await? != external
            || !self.edit_text().starts_with("# local")
        {
            return Err("Conflict cancellation overwrote disk or discarded local edits".into());
        }
        self.editor.save.emit_clicked();
        self.edit_response("overwrite").await?;
        until("explicit overwrite", || {
            !self.editor.busy.get() && !self.editor.dirty.get()
        })
        .await?;
        let mut other_expected = "# local 中文😀\n".as_bytes().to_vec();
        other_expected.extend_from_slice(&other_original);
        if read(other.clone()).await? != other_expected {
            return Err("Explicit overwrite did not save the intended snapshot".into());
        }
        self.edit_insert("# unsaved close candidate\n");
        self.close.emit_clicked();
        self.edit_response("cancel").await?;
        if !self.editor.dirty.get() {
            return Err("Cancelled close lost in-memory edits".into());
        }
        if self.allow_window_close() {
            return Err("Window close bypassed unsaved source edits".into());
        }
        self.edit_response("cancel").await?;
        self.close.emit_clicked();
        self.edit_response("discard").await?;
        if self.has_document() || read(other.clone()).await? != other_expected {
            return Err("Discard closed incorrectly or wrote the source".into());
        }
        let shared = root.join("readonly/shared.rs");
        self.open(project.to_str().unwrap(), shared.to_str().unwrap());
        until("shared source", || self.text.borrow().is_some()).await?;
        if !self
            .text
            .borrow()
            .as_ref()
            .unwrap()
            .file
            .as_ref()
            .unwrap()
            .read_only
            || self.editor.modes.is_visible()
            || self.editor.save.is_visible()
        {
            return Err("Read-only shared source exposed editing".into());
        }
        self.open(project.to_str().unwrap(), "real-main.rs");
        until("real source", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("real-main.rs"))
        })
        .await?;
        self.editor.edit.set_active(true);
        until("real edit", || {
            !self.editor.busy.get() && self.editor.page.borrow().is_some()
        })
        .await?;
        self.edit_insert("// Native source editing fixture 中文😀\n");
        self.editor.save.emit_clicked();
        until("real source saved", || {
            !self.editor.busy.get() && !self.editor.dirty.get()
        })
        .await?;
        capture("source-edit-real-file.png");
        self.editor.source.set_active(true);
        if !self
            .visible_text()
            .starts_with("// Native source editing fixture")
        {
            return Err("Source did not refresh after saving".into());
        }
        self.close.emit_clicked();
        Ok(
            serde_json::json!({"ok":true,"bytes":original.len(),"firstSaveMs":save_ms,"saveHeartbeatTicks":ticks.get(),"saveMaxHeartbeatGapMs":gap.get(),"explicitSave":true,"dirtyViewSwitch":true,"undoRedoAcrossSave":true,"pageChangeSave":true,"replacementCancelSave":true,"conflictCancelOverwrite":true,"closeCancelDiscard":true,"windowCloseGuard":true,"inflightCloseGuard":true,"readOnlyShared":true,"realFileSave":true,"exactCrlf":true}),
        )
    }
    pub(crate) fn edit_insert(&self, text: &str) {
        let buffer = self.editor.view.buffer();
        buffer.begin_user_action();
        let mut at = buffer.start_iter();
        buffer.insert(&mut at, text);
        buffer.place_cursor(&at);
        buffer.end_user_action();
    }
    pub(crate) fn edit_text(&self) -> String {
        let buffer = self.editor.view.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string()
    }
    pub(crate) async fn edit_response(&self, id: &str) -> Result<(), String> {
        let start = Instant::now();
        loop {
            let guard = self.editor.guard.borrow().clone();
            if let Some(guard) = guard.filter(|g| g.is_mapped()) {
                if !guard.has_response(id) {
                    return Err("Missing native editor response".into());
                }
                let label = guard.response_label(id);
                fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
                    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                        if button.label().as_deref() == Some(label) {
                            return Some(button.clone());
                        }
                    }
                    let mut child = widget.first_child();
                    while let Some(current) = child {
                        child = current.next_sibling();
                        if let Some(button) = find(&current, label) {
                            return Some(button);
                        }
                    }
                    None
                }
                if let Some(button) = find(guard.upcast_ref(), &label) {
                    button.emit_clicked();
                    return Ok(());
                }
            }
            if start.elapsed() > Duration::from_secs(15) {
                return Err(format!(
                    "Native editor response not available: {id}; status={}",
                    self.status.text()
                ));
            }
            glib::timeout_future(Duration::from_millis(10)).await;
        }
    }
    pub(crate) fn smoke_edit_mode(&self, editing: bool) {
        if editing {
            self.editor.edit.set_active(true);
        } else {
            self.editor.source.set_active(true);
        }
    }
    pub(crate) fn smoke_edit_history(&self, redo: bool) {
        if redo {
            self.editor.redo.emit_clicked();
        } else {
            self.editor.undo.emit_clicked();
        }
    }
}
