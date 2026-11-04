//! An admitted local GFile write survives a simulated gateway-loss callback.
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!("Favorites write timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
impl Favorites {
    pub(super) async fn smoke_hold_write(&self, file: &gio::File) -> Result<(), String> {
        if std::env::var_os("STUDIO_GNOME_SMOKE_WRITE_FAILURES").is_none() {
            return Ok(());
        }
        let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from)
        else {
            return Ok(());
        };
        if std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
            != Some(root.join("fake-codex.py"))
            || std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
                != Some(root.join("config"))
        {
            return Ok(());
        }
        let Some(name) = ["failed-favorites", "interrupted-favorites"]
            .into_iter()
            .find(|name| file.path() == Some(root.join(format!("{name}.md"))))
        else {
            return Ok(());
        };
        let path = root.join(format!("{name}-started"));
        gio::spawn_blocking(move || std::fs::write(path, b"started"))
            .await
            .map_err(|_| "Write hold worker stopped")?
            .map_err(|e| e.to_string())?;
        let at = Instant::now();
        loop {
            let path = root.join(format!("{name}-release"));
            if gio::spawn_blocking(move || path.is_file())
                .await
                .map_err(|_| "Write hold worker stopped")?
            {
                return Ok(());
            }
            if at.elapsed() > Duration::from_secs(15) {
                return Err("Private Favorites write fixture did not release its write".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
    }
    pub async fn smoke_write_failures(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        let title = "Write lifecycle favorite 中文😀";
        self.draft(Favorite {
            id: glib::uuid_string_random().to_string(),
            scope: "message".into(),
            backend: "codex".into(),
            thread_id: "target".into(),
            thread_title: "Write failure fixture".into(),
            project_path: root.join("project").to_string_lossy().into_owned(),
            turn_id: "write-turn".into(),
            item_id: "write-item".into(),
            title: title.into(),
            question: "Write question".into(),
            content: "Persisted content 中文😀\r\n".into(),
            note: "Persisted lifecycle note".into(),
            tags: vec![],
            created_at: "2026-10-25T00:00:00Z".into(),
        });
        until("draft", || {
            self.new.get() && self.current.borrow().is_some()
        })
        .await?;
        self.save.emit_clicked();
        until("persisted favorite", || {
            !self.new.get() && !self.dirty.get() && self.write_serial.get() == 0
        })
        .await?;
        until("mapped library", || self.dialog.is_mapped()).await?;
        self.title.set_text("UNSAVED failure draft 中文😀");
        for (call, name, failure) in [
            (5, "failed-favorites", true),
            (6, "interrupted-favorites", false),
        ] {
            self.export.emit_clicked();
            crate::smoke_portal::wait(root, "calls", call).await?;
            until("local write admitted", || {
                self.export_writing.get() && self.export_serial.get() != 0
            })
            .await?;
            let serial = self.export_serial.get();
            self.gateway_failed("Simulated connection loss during admitted local write");
            self.event(serial, Err("Duplicate late gateway response".into()));
            self.dialog.close();
            self.export();
            glib::timeout_future(Duration::from_millis(80)).await;
            if self.export_serial.get() != serial
                || !self.export_writing.get()
                || self.export.is_sensitive()
                || !self.dialog.is_mapped()
                || self.feedback.blocks_close()
            {
                return Err(
                    "Gateway loss discarded a local write's identity or close guard".into(),
                );
            }
            if crate::smoke_portal::read(root).await?["calls"]
                .as_array()
                .map(Vec::len)
                != Some(call)
            {
                return Err("Local write admitted a concurrent export".into());
            }
            let path = root.join(format!("{name}-release"));
            gio::spawn_blocking(move || std::fs::write(path, b"release"))
                .await
                .map_err(|_| "Release worker stopped")?
                .map_err(|e| e.to_string())?;
            if failure {
                let alert = self
                    .feedback
                    .smoke_wait("Favorites", "failed-favorites.md")
                    .await?;
                self.feedback
                    .smoke_capture(&root.join("write-error-favorites.png"))
                    .await?;
                crate::write_feedback::Queue::smoke_ack(&alert)?;
                self.feedback.smoke_clear().await?;
            }
            until("write completed", || {
                self.export_serial.get() == 0
                    && !self.export_writing.get()
                    && self.export.is_sensitive()
            })
            .await?;
            if !self.dirty.get()
                || self.title.text() != "UNSAVED failure draft 中文😀"
                || self
                    .current
                    .borrow()
                    .as_ref()
                    .is_none_or(|f| f.title != title)
            {
                return Err("Export failure or connection loss changed unsaved metadata".into());
            }
        }
        let path = root.join("interrupted-favorites.md");
        let text = gio::spawn_blocking(move || std::fs::read_to_string(path))
            .await
            .map_err(|_| "Result worker stopped")?
            .map_err(|e| e.to_string())?;
        if !text.contains(title)
            || !text.contains("Persisted lifecycle note")
            || text.contains("UNSAVED failure draft")
        {
            return Err("Resumed local write lost its immutable persisted data".into());
        }
        self.dialog.close();
        let parent = self
            .dialog
            .root()
            .and_downcast::<adw::ApplicationWindow>()
            .ok_or("Favorites parent missing")?;
        let alert = Self::alert(&parent).await?;
        Self::respond(&alert, "Discard")?;
        until("discarded metadata and closed library", || {
            !self.dialog.is_mapped() && parent.visible_dialog().is_none()
        })
        .await?;
        Ok(
            serde_json::json!({"localWriteSurvivesGatewayLoss":true,"lateGatewayReplyIgnored":true,"localWriteCloseGuard":true,"duplicateExportRejected":true,
            "failureDestinationVisible":true,"writeGuardReleased":true,"unsavedMetadataRetained":true,"persistedSnapshotExported":true}),
        )
    }
}
