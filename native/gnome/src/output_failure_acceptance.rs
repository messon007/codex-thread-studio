//! Real failed writes after closed/replaced viewers; queued errors keep context.
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
            return Err(format!("Write failure timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn started(root: &Path, name: &str) -> Result<(), String> {
    let at = Instant::now();
    loop {
        let path = root.join(format!("{name}-started"));
        if gio::spawn_blocking(move || path.is_file())
            .await
            .map_err(|_| "Write probe stopped")?
        {
            return Ok(());
        }
        if at.elapsed() > Duration::from_secs(10) {
            return Err(format!("Write was not admitted: {name}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
async fn release(root: &Path, name: &str) -> Result<(), String> {
    let path = root.join(format!("{name}-release"));
    gio::spawn_blocking(move || std::fs::write(path, b"release"))
        .await
        .map_err(|_| "Write probe stopped")?
        .map_err(|e| e.to_string())
}
impl Host {
    async fn failure_panel(
        self: &Rc<Self>,
        source: Reference,
        title: &str,
    ) -> Result<Rc<Panel>, String> {
        self.present(source, title);
        let p = self.panel.borrow().clone().ok_or("Output viewer missing")?;
        until("mapped viewer", || {
            p.page.borrow().is_some() && !p.busy.get() && p.dialog.is_mapped()
        })
        .await?;
        Ok(p)
    }
    pub async fn smoke_write_failures(
        self: &Rc<Self>,
        root: &Path,
        close_window: impl Fn(),
    ) -> Result<Value, String> {
        let path = root.to_owned();
        let (a, b) = gio::spawn_blocking(move || {
            let a =
                std::fs::read_to_string(path.join("snapshot-a.txt")).map_err(|e| e.to_string())?;
            let b =
                std::fs::read_to_string(path.join("snapshot-b.txt")).map_err(|e| e.to_string())?;
            Ok::<_, String>((
                crate::conversation_output::retain(a),
                crate::conversation_output::retain(b),
            ))
        })
        .await
        .map_err(|_| "Snapshot worker stopped")??;
        let first = self
            .failure_panel(a.clone(), "Closed source 中文😀")
            .await?;
        first.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 1).await?;
        started(root, "failed-output-closed").await?;
        first.dialog.force_close();
        until("viewer closed", || self.panel.borrow().is_none()).await?;
        close_window();
        glib::timeout_future(Duration::from_millis(50)).await;
        if self.parent.upgrade().is_none_or(|p| !p.is_mapped()) || self.allow_window_close() {
            return Err("Pending export did not guard window close".into());
        }
        release(root, "failed-output-closed").await?;
        let alert = self
            .feedback
            .smoke_wait("Closed source 中文😀", "failed-output-closed.txt")
            .await?;
        self.feedback
            .smoke_capture(&root.join("write-error-closed.png"))
            .await?;
        if !self.pending_exports.borrow().is_empty() || !self.feedback.blocks_close() {
            return Err("Failed export did not finish before its error was acknowledged".into());
        }
        crate::write_feedback::Queue::smoke_ack(&alert)?;
        self.feedback.smoke_clear().await?;
        self.exported(
            first.id,
            first.export_serial.get(),
            Err("Duplicate late completion".into()),
        );
        if self.feedback.blocks_close() {
            return Err("Duplicate completion repeated the export failure".into());
        }

        let old = self
            .failure_panel(a.clone(), "Previous source 中文😀")
            .await?;
        old.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 2).await?;
        started(root, "failed-output-replaced").await?;
        let current = self.failure_panel(b.clone(), "Replacement source").await?;
        current.buffer.select_range(
            &current.buffer.iter_at_offset(20),
            &current.buffer.iter_at_offset(45),
        );
        let selection = current
            .buffer
            .selection_bounds()
            .map(|(s, e)| current.buffer.text(&s, &e, false).to_string());
        let page = current.page.borrow().as_ref().unwrap().index;
        release(root, "failed-output-replaced").await?;
        let alert = self
            .feedback
            .smoke_wait("Previous source 中文😀", "failed-output-replaced.txt")
            .await?;
        if current.exporting.get()
            || !current.export.is_sensitive()
            || current.notice.text().starts_with("Export failed")
            || current
                .page
                .borrow()
                .as_ref()
                .is_none_or(|p| p.index != page)
            || current
                .buffer
                .selection_bounds()
                .map(|(s, e)| current.buffer.text(&s, &e, false).to_string())
                != selection
        {
            return Err("Previous export completion changed the replacement viewer".into());
        }
        self.feedback
            .smoke_capture(&root.join("write-error-replaced.png"))
            .await?;
        crate::write_feedback::Queue::smoke_ack(&alert)?;
        self.feedback.smoke_clear().await?;

        let queued_a = self.failure_panel(a, "Queued source A").await?;
        queued_a.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 3).await?;
        started(root, "failed-output-queue-a").await?;
        let queued_b = self.failure_panel(b, "Queued source B").await?;
        queued_b.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 4).await?;
        started(root, "failed-output-queue-b").await?;
        queued_b.dialog.force_close();
        until("queued viewers closed", || self.panel.borrow().is_none()).await?;
        release(root, "failed-output-queue-a").await?;
        let first_error = self
            .feedback
            .smoke_wait("Queued source A", "failed-output-queue-a.txt")
            .await?;
        release(root, "failed-output-queue-b").await?;
        until("two distinct failures retained", || {
            self.feedback.smoke_count() == 2 && self.pending_exports.borrow().is_empty()
        })
        .await?;
        if self
            .parent
            .upgrade()
            .is_none_or(|p| p.dialogs().n_items() != 1)
        {
            return Err("Background failures stacked multiple visible dialogs".into());
        }
        crate::write_feedback::Queue::smoke_ack(&first_error)?;
        let second_error = self
            .feedback
            .smoke_wait("Queued source B", "failed-output-queue-b.txt")
            .await?;
        if first_error == second_error {
            return Err("Acknowledging an error lost the next failure".into());
        }
        crate::write_feedback::Queue::smoke_ack(&second_error)?;
        self.feedback.smoke_clear().await?;
        if !self.allow_window_close() {
            return Err("Completed failed writes retained their close guard".into());
        }
        Ok(
            json!({"closedViewerErrorVisible":true,"replacementContextRetained":true,"replacementSelectionRetained":true,"duplicateCompletionIgnored":true,
            "multipleFailuresQueued":true,"singleVisibleAlert":true,"pendingWriteCloseGuard":true,"acknowledgedErrorsClear":true}),
        )
    }
}
