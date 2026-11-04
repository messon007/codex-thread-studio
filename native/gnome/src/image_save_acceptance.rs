//! Original bytes, real Open/Save choices, late replies and admitted writes.
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
            return Err(format!("Image Save timed out: {step}"));
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
            .map_err(|_| "Image probe stopped")?
        {
            return Ok(());
        }
        if at.elapsed() > Duration::from_secs(10) {
            return Err(format!("Image write not admitted: {name}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
async fn release(root: &Path, name: &str) -> Result<(), String> {
    let path = root.join(format!("{name}-release"));
    gio::spawn_blocking(move || std::fs::write(path, b"release"))
        .await
        .map_err(|_| "Image probe stopped")?
        .map_err(|e| e.to_string())
}
impl Viewer {
    pub async fn smoke_embedded_save(
        self: &Rc<Self>,
        root: &Path,
        name: &str,
        size: (u32, u32),
    ) -> Result<(), String> {
        until("embedded source ready", || {
            self.save.is_sensitive() && self.picture.paintable().is_some()
        })
        .await?;
        if self.size.get() != size
            || !matches!(
                self.location.borrow().as_ref().map(|l| &l.source),
                Some(crate::images::Source::Embedded(_))
            )
        {
            return Err(format!("Embedded source identity/dimensions changed: actual {:?}, expected {size:?}, source {:?}", self.size.get(), self.location.borrow().as_ref().map(|l| &l.source)));
        }
        self.plus.emit_clicked();
        self.copy.emit_clicked();
        self.save.emit_clicked();
        until("embedded original write finished", || {
            self.save_picker.borrow().is_none() && self.exports.borrow().is_empty()
        })
        .await?;
        let path = root.join(name);
        let present = gio::spawn_blocking(move || path.is_file())
            .await
            .map_err(|_| "Image export probe stopped")?;
        if !present {
            return Err("Embedded original was not written through the native chooser".into());
        }
        Ok(())
    }
    pub async fn smoke_file_original_save(
        self: &Rc<Self>,
        root: &Path,
        name: &str,
        source: &Path,
        size: (u32, u32),
    ) -> Result<(), String> {
        until("file source ready", || {
            self.save.is_sensitive() && self.picture.paintable().is_some()
        })
        .await?;
        if self.size.get()!=size || !self.location.borrow().as_ref().is_some_and(|l|matches!(&l.source,crate::images::Source::File{path,..} if Path::new(path)==source)) {return Err("File source identity or dimensions changed".into());}
        self.plus.emit_clicked();
        self.copy.emit_clicked();
        self.save.emit_clicked();
        until("file original write finished", || {
            self.save_picker.borrow().is_none() && self.exports.borrow().is_empty()
        })
        .await?;
        let path = root.join(name);
        if !gio::spawn_blocking(move || path.is_file())
            .await
            .map_err(|_| "File export probe stopped")?
        {
            return Err("File original was not written through the native chooser".into());
        }
        Ok(())
    }
    async fn save_loaded(self: &Rc<Self>, root: &Path, name: &str) -> Result<(), String> {
        self.open(
            &root.join("project").to_string_lossy(),
            &root.join("project").join(name).to_string_lossy(),
        );
        until("loaded source", || {
            self.save.is_sensitive() && self.picture.paintable().is_some()
        })
        .await
    }
    pub async fn smoke_save(
        self: &Rc<Self>,
        root: &Path,
        close_window: impl Fn(),
    ) -> Result<Value, String> {
        self.save_loaded(root, "sample.png").await?;
        self.plus.emit_clicked();
        let zoom = self.zoom.get();
        let size = self.size.get();
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 1).await?;
        until("Save cancelled", || {
            self.save_picker.borrow().is_none() && self.save.is_sensitive()
        })
        .await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 2).await?;
        until("nonlocal choice rejected", || {
            self.status.text() == "Choose a local destination for the original image"
                && self.save.is_sensitive()
        })
        .await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 3).await?;
        until("chooser failure visible", || {
            self.status
                .text()
                .starts_with("Unable to open image Save chooser:")
                && self.save.is_sensitive()
        })
        .await?;
        if self.zoom.get() != zoom || self.size.get() != size || self.picture.paintable().is_none()
        {
            return Err("Chooser cancellation changed image layout".into());
        }
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 4).await?;
        until("PNG original written", || {
            self.exports.borrow().is_empty() && self.status.text() == "Original image saved"
        })
        .await?;
        let parent = self.parent.upgrade().ok_or("Image parent missing")?;
        crate::window_capture::capture(&parent, &root.join("image-save-original.png")).await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 5).await?;
        self.choose_save();
        self.choose_save();
        glib::timeout_future(Duration::from_millis(100)).await;
        if crate::smoke_portal::read(root).await?["calls"]
            .as_array()
            .map(Vec::len)
            != Some(5)
        {
            return Err("Image Save opened duplicate choosers".into());
        }
        self.close();
        crate::smoke_portal::wait(root, "closed", 1).await?;
        glib::timeout_future(Duration::from_millis(120)).await;
        if self.location.borrow().is_some() || self.save_picker.borrow().is_some() {
            return Err("Closed image accepted its late Save reply".into());
        }
        self.save_loaded(root, "sample.png").await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 6).await?;
        self.save_loaded(root, "vector.svg").await?;
        crate::smoke_portal::wait(root, "closed", 2).await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 7).await?;
        until("SVG original written", || {
            self.exports.borrow().is_empty() && self.status.text() == "Original image saved"
        })
        .await?;

        self.save_loaded(root, "sample.png").await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 8).await?;
        started(root, "admitted-original.png").await?;
        self.save_loaded(root, "vector.svg").await?;
        let current = self.status.text();
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 9).await?;
        close_window();
        crate::smoke_portal::wait(root, "closed", 3).await?;
        if self.exports.borrow().is_empty()
            || !parent.is_mapped()
            || self.save_picker.borrow().is_some()
        {
            return Err(
                "Window close discarded a confirmed save or kept its unconfirmed choice".into(),
            );
        }
        release(root, "admitted-original.png").await?;
        until("previous image write completed", || {
            self.exports.borrow().is_empty()
        })
        .await?;
        if self.status.text() != current || self.feedback.smoke_waiting("image") {
            return Err(
                "Old save completion changed the replacement image or kept its close notice".into(),
            );
        }

        let path = root.join("inline.gif");
        let image=gio::spawn_blocking(move||{
            use base64::Engine;
            let bytes=std::fs::read(path).map_err(|e|e.to_string())?;
            crate::conversation_images::Image::from_part(&json!({"type":"image","url":format!("data:image/gif;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}))
                .ok_or("Inline image missing".to_string())
        }).await.map_err(|_|"Inline worker stopped")??;
        let crate::conversation_images::Source::Inline { token } = image.source else {
            return Err("Inline GIF did not receive a private reference".into());
        };
        self.open_inline(&token);
        until("inline GIF loaded", || self.save.is_sensitive()).await?;
        if self.identity.borrow().as_ref().map(|i| i.extension()) != Some("gif") {
            return Err("Original GIF lost its format metadata".into());
        }
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 10).await?;
        started(root, "inline-original.gif").await?;
        self.close();
        let old = token.clone();
        gio::spawn_blocking(move || crate::conversation_images::smoke_evict(&old))
            .await
            .map_err(|_| "Eviction worker stopped")??;
        let old = token;
        let absent = gio::spawn_blocking(move || crate::conversation_images::bytes(&old).is_err())
            .await
            .map_err(|_| "Eviction worker stopped")?;
        if !absent {
            return Err("Inline image was not evicted before admitted write completed".into());
        }
        release(root, "inline-original.gif").await?;
        until("inline write after close and eviction", || {
            self.exports.borrow().is_empty()
        })
        .await?;

        self.save_loaded(root, "sample.png").await?;
        let path = root.join("project/sample.png");
        let changed = root.join("changed.png");
        gio::spawn_blocking(move || std::fs::copy(changed, path))
            .await
            .map_err(|_| "Mutation worker stopped")?
            .map_err(|e| e.to_string())?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 11).await?;
        let alert = self
            .feedback
            .smoke_wait("sample.png", "changed-source.png")
            .await?;
        if !self
            .status
            .text()
            .contains("Image changed since it was opened")
        {
            return Err("Source change did not stop the image save".into());
        }
        self.feedback
            .smoke_capture(&root.join("image-save-changed.png"))
            .await?;
        crate::write_feedback::Queue::smoke_ack(&alert)?;
        self.feedback.smoke_clear().await?;
        let path = root.join("project/sample.png");
        let original = root.join("expected.png");
        gio::spawn_blocking(move || std::fs::copy(original, path))
            .await
            .map_err(|_| "Mutation worker stopped")?
            .map_err(|e| e.to_string())?;
        self.save_loaded(root, "sample.png").await?;
        self.save.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 12).await?;
        started(root, "failed-original.png").await?;
        self.close();
        release(root, "failed-original.png").await?;
        let alert = self
            .feedback
            .smoke_wait("sample.png", "failed-original.png")
            .await?;
        self.feedback
            .smoke_capture(&root.join("image-save-failed.png"))
            .await?;
        crate::write_feedback::Queue::smoke_ack(&alert)?;
        self.feedback.smoke_clear().await?;

        self.save_loaded(root, "sample.png").await?;
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 13).await?;
        until("Open cancelled", || {
            self.open_picker.borrow().is_none() && self.open.is_sensitive()
        })
        .await?;
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 14).await?;
        self.close();
        crate::smoke_portal::wait(root, "closed", 4).await?;
        glib::timeout_future(Duration::from_millis(120)).await;
        if self.location.borrow().is_some() {
            return Err("Late Open resurrected a closed image".into());
        }
        self.save_loaded(root, "sample.png").await?;
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 15).await?;
        self.save_loaded(root, "vector.svg").await?;
        crate::smoke_portal::wait(root, "closed", 5).await?;
        glib::timeout_future(Duration::from_millis(120)).await;
        if self
            .location
            .borrow()
            .as_ref()
            .is_none_or(|l| l.title != "vector.svg")
        {
            return Err("Late Open replaced the current image".into());
        }
        self.open.emit_clicked();
        crate::smoke_portal::wait(root, "calls", 16).await?;
        until("chosen image loaded", || {
            self.location
                .borrow()
                .as_ref()
                .is_some_and(|l| l.title == "sample.png")
                && self.save.is_sensitive()
        })
        .await?;
        if !self.exports.borrow().is_empty()
            || self.save_picker.borrow().is_some()
            || self.open_picker.borrow().is_some()
        {
            return Err("Completed image workflow retained a write/chooser guard".into());
        }
        self.close();
        Ok(
            json!({"originalPngSvgGif":true,"cancelKeepsView":true,"nonlocalRejected":true,"chooserErrorsVisible":true,"duplicatesCoalesced":true,
            "closeAndReplacementCancel":true,"lateChoicesIgnored":true,"parentCloseCancelsOnlyChoice":true,"admittedSaveCloseGuard":true,
            "replacementStateRetained":true,"inlineLeaseAfterEviction":true,"changedSourceRejected":true,"diskFailureAfterCloseVisible":true,"openChoiceLifecycle":true}),
        )
    }
}
