//! Actual GtkFileDialog transport and pending-request ownership, private files only.
use super::*;
use serde_json::{json, Value};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!("Reader chooser timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
impl Reader {
    pub async fn smoke_file_picker(
        self: &Rc<Self>,
        root: &Path,
        remap: impl Fn(),
    ) -> Result<Value, String> {
        // Portal cancellation, successful selection, error, then unsupported URI.
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 1).await?;
        until("cancel released chooser", || !self.picker.is_pending()).await?;
        if self.location().is_some() || !self.open.is_sensitive() {
            return Err("Cancel changed document or left Open disabled".into());
        }
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 2).await?;
        let selected = root.join("project/selected 中文 #.md");
        until("portal-selected document", || {
            self.is_ready() && self.location().is_some_and(|(_, p)| p == selected)
        })
        .await?;
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 3).await?;
        until("portal error displayed", || {
            !self.picker.is_pending()
                && self
                    .status
                    .text()
                    .starts_with("Unable to open file chooser:")
        })
        .await?;
        if !self.open.is_sensitive() {
            return Err("Chooser error disabled Open".into());
        }
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 4).await?;
        until("nonlocal selection rejected", || {
            !self.picker.is_pending()
                && self
                    .status
                    .text()
                    .starts_with("Choose a local Markdown file")
        })
        .await?;
        if !self.location().is_some_and(|(_, p)| p == selected) {
            return Err("Failed chooser replaced current document".into());
        }
        // While held, repeated Open is coalesced; another explicit document wins.
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 5).await?;
        self.choose();
        self.choose();
        glib::timeout_future(Duration::from_millis(150)).await;
        if crate::smoke_portal::read(root).await?["calls"]
            .as_array()
            .map(Vec::len)
            != Some(5)
            || self.open.is_sensitive()
        {
            return Err("Repeated Open created another chooser".into());
        }
        let alternate = root.join("project/alternate.md");
        self.open_path(&alternate);
        crate::smoke_portal::wait(root, "closed", 1).await?;
        until("replacement document ready", || {
            self.is_ready() && self.location().is_some_and(|(_, p)| p == alternate)
        })
        .await?;
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 6).await?;
        if !self.picker.is_pending() || self.open.is_sensitive() {
            return Err("Old cancellation cleared a newer chooser".into());
        }
        self.widget.set_visible(false);
        if self.picker.is_pending() {
            return Err("Hidden reader retained its chooser".into());
        }
        self.widget.set_visible(true);
        remap();
        // Reopen before yielding so the old cancellation callback races with
        // a newer pending chooser owned by this same Reader.
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "closed", 2).await?;
        crate::smoke_portal::wait(root, "calls", 7).await?;
        if !self.picker.is_pending() || self.open.is_sensitive() {
            return Err("Old cancellation cleared the reopened chooser".into());
        }
        if !self.allow_window_close() {
            return Err("Clean reader rejected window close".into());
        }
        crate::smoke_portal::wait(root, "closed", 3).await?;
        if self.picker.is_pending() || !self.open.is_sensitive() {
            return Err("Window-close cancellation retained chooser state".into());
        }
        until("reader visible after chooser operations", || {
            self.widget.is_mapped() && self.is_ready()
        })
        .await?;
        Ok(
            json!({"cancel":true,"selectedLocalUnicodePath":true,"errorsVisible":true,"nonlocalRejected":true,"duplicateOpenCoalesced":true,"explicitReplacementWins":true,"newPickerRetained":true,"unmapCancellation":true,"windowCloseCancellation":true}),
        )
    }
}
