//! Reload through native controls and the real isolated file service.
use super::*;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
impl Source {
    pub(crate) async fn smoke_reload(&self, root: &Path) -> Result<serde_json::Value, String> {
        async fn until(step: &str, check: impl Fn() -> bool) -> Result<(), String> {
            crate::acceptance_wait::until(step, check).await
        }
        async fn write(path: PathBuf, text: &str) -> Result<(), String> {
            let text = text.to_owned();
            gtk::gio::spawn_blocking(move || std::fs::write(path, text).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Fixture writer stopped")?
        }
        async fn read(path: PathBuf) -> Result<Vec<u8>, String> {
            gtk::gio::spawn_blocking(move || std::fs::read(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Fixture reader stopped")?
        }
        let project = root.join("project");
        let project = project.to_str().unwrap();
        let path = root.join("project/reload.rs");
        self.open(project, "reload.rs");
        until("ordinary file", || {
            self.text.borrow().is_some() && !self.reading.get()
        })
        .await?;
        self.page(true);
        if self.window.borrow().as_ref().is_none_or(|w| w.line <= 1) {
            return Err("Initial page was not advanced".into());
        }
        self.editor.reload.emit_clicked();
        until("unchanged file context", || {
            !self.reading.get() && self.text.borrow().is_some()
        })
        .await?;
        if self.window.borrow().as_ref().is_none_or(|w| w.line <= 1) {
            return Err("Unchanged Reload lost page context".into());
        }
        write(path.clone(), "// External short 中文😀\r\n").await?;
        self.editor.reload.emit_clicked();
        until("external short file", || {
            !self.reading.get() && self.visible_text() == "// External short 中文😀\r\n"
        })
        .await?;
        if self.editor.editing.get() || self.window.borrow().as_ref().is_none_or(|w| w.line != 1) {
            return Err("Shortened file opened an invalid/empty context".into());
        }
        self.editor.edit.set_active(true);
        until("reload editor", || {
            !self.editor.busy.get() && self.editor.page.borrow().is_some()
        })
        .await?;
        self.edit_insert("// Cancelled draft\n");
        self.editor.reload.emit_clicked();
        self.edit_response("cancel").await?;
        if !self.editor.dirty.get()
            || !self.edit_text().starts_with("// Cancelled draft\n")
            || read(path.clone()).await?
                != b"// External short \xe4\xb8\xad\xe6\x96\x87\xf0\x9f\x98\x80\r\n"
        {
            return Err("Cancelled Reload changed edits or disk".into());
        }
        self.editor.reload.emit_clicked();
        self.edit_response("discard").await?;
        until("discard and reload editor", || {
            !self.reading.get()
                && !self.editor.busy.get()
                && self.editor.editing.get()
                && self.editor.page.borrow().is_some()
        })
        .await?;
        if self.editor.dirty.get() || self.edit_text().starts_with("// Cancelled") {
            return Err("Reload discard did not restore Edit from disk".into());
        }
        self.edit_insert("// Saved on reload\n");
        self.editor.reload.emit_clicked();
        self.edit_response("save").await?;
        until("save then reload editor", || {
            !self.reading.get()
                && !self.editor.busy.get()
                && !self.editor.dirty.get()
                && self.editor.editing.get()
        })
        .await?;
        let saved = "// Saved on reload\r\n// External short 中文😀\r\n";
        if read(path.clone()).await? != saved.as_bytes()
            || !self.edit_text().starts_with("// Saved on reload\n")
        {
            return Err("Save/Reload changed CRLF or lost Edit".into());
        }
        self.edit_insert("// Conflict draft\n");
        write(path.clone(), "// External conflict 中文😀\r\n").await?;
        self.editor.reload.emit_clicked();
        self.edit_response("save").await?;
        until("reload save conflict", || {
            self.editor
                .guard
                .borrow()
                .as_ref()
                .is_some_and(|g| g.has_response("overwrite"))
        })
        .await?;
        self.edit_response("cancel").await?;
        if !self.editor.dirty.get() || !self.edit_text().starts_with("// Conflict draft") {
            return Err("Conflict cancellation lost edit".into());
        }
        self.editor.reload.emit_clicked();
        self.edit_response("discard").await?;
        until("external conflict version loaded", || {
            !self.reading.get()
                && !self.editor.busy.get()
                && self.editor.editing.get()
                && self.edit_text() == "// External conflict 中文😀\n"
        })
        .await?;
        self.editor.source.set_active(true);
        let removed = path.clone();
        gtk::gio::spawn_blocking(move || std::fs::remove_file(removed).map_err(|e| e.to_string()))
            .await
            .map_err(|_| "Fixture removal stopped")??;
        self.editor.reload.emit_clicked();
        until("failed read remains retryable", || {
            !self.reading.get() && self.text.borrow().is_none()
        })
        .await?;
        if !self.editor.reload.is_visible() || !self.editor.reload.is_sensitive() {
            return Err("Read error removed Reload recovery".into());
        }
        write(path.clone(), "// Restored after missing file\r\n").await?;
        self.editor.reload.emit_clicked();
        until("read retry", || {
            !self.reading.get() && self.visible_text() == "// Restored after missing file\r\n"
        })
        .await?;
        self.editor.reload.emit_clicked();
        self.open(project, "other.rs");
        until("stale reload superseded", || {
            !self.reading.get()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.path.ends_with("other.rs"))
        })
        .await?;
        if self.editor.editing.get() || self.preview_active() {
            return Err("Old Reload restored its mode over a new file".into());
        }
        self.open(project, root.join("readonly/shared.json").to_str().unwrap());
        until("read-only Preview", || {
            !self.reading.get() && self.preview_active()
        })
        .await?;
        write(root.join("readonly/shared.json"), "{\"version\":2}").await?;
        self.editor.reload.emit_clicked();
        until("read-only Preview reloaded", || {
            !self.reading.get()
                && self.preview_active()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.document.as_str().contains(":2"))
        })
        .await?;
        if self.writable_source() || self.editor.edit.is_visible() {
            return Err("Reload promoted a shared file to writable".into());
        }
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if self.editor.reload.tooltip_text().as_deref() != Some("重新加载已保存的文档") {
            return Err("Reload label did not change language".into());
        }
        let window = self
            .widget
            .root()
            .and_downcast::<gtk::Window>()
            .ok_or("No native window")?;
        crate::window_capture::capture(&window, &root.join("reload-readonly.png")).await?;
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.open(project, "reload.html");
        until("HTML original", || {
            !self.reading.get() && self.html_active() && self.editor.html.ready.get()
        })
        .await?;
        write(
            root.join("project/reload.html"),
            "<h1>Reloaded HTML 中文😀</h1>",
        )
        .await?;
        self.editor.reload.emit_clicked();
        until("HTML Preview reloaded", || {
            !self.reading.get()
                && self.html_active()
                && self.editor.html.ready.get()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.document.as_str().contains("Reloaded HTML"))
        })
        .await?;
        self.open_now(
            "review.diff",
            1,
            Request::Diff {
                root: project.into(),
                path: "reload.rs".into(),
                scope: "working".into(),
                commit: None,
            },
        );
        // The fake project is not a Git repository. Even a failed historical/
        // synthetic diff must never gain a working-file Reload target.
        until("nonfile response", || !self.reading.get()).await?;
        if self.editor.reload.is_visible() || self.reload_target.borrow().is_some() {
            return Err("A diff acquired file Reload".into());
        }
        self.close.emit_clicked();
        gtk::glib::timeout_future(Duration::from_millis(50)).await;
        Ok(
            serde_json::json!({"ok":true,"unchangedContext":true,"shortenedContext":true,"cancelPreservesDraft":true,"discardReloadsEditor":true,"saveReloadsEditor":true,"conflictCancel":true,"missingFileRetry":true,"staleReply":true,"readOnly":true,"htmlPreview":true,"liveLanguage":true,"noDiffReload":true,"exactCrlf":true}),
        )
    }
}
