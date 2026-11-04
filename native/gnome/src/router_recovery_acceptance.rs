use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
async fn wait(label: &str, ready: impl Fn() -> bool) -> Result<(), String> {
    let at = Instant::now();
    while !ready() {
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!("Controller recovery timed out: {label}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn flag(directory: &Path, name: &str, on: bool) -> Result<(), String> {
    let path = directory.join(name);
    gtk::gio::spawn_blocking(move || {
        if on {
            std::fs::write(path, b"private-fixture")
        } else {
            std::fs::remove_file(path)
        }
    })
    .await
    .map_err(|_| "Fixture file worker stopped")?
    .map_err(|e| e.to_string())
}
impl Recovery {
    pub async fn smoke(
        self: &Rc<Self>,
        directory: &Path,
        capture: impl Fn(&adw::Dialog, &str),
    ) -> Result<serde_json::Value, String> {
        wait("journal", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        if self.snapshot.borrow().as_ref().unwrap().state != "uncertain"
            || self.adopt.is_sensitive()
        {
            return Err("Recovery did not require explicit inspection".into());
        }
        wait("session choices", || {
            self.keys.borrow().iter().any(|id| id == "router")
        })
        .await?;
        let index = self
            .keys
            .borrow()
            .iter()
            .position(|id| id == "router")
            .unwrap();
        self.selector.set_selected(index as u32);
        if self.entry.text() != "router" || self.adopt.is_sensitive() {
            return Err("Picking a controller adopted it before inspection".into());
        }
        flag(directory, "force-recovery-busy", true).await?;
        self.inspect.emit_clicked();
        wait("busy metadata rejection", || !self.busy.get()).await?;
        if !self.status.text().contains("idle") || self.adopt.is_sensitive() {
            return Err("Recovery accepted busy controller metadata".into());
        }
        flag(directory, "force-recovery-busy", false).await?;
        self.inspect.emit_clicked();
        wait("inspection", || {
            !self.busy.get() && self.adopt.is_sensitive()
        })
        .await?;
        capture(&self.dialog, "controller-recovery-inspected.png");
        let reviewed = self.snapshot.borrow().clone().unwrap();
        self.entry.set_text("another-session");
        self.event(Ok(Reply::Recovery(reviewed)));
        if self.adopt.is_sensitive() {
            return Err("A stale inspection enabled a changed session ID".into());
        }
        self.entry.set_text("router");
        flag(directory, "hold-recovery-read", true).await?;
        self.inspect.emit_clicked();
        self.dialog.force_close();
        let owner = self.owner.upgrade().ok_or("Recovery settings closed")?;
        owner.open_recovery();
        let current = owner
            .recovery
            .borrow()
            .clone()
            .ok_or("Recovery did not reopen")?;
        if current.id == self.id {
            return Err("Reopened recovery reused its old request identity".into());
        }
        flag(directory, "hold-recovery-read", false).await?;
        wait("reopened journal", || {
            !current.busy.get() && current.snapshot.borrow().is_some()
        })
        .await?;
        glib::timeout_future(Duration::from_millis(450)).await;
        if current.adopt.is_sensitive() || current.reviewed.borrow().is_some() {
            return Err("A closed recovery dialog's inspection reached its replacement".into());
        }
        current.entry.set_text("router");
        current.inspect.emit_clicked();
        wait("reinspection", || {
            !current.busy.get() && current.adopt.is_sensitive()
        })
        .await?;
        let snapshot_path = directory.join("config/codex-thread-studio/settings.json");
        let before = gtk::gio::spawn_blocking(move || std::fs::read(snapshot_path))
            .await
            .map_err(|_| "Fixture read worker stopped")?
            .map_err(|e| e.to_string())?;
        let raw: serde_json::Value = serde_json::from_slice(&before).map_err(|e| e.to_string())?;
        if raw.pointer("/router/controllers/codex").is_some() {
            return Err("Inspection registered the controller before adoption".into());
        }
        flag(directory, "hold-recovery-read", true).await?;
        current.adopt.emit_clicked();
        if !current.mutating.get()
            || current.dialog.can_close()
            || current.entry.is_sensitive()
            || owner.save.is_sensitive()
        {
            return Err("Adoption did not protect its in-flight identity".into());
        }
        current.dialog.close();
        if !current.dialog.is_visible() {
            return Err("Adoption closed before its result was known".into());
        }
        flag(directory, "hold-recovery-read", false).await?;
        wait("registration", || {
            owner.recovery.borrow().is_none() && !owner.saving()
        })
        .await?;
        if owner
            .snapshot
            .borrow()
            .as_ref()
            .unwrap()
            .controllers
            .get("codex")
            .map(String::as_str)
            != Some("router")
        {
            return Err("The adopted identity was not registered".into());
        }
        if owner.recover.is_visible() {
            return Err("Configured controller still offers uncertain recovery".into());
        }
        Ok(
            serde_json::json!({"ok":true,"inspectBeforeAdopt":true,"actualPicker":true,"busyMetadataRejected":true,"staleIdRejected":true,"staleDialogRejected":true,"adoptionCloseGuard":true,"settingsDraftRetention":true,"explicitAdoption":true}),
        )
    }
}
