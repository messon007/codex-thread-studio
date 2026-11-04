//! Exercises actual controls against a private preferences file, never user data.
use super::*;
use gtk::glib;
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(12) {
            return Err(format!("Shared folders timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
fn button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(b) = widget
        .downcast_ref::<gtk::Button>()
        .filter(|b| b.label().as_deref() == Some(label))
    {
        return Some(b.clone());
    }
    let mut child = widget.first_child();
    while let Some(w) = child {
        if let Some(b) = button(&w, label) {
            return Some(b);
        }
        child = w.next_sibling();
    }
    None
}
impl Panel {
    fn answer(&self, label: &str) -> Result<(), String> {
        let alert = self
            .confirmation
            .borrow()
            .clone()
            .ok_or("Discard confirmation missing")?;
        let button =
            button(alert.upcast_ref(), label).ok_or("Native confirmation button missing")?;
        button.emit_clicked();
        Ok(())
    }
    fn remove_first(&self) -> Result<(), String> {
        let row = self
            .rows
            .borrow()
            .first()
            .cloned()
            .ok_or("Folder row missing")?;
        fn find(w: &gtk::Widget) -> Option<gtk::Button> {
            if let Some(b) = w
                .downcast_ref::<gtk::Button>()
                .filter(|b| b.icon_name().as_deref() == Some("list-remove-symbolic"))
            {
                return Some(b.clone());
            }
            let mut child = w.first_child();
            while let Some(w) = child {
                if let Some(b) = find(&w) {
                    return Some(b);
                }
                child = w.next_sibling();
            }
            None
        }
        find(row.upcast_ref())
            .ok_or("Remove control missing")?
            .emit_clicked();
        Ok(())
    }
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &Path,
        restore: bool,
        parent_guard: impl Fn() -> bool,
        capture: impl Fn(&adw::PreferencesDialog, &str),
    ) -> Result<Value, String> {
        until("load", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        let expected = vec![
            directory
                .join("shared docs 中文 #")
                .to_str()
                .unwrap()
                .to_owned(),
            directory
                .join("unmounted 中文 # ")
                .to_str()
                .unwrap()
                .to_owned(),
        ];
        if restore {
            if *self.paths.borrow() != expected || !self.may_close() {
                return Err("Shared folders did not survive restart".into());
            }
            capture(&self.dialog, "shared-settings-restored.png");
            return Ok(json!({"ok":true,"restored":true}));
        }
        self.input.set_text("relative-folder");
        self.add.emit_clicked();
        if !self.paths.borrow().is_empty()
            || self.input.text().as_str() != "relative-folder"
            || self.save.is_sensitive()
        {
            return Err("Invalid path was accepted or omitted from Save".into());
        }
        self.input.set_text("");
        for path in &expected {
            self.input.set_text(path);
            self.add.emit_clicked();
        }
        if !self.dirty() || !self.save.is_sensitive() || self.may_close() || !parent_guard() {
            return Err("Dirty child did not protect Settings".into());
        }
        self.dialog.close();
        self.answer("Keep editing")?;
        if !self.dirty() || !self.dialog.is_visible() {
            return Err("Cancel discarded folder edits".into());
        }
        self.save.emit_clicked();
        if !self.busy.get() || self.dialog.can_close() {
            return Err("Saving did not protect dialog closure".into());
        }
        until("save", || !self.busy.get()).await?;
        if self.dirty() || *self.paths.borrow() != expected {
            return Err(format!("Save failed: {}", self.status.text()));
        }
        capture(&self.dialog, "shared-settings-saved.png");
        self.remove_first()?;
        self.reload.emit_clicked();
        self.answer("Keep editing")?;
        if !self.dirty() || self.paths.borrow().len() != 1 {
            return Err("Reload cancel lost edits".into());
        }
        self.reload.emit_clicked();
        self.answer("Discard changes")?;
        until("reload", || !self.busy.get()).await?;
        if *self.paths.borrow() != expected || self.dirty() {
            return Err("Reload did not restore saved folders".into());
        }
        // Simulate a trusted concurrent preferences writer while the editor stays open.
        let settings = directory.join("config/codex-thread-studio/settings.json");
        let other = directory
            .join("concurrent shared")
            .to_str()
            .unwrap()
            .to_owned();
        let other_copy = other.clone();
        gio::spawn_blocking(move || -> Result<(), String> {
            let mut raw: Value =
                serde_json::from_slice(&std::fs::read(&settings).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            raw["sharedDocumentDirectories"] = json!([other_copy]);
            raw["futureSharedSettings"] = json!({"preserve":true});
            std::fs::write(settings, serde_json::to_vec(&raw).unwrap()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Concurrent settings worker stopped")??;
        self.remove_first()?;
        let unsaved = self.paths.borrow().clone();
        self.save.emit_clicked();
        until("conflict", || !self.busy.get()).await?;
        if !self.dirty()
            || *self.paths.borrow() != unsaved
            || !self.status.text().contains("Reload before saving")
        {
            return Err(format!("Conflict lost edits: {}", self.status.text()));
        }
        capture(&self.dialog, "shared-settings-conflict.png");
        self.reload.emit_clicked();
        self.answer("Discard changes")?;
        until("conflict reload", || !self.busy.get()).await?;
        if *self.paths.borrow() != vec![other] || self.dirty() {
            return Err("Conflict reload failed".into());
        }
        self.remove_first()?;
        for path in &expected {
            self.input.set_text(path);
            self.add.emit_clicked();
        }
        self.save.emit_clicked();
        until("final save", || !self.busy.get()).await?;
        if self.dirty() {
            return Err(format!("Final save failed: {}", self.status.text()));
        }
        self.input.set_text("/not-yet-added");
        if self.save.is_sensitive() || self.may_close() {
            return Err("Unadded typed path was silently ignored".into());
        }
        self.reload.emit_clicked();
        self.answer("Discard changes")?;
        until("input reload", || !self.busy.get()).await?;
        if !self.input.text().is_empty() || self.dirty() {
            return Err("Reload did not discard pending input".into());
        }
        Ok(
            json!({"ok":true,"explicitSave":true,"closeGuards":true,"reloadCancelDiscard":true,
            "conflictRetainedEdits":true,"unavailableLiteralPath":true,"typedInputGuard":true,
            "folderChooserPortal":"not exercised"}),
        )
    }
}

impl Panel {
    pub async fn smoke_portal_picker(self: &Rc<Self>, root: &Path) -> Result<Value, String> {
        let trace = crate::smoke_portal::read(root).await?;
        let calls = trace["calls"].as_array().map_or(0, Vec::len);
        let closed = trace["closed"].as_array().map_or(0, Vec::len);
        until("shared picker ready", || {
            self.snapshot.borrow().is_some() && !self.busy.get()
        })
        .await?;
        let expected = root
            .join("picked shared 中文 # ")
            .to_string_lossy()
            .into_owned();
        let settings = root.join("config/codex-thread-studio/settings.json");
        let original = gio::spawn_blocking(move || std::fs::read(settings))
            .await
            .map_err(|_| "Settings fixture worker stopped")?
            .map_err(|e| e.to_string())?;
        self.choose.emit_clicked();
        crate::smoke_portal::wait(root, "calls", calls + 1).await?;
        until("selected folder added", || {
            self.picker.borrow().is_none() && self.paths.borrow().contains(&expected)
        })
        .await?;
        if !self.dirty() {
            return Err("Choosing a folder silently persisted it".into());
        }
        let settings = root.join("config/codex-thread-studio/settings.json");
        let unchanged = gio::spawn_blocking(move || std::fs::read(settings))
            .await
            .map_err(|_| "Settings fixture worker stopped")?
            .map_err(|e| e.to_string())?;
        if unchanged != original {
            return Err("Chooser wrote shared settings before Save".into());
        }
        self.save.emit_clicked();
        until("explicit folder Save", || {
            !self.busy.get()
                && !self.dirty()
                && self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .is_some_and(|s| s.directories.contains(&expected))
        })
        .await?;
        self.choose.emit_clicked();
        crate::smoke_portal::wait(root, "calls", calls + 2).await?;
        self.dialog.close();
        crate::smoke_portal::wait(root, "closed", closed + 1).await?;
        until("folder picker cancelled on close", || {
            self.picker.borrow().is_none() && !self.dialog.is_mapped()
        })
        .await?;
        Ok(
            json!({"folderSelection":true,"literalUnicodeAndTrailingSpace":true,"explicitSave":true,"panelCloseCancellation":true}),
        )
    }
}
