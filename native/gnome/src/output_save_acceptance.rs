//! SaveFile transport, cancelled choice isolation and admitted snapshot exports.
use super::*;
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!("Output Save timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
async fn exists(path: std::path::PathBuf) -> Result<bool, String> {
    gio::spawn_blocking(move || path.is_file())
        .await
        .map_err(|_| "Export fixture worker stopped".into())
}
async fn file_ready(root: &Path, name: &str) -> Result<(), String> {
    let at = Instant::now();
    while !exists(root.join(name)).await? {
        if at.elapsed() > Duration::from_secs(10) {
            return Err(format!("Export fixture file missing: {name}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
impl Host {
    pub async fn smoke_save_portal(
        self: &Rc<Self>,
        root: &Path,
        capture: impl Fn(&adw::Dialog),
        attempt_application_close: impl Fn(),
    ) -> Result<Value, String> {
        let fixture = root.to_owned();
        let (a, b) = gio::spawn_blocking(move || {
            let a = std::fs::read_to_string(fixture.join("snapshot-a.txt"))
                .map_err(|e| e.to_string())?;
            let b = std::fs::read_to_string(fixture.join("snapshot-b.txt"))
                .map_err(|e| e.to_string())?;
            Ok::<_, String>((
                crate::conversation_output::retain(a),
                crate::conversation_output::retain(b),
            ))
        })
        .await
        .map_err(|_| "Snapshot fixture worker stopped")??;
        self.present(a.clone(), "Assistant");
        let first = self
            .panel
            .borrow()
            .clone()
            .ok_or("Snapshot viewer missing")?;
        until("snapshot A ready", || {
            first.page.borrow().is_some() && !first.busy.get() && first.dialog.is_mapped()
        })
        .await?;
        first.buffer.select_range(
            &first.buffer.iter_at_offset(120),
            &first.buffer.iter_at_offset(160),
        );
        let selected = first
            .buffer
            .selection_bounds()
            .map(|(a, z)| first.buffer.text(&a, &z, false).to_string());
        let page = first.page.borrow().as_ref().unwrap().index;
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 1).await?;
        until("Save cancelled", || {
            !first.exporting.get() && first.export_picker.borrow().is_none()
        })
        .await?;
        if !first.export.is_sensitive() {
            return Err("Cancel left Export disabled".into());
        }
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 2).await?;
        until("nonlocal Save rejected", || {
            !first.exporting.get() && first.notice.text() == "Choose a local path for export"
        })
        .await?;
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 3).await?;
        until("empty selection error visible", || {
            !first.exporting.get()
                && first
                    .notice
                    .text()
                    .starts_with("Unable to open export chooser:")
        })
        .await?;
        let retained = first
            .buffer
            .selection_bounds()
            .map(|(a, z)| first.buffer.text(&a, &z, false).to_string());
        if selected.is_none()
            || selected != retained
            || first.view.is_editable()
            || first.page.borrow().as_ref().is_none_or(|p| p.index != page)
        {
            return Err(
                "Save cancellation/errors changed the read-only source page or selection".into(),
            );
        }
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 4).await?;
        until("snapshot A exported", || {
            !first.exporting.get() && first.notice.text() == "Exported complete snapshot"
        })
        .await?;
        file_ready(root, "first-output 中文 #.txt").await?;
        capture(&first.dialog);
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 5).await?;
        self.choose_export(first.clone());
        self.choose_export(first.clone());
        glib::timeout_future(Duration::from_millis(120)).await;
        if crate::smoke_portal::read(root).await?["calls"]
            .as_array()
            .map(Vec::len)
            != Some(5)
        {
            return Err("Repeated Export opened multiple Save dialogs".into());
        }
        if !self.allow_window_close() || !first.export.is_sensitive() || first.exporting.get() {
            return Err("Cancelling an unconfirmed Save left the viewer blocked".into());
        }
        // Reopen in the same turn, before the old cancellation callback runs.
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "closed", 1).await?;
        crate::smoke_portal::wait(root, "calls", 6).await?;
        if first.export_picker.borrow().is_none() || !first.exporting.get() {
            return Err("Old Save cancellation cleared a newer chooser".into());
        }
        first.dialog.force_close();
        crate::smoke_portal::wait(root, "closed", 2).await?;
        until("first viewer closed", || self.panel.borrow().is_none()).await?;
        self.present(a, "Assistant");
        let old = self.panel.borrow().clone().unwrap();
        until("second viewer ready", || {
            old.page.borrow().is_some() && !old.busy.get() && old.dialog.is_mapped()
        })
        .await?;
        old.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 7).await?;
        self.present(b, "Assistant");
        crate::smoke_portal::wait(root, "closed", 3).await?;
        let current = self.panel.borrow().clone().unwrap();
        until("replacement snapshot ready", || {
            current.page.borrow().is_some() && !current.busy.get() && current.dialog.is_mapped()
        })
        .await?;
        current.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 8).await?;
        file_ready(root, "export-worker-started").await?;
        if current.export_picker.borrow().is_some() || !current.exporting.get() {
            return Err("Export was not admitted before closing its viewer".into());
        }
        attempt_application_close();
        glib::timeout_future(Duration::from_millis(50)).await;
        if self.parent.upgrade().is_none_or(|p| !p.is_mapped()) || self.allow_window_close() {
            return Err("Application closed while a confirmed export was still writing".into());
        }
        current.dialog.force_close();
        until("confirmed export viewer closed", || {
            self.panel.borrow().is_none()
        })
        .await?;
        // libadwaita may consume a window close to dismiss its visible dialog.
        // Repeat with no viewer so the application's pending-write guard runs.
        attempt_application_close();
        glib::timeout_future(Duration::from_millis(50)).await;
        if self.parent.upgrade().is_none_or(|p| !p.is_mapped()) || self.allow_window_close() {
            return Err("Application closed with a confirmed export after viewer dismissal".into());
        }
        let path = root.join("export-worker-release");
        gio::spawn_blocking(move || std::fs::write(path, b"release"))
            .await
            .map_err(|_| "Export fixture worker stopped")?
            .map_err(|e| e.to_string())?;
        file_ready(root, "confirmed-output.txt").await?;
        until("accepted export completion acknowledged", || {
            self.pending_exports.borrow().is_empty()
        })
        .await?;
        glib::timeout_future(Duration::from_millis(150)).await;
        if first.export_picker.borrow().is_some() || old.export_picker.borrow().is_some() {
            return Err("Closed viewers kept a Save chooser".into());
        }
        Ok(
            json!({"cancel":true,"errorsVisible":true,"nonlocalRejected":true,"duplicateSaveCoalesced":true,"closeCancellation":true,"replacementCancellation":true,"sameViewerReopen":true,"parentChoiceCancellation":true,"immutableSnapshotExport":true,"admittedWriteAfterClose":true,"applicationCloseGuard":true,"readOnlyPageAndSelectionRetained":true}),
        )
    }
}
