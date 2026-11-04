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
            return Err(format!("Session visibility timed out: {step}"));
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
    pub async fn smoke_close_replacement(self: &Rc<Self>) -> Result<(), String> {
        until("replacement close setup", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        self.fields[0].replace.emit_clicked();
        if self.confirmation.borrow().is_none() {
            return Err("Replacement dialog was not opened".into());
        }
        self.dialog.force_close();
        if self.confirmation.borrow().is_some() {
            return Err("Replacement dialog survived its editor".into());
        }
        Ok(())
    }
    pub async fn smoke_recovery(
        self: &Rc<Self>,
        directory: &Path,
        parent_guard: impl Fn() -> bool,
        check: impl Fn(bool) -> bool,
        capture: impl Fn(&adw::PreferencesDialog, &str),
    ) -> Result<Value, String> {
        until("recovery load", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        if self.available(0)
            || self.available(1)
            || self.dirty()
            || self.save.is_sensitive()
            || !self.status.text().contains("cannot be applied")
            || !check(false)
        {
            return Err("Malformed values were applied or sent to text layout".into());
        }
        let path = directory.join("config/codex-thread-studio/settings.json");
        let source = path.clone();
        let before = gio::spawn_blocking(move || std::fs::read(source).map_err(|e| e.to_string()))
            .await
            .map_err(|_| "Recovery read worker stopped")??;
        capture(&self.dialog, "visibility-recovery.png");
        self.fields[0].replace.emit_clicked();
        self.answer("Keep original")?;
        if self.available(0) || self.dirty() {
            return Err("Replacement cancellation changed the editor".into());
        }
        for index in 0..2 {
            self.fields[index].replace.emit_clicked();
            self.answer("Replace value")?;
        }
        if !self.dirty() || !self.save.is_sensitive() || !parent_guard() {
            return Err("Replacement lost explicit-save/parent guards".into());
        }
        self.reload.emit_clicked();
        self.answer("Keep editing")?;
        if !self.dirty() {
            return Err("Reload cancel discarded replacements".into());
        }
        let source = path.clone();
        let unchanged =
            gio::spawn_blocking(move || std::fs::read(source).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Recovery read worker stopped")??;
        if unchanged != before || !check(false) {
            return Err("Replacement wrote or applied before Save".into());
        }
        self.reload.emit_clicked();
        self.answer("Discard changes")?;
        until("recovery reload", || !self.busy.get()).await?;
        if self.available(0) || self.available(1) || self.dirty() {
            return Err("Discard did not restore raw saved values".into());
        }
        for index in 0..2 {
            self.fields[index].replace.emit_clicked();
            self.answer("Replace value")?;
        }
        let hidden = directory.join("hidden").to_string_lossy().into_owned();
        let project = directory.join("project").to_string_lossy().into_owned();
        let rules = format!("# Native rules 中文\n**/build/[dk]*/\n!{project}/build/keep/");
        self.hidden.set_text(&hidden);
        self.rules.set_text(&rules);
        self.save.emit_clicked();
        until("recovery save", || !self.busy.get()).await?;
        if self.dirty() {
            return Err(format!("Recovery save failed: {}", self.status.text()));
        }
        until("recovered list", || check(true)).await?;
        // A readable but invalid list should stay editable, including after an external edit.
        let source = path.clone();
        gio::spawn_blocking(move || -> Result<(), String> {
            let mut raw: Value =
                serde_json::from_slice(&std::fs::read(&source).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            raw["sessionDirectoryIgnore"] = json!(vec!["scratch-*"; 513]);
            std::fs::write(source, serde_json::to_vec(&raw).unwrap()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Recovery write worker stopped")??;
        self.reload.emit_clicked();
        until("invalid list reload", || !self.busy.get()).await?;
        if !self.available(0)
            || !self.available(1)
            || !self.fields[1].view.is_sensitive()
            || self.dirty()
        {
            return Err("Readable invalid rules were not editable".into());
        }
        until("invalid list closed", || check(false)).await?;
        let invalid = text(&self.rules);
        self.rules.set_text("# temporary edit");
        self.rules.set_text(&invalid);
        if self.dirty() || !self.status.text().contains("cannot be applied") {
            return Err("Reverting an invalid saved list hid its validation error".into());
        }
        self.rules.set_text(&rules);
        self.save.emit_clicked();
        until("count repair", || !self.busy.get()).await?;
        until("count repaired list", || check(true)).await?;
        capture(&self.dialog, "visibility-recovered.png");
        Ok(
            json!({"ok":true,"rawRecovery":true,"explicitReplacement":true,"cancelDiscard":true,"savedBytesPreservedUntilSave":true,"oneMiBLineNotLaidOut":true,"readableInvalidListEditable":true,"unrelatedSettingsPreserved":true}),
        )
    }
    fn answer(&self, label: &str) -> Result<(), String> {
        let alert = self
            .confirmation
            .borrow()
            .clone()
            .ok_or("Discard alert missing")?;
        button(alert.upcast_ref(), label)
            .ok_or("Discard button missing")?
            .emit_clicked();
        Ok(())
    }
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &Path,
        restore: bool,
        parent_guard: impl Fn() -> bool,
        check: impl Fn(&str) -> bool,
        capture: impl Fn(&adw::PreferencesDialog, &str),
    ) -> Result<Value, String> {
        until("load", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        let hidden = directory.join("hidden").to_string_lossy().into_owned();
        let project = directory.join("project").to_string_lossy().into_owned();
        let rules = format!("# Native rules 中文\n**/build/[dk]*/\n!{project}/build/keep/");
        if restore {
            if text(&self.hidden) != hidden || text(&self.rules) != rules {
                return Err("Saved visibility did not survive restart".into());
            }
            until("restored list", || check("restore")).await?;
            capture(&self.dialog, "visibility-restored.png");
            return Ok(json!({"ok":true,"restored":true}));
        }
        self.hidden.set_text(&format!("{hidden}\n{project}"));
        self.rules.set_text(&rules);
        if !self.dirty() || !self.save.is_sensitive() || self.may_close() || !parent_guard() {
            return Err("Dirty visibility did not protect Settings".into());
        }
        self.dialog.close();
        self.answer("Keep editing")?;
        if !self.dirty() || !self.dialog.is_visible() {
            return Err("Close cancel discarded visibility edits".into());
        }
        self.rules.set_text("bad\trule");
        self.save.emit_clicked();
        until("invalid rule", || !self.busy.get()).await?;
        if !self.dirty() || text(&self.rules) != "bad\trule" {
            return Err("Rejected rule discarded edits".into());
        }
        self.rules.set_text(&rules);
        self.save.emit_clicked();
        if !self.busy.get() || self.dialog.can_close() {
            return Err("Busy visibility lost close protection".into());
        }
        until("save", || !self.busy.get()).await?;
        if self.dirty() {
            return Err(format!("Visibility save failed: {}", self.status.text()));
        }
        until("hidden list", || check("hide")).await?;
        capture(&self.dialog, "visibility-hidden.png");
        self.hidden.set_text(&hidden);
        self.reload.emit_clicked();
        self.answer("Keep editing")?;
        if !self.dirty() {
            return Err("Reload cancel lost edits".into());
        }
        self.reload.emit_clicked();
        self.answer("Discard changes")?;
        until("reload", || !self.busy.get()).await?;
        if text(&self.hidden) != format!("{hidden}\n{project}") || self.dirty() {
            return Err("Reload failed".into());
        }
        self.hidden.set_text(&hidden);
        self.save.emit_clicked();
        until("restore save", || !self.busy.get()).await?;
        until("restored list", || check("restore")).await?;
        let settings = directory.join("config/codex-thread-studio/settings.json");
        gio::spawn_blocking(move || -> Result<(), String> {
            let mut raw: Value =
                serde_json::from_slice(&std::fs::read(&settings).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            raw["sessionDirectoryIgnore"] = json!(["**/concurrent/"]);
            raw["futureVisibility"] = json!({"preserve":true});
            std::fs::write(settings, serde_json::to_vec(&raw).unwrap()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Visibility fixture worker stopped")??;
        self.hidden.set_text(&format!("{hidden}\n{project}"));
        self.save.emit_clicked();
        until("conflict", || !self.busy.get()).await?;
        if !self.dirty()
            || !self.status.text().contains("Reload before saving")
            || text(&self.hidden) != format!("{hidden}\n{project}")
        {
            return Err(format!("Conflict lost edits: {}", self.status.text()));
        }
        if !check("restore") {
            return Err("Rejected visibility was applied to the list".into());
        }
        capture(&self.dialog, "visibility-conflict.png");
        self.reload.emit_clicked();
        self.answer("Discard changes")?;
        until("conflict reload", || !self.busy.get()).await?;
        until("external rules", || check("reset")).await?;
        if text(&self.rules) != "**/concurrent/" {
            return Err("Reload missed external rules".into());
        }
        self.rules.set_text(&rules);
        self.save.emit_clicked();
        until("final save", || !self.busy.get()).await?;
        until("final list", || check("restore")).await?;
        if self.dirty() {
            return Err("Final rules were not saved".into());
        }
        Ok(
            json!({"ok":true,"explicitSave":true,"liveVisibility":true,"selectedSessionPreserved":true,"orderedUnignore":true,"invalidRetained":true,"conflictRetainedEdits":true,"reloadCancelDiscard":true,"closeGuards":true,"folderChooserPortal":"not exercised"}),
        )
    }
}
