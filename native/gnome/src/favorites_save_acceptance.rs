//! Private real-portal coverage for the library's pending Save choice.
use super::*;
use std::{path::Path, time::Duration};

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!("Favorites Save acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}

fn notice(widget: &gtk::Widget, text: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|label| label.is_mapped() && label.height() > 0 && label.text().contains(text))
    {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if notice(&widget, text) {
            return true;
        }
    }
    false
}

impl Favorites {
    pub async fn smoke_save_portal(
        self: &Rc<Self>,
        root: &Path,
        close_window: impl Fn(),
        capture: impl Fn(&adw::Dialog),
    ) -> Result<serde_json::Value, String> {
        let original_title = "Saved portal favorite 中文😀";
        self.draft(Favorite {
            id: glib::uuid_string_random().to_string(),
            scope: "message".into(),
            backend: "codex".into(),
            thread_id: "target".into(),
            thread_title: "Private Save fixture".into(),
            project_path: root.join("project").to_string_lossy().into_owned(),
            turn_id: "private-turn".into(),
            item_id: "private-item".into(),
            title: original_title.into(),
            question: "Private question 中文😀".into(),
            content: "# Persisted favorite\r\nLiteral 中文😀 and trailing space \r\n".into(),
            note: "Persisted note 中文😀".into(),
            tags: vec!["portal".into()],
            created_at: "2026-10-25T00:00:00Z".into(),
        });
        until("prepared draft", || {
            self.new.get() && self.current.borrow().is_some()
        })
        .await?;
        self.save.emit_clicked();
        until("explicitly saved favorite", || {
            !self.new.get() && !self.dirty.get() && self.write_serial.get() == 0
        })
        .await?;
        until("mapped library", || self.dialog.is_mapped()).await?;
        self.title.set_text("UNSAVED portal title 中文😀");
        self.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 9).await?;
        until("user cancellation", || {
            self.export_picker.borrow().is_none() && self.export.is_sensitive()
        })
        .await?;
        self.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 10).await?;
        until("visible chooser error toast", || {
            self.export_picker.borrow().is_none()
                && self.export.is_sensitive()
                && notice(self.toast.upcast_ref(), "Unable to open export chooser:")
        })
        .await?;
        self.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 11).await?;
        self.export();
        self.export();
        glib::timeout_future(Duration::from_millis(120)).await;
        if crate::smoke_portal::read(root).await?["calls"]
            .as_array()
            .map(Vec::len)
            != Some(11)
        {
            return Err("Favorites opened duplicate Save dialogs".into());
        }
        close_window();
        if self.export_picker.borrow().is_some()
            || !self.dirty.get()
            || self.title.text() != "UNSAVED portal title 中文😀"
            || !self.export.is_sensitive()
        {
            return Err("Window close failed to cancel only the favorite Save choice".into());
        }
        // Reopen before yielding to the old choice's cancelled callback.
        self.export.emit_clicked();
        crate::smoke_portal::wait(root, "closed", 4).await?;
        crate::smoke_portal::wait(root, "calls", 12).await?;
        until("completed export and visible confirmation", || {
            self.export_picker.borrow().is_none()
                && self.export_serial.get() == 0
                && self.export.is_sensitive()
                && notice(self.toast.upcast_ref(), "Favorites exported")
        })
        .await?;
        let path = root.join("favorites saved 中文 #.md");
        let exported = gio::spawn_blocking(move || std::fs::read_to_string(path))
            .await
            .map_err(|_| "Favorites export probe worker stopped")?
            .map_err(|e| e.to_string())?;
        if !exported.contains(original_title)
            || !exported.contains("Persisted note 中文😀")
            || !exported.contains("Literal 中文😀 and trailing space")
            || exported.contains("UNSAVED portal title")
            || !self.dirty.get()
        {
            return Err("Export changed the draft or omitted persisted favorite data".into());
        }
        glib::timeout_future(Duration::from_millis(350)).await;
        capture(&self.dialog);
        self.export.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 13).await?;
        self.dialog.close();
        let parent = self
            .dialog
            .root()
            .and_downcast::<adw::ApplicationWindow>()
            .ok_or("Favorites lost its parent before discard confirmation")?;
        let alert = Self::alert(&parent).await?;
        Self::respond(&alert, "Discard")?;
        crate::smoke_portal::wait(root, "closed", 5).await?;
        until("closed library and dialog stack", || {
            !self.dialog.is_mapped() && parent.visible_dialog().is_none()
        })
        .await
        .map_err(|error| {
            format!(
                "{error}; mapped={}, dirty={}, prompt={}, canClose={}, remaining={:?}",
                self.dialog.is_mapped(),
                self.dirty.get(),
                self.prompt.get(),
                self.dialog.can_close(),
                parent.visible_dialog().map(|d| d.title().to_string())
            )
        })?;
        glib::timeout_future(Duration::from_millis(150)).await;
        if self.export_picker.borrow().is_some() || self.export_serial.get() != 0 {
            return Err(
                "Closed Favorites retained a Save choice or accepted its late reply".into(),
            );
        }
        Ok(serde_json::json!({
            "cancel":true,"chooserErrorVisible":true,"duplicateSaveCoalesced":true,
            "windowCloseDraftRetention":true,"sameViewerReopen":true,
            "explicitDiscard":true,"closeCancellation":true,"persistedLibraryExport":true
        }))
    }
}
