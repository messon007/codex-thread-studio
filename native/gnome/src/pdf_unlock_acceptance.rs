//! Actual PDF password prompts and callbacks, never stored fixture passwords.
use super::*;
use crate::acceptance_wait::until;
impl Pdf {
    async fn smoke_unlock_prompt(&self) -> Result<adw::AlertDialog, String> {
        until("PDF unlock prompt", || {
            self.password_prompt
                .borrow()
                .as_ref()
                .cloned()
                .is_some_and(|d| d.is_mapped())
                && !self.state.borrow().busy
        })
        .await
        .map_err(|e| format!("{e}: {}", self.status.text()))?;
        self.password_prompt
            .borrow()
            .as_ref()
            .cloned()
            .ok_or("PDF prompt disappeared".into())
    }
    pub(super) fn smoke_prompt_entry(
        dialog: &adw::AlertDialog,
    ) -> Result<gtk::PasswordEntry, String> {
        dialog
            .extra_child()
            .and_downcast::<gtk::PasswordEntry>()
            .ok_or("PDF password entry missing".into())
    }
    fn smoke_prompt_respond(dialog: &adw::AlertDialog, answer: &str) -> Result<(), String> {
        fn button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
            if let Some(button) = widget
                .downcast_ref::<gtk::Button>()
                .filter(|b| b.label().as_deref() == Some(label))
            {
                return Some(button.clone());
            }
            let mut child = widget.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if let Some(b) = button(&w, label) {
                    return Some(b);
                }
            }
            None
        }
        let label = dialog.response_label(answer);
        button(dialog.upcast_ref(), &label)
            .ok_or("PDF unlock response button missing")?
            .emit_clicked();
        Ok(())
    }
    async fn smoke_password_response(
        &self,
        dialog: &adw::AlertDialog,
        password: &str,
        answer: &str,
    ) -> Result<(), String> {
        let entry = Self::smoke_prompt_entry(dialog)?;
        entry.set_text(password);
        Self::smoke_prompt_respond(dialog, answer)?;
        until("PDF unlock response settled", || entry.text().is_empty()).await?;
        Ok(())
    }
    async fn smoke_wrong_then_unlock(
        self: &Rc<Self>,
        dialog: adw::AlertDialog,
    ) -> Result<(), String> {
        self.smoke_password_response(&dialog, "private-wrong-password", "open")
            .await?;
        let retry = self
            .smoke_unlock_prompt()
            .await
            .map_err(|e| format!("Wrong-password retry: {e}"))?;
        if retry == dialog {
            return Err("Wrong PDF password did not produce a fresh prompt".into());
        }
        let serial = self.state.borrow().serial;
        dialog.emit_by_name::<()>("response", &[&"open"]);
        if self.state.borrow().serial != serial
            || self.password_prompt.borrow().as_ref().cloned().as_ref() != Some(&retry)
        {
            return Err("A retired PDF prompt dispatched into its replacement".into());
        }
        self.smoke_password_response(&retry, "open-sesame", "open")
            .await
    }
    pub(super) async fn smoke_replace_password_prompt(
        self: &Rc<Self>,
        root: &std::path::Path,
    ) -> Result<(), String> {
        let dialog = self.smoke_unlock_prompt().await?;
        let entry = Self::smoke_prompt_entry(&dialog)?;
        entry.set_text("private-replaced-password");
        self.open(
            &root.to_string_lossy(),
            &root.join("sample.pdf").to_string_lossy(),
            0,
        );
        let serial = self.state.borrow().serial;
        dialog.emit_by_name::<()>("response", &[&"open"]);
        if !entry.text().is_empty()
            || self.password_prompt.borrow().is_some()
            || self.state.borrow().serial != serial
        {
            return Err("A replaced PDF retained an unlock prompt or its request".into());
        }
        until("replacement PDF after cancelled unlock", || {
            self.state.borrow().pages == 3 && !self.state.borrow().busy
        })
        .await?;
        if dialog.is_mapped() {
            return Err("Old unlock dialog stayed mapped over the replacement PDF".into());
        }
        self.open(
            &root.to_string_lossy(),
            &root.join("locked.pdf").to_string_lossy(),
            0,
        );
        Ok(())
    }
    pub(crate) async fn smoke_initial_unlock(self: &Rc<Self>) -> Result<(), String> {
        let dialog = self.smoke_unlock_prompt().await?;
        self.smoke_password_language(&dialog).await?;
        self.smoke_wrong_then_unlock(dialog).await
    }
    pub(crate) async fn smoke_recovery_unlock(self: &Rc<Self>) -> Result<Value, String> {
        let dialog = self.smoke_unlock_prompt().await?;
        {
            let s = self.state.borrow();
            if !s.recovering
                || s.id.is_some()
                || s.page != 1
                || s.hash.is_empty()
                || self.picture.paintable().is_none()
            {
                return Err("Unlock recovery discarded the verified PDF view".into());
            }
        }
        if let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR") {
            crate::window_capture::capture_dialog(
                &self.parent.upgrade().ok_or("PDF unlock parent missing")?,
                &dialog,
                &std::path::PathBuf::from(root).join("pdf-recovery-unlock.png"),
            )
            .await?;
        }
        let entry = Self::smoke_prompt_entry(&dialog)?;
        entry.set_text("private-cancelled-password");
        let serial = self.state.borrow().serial;
        self.gateway_restored();
        if self.state.borrow().serial != serial
            || entry.text().as_str() != "private-cancelled-password"
        {
            return Err("Duplicate recovery replaced an active password prompt".into());
        }
        Self::smoke_prompt_respond(&dialog, "cancel")?;
        until("PDF unlock cancel settled", || {
            entry.text().is_empty() && self.password_prompt.borrow().is_none()
        })
        .await?;
        if !entry.text().is_empty()
            || self.password_prompt.borrow().is_some()
            || self.state.borrow().id.is_some()
            || self.next.is_sensitive()
            || self.picture.paintable().is_none()
        {
            return Err("Cancel did not retain a disabled PDF snapshot or clear its prompt".into());
        }
        dialog.emit_by_name::<()>("response", &[&"open"]);
        if self.state.borrow().serial != serial {
            return Err("Cancelled password prompt sent a later request".into());
        }
        self.gateway_restored();
        let retry = self.smoke_unlock_prompt().await?;
        self.smoke_wrong_then_unlock(retry).await?;
        Ok(
            json!({"reentryRequired":true,"cancelRetainsSnapshot":true,"wrongPasswordRetried":true,"retiredPromptsRejected":true,"duplicateRecoveryGuarded":true,"entriesCleared":true}),
        )
    }
}
