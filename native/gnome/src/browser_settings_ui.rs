//! Native browser policy editor with explicit saves and scoped conflict handling.
use crate::i18n_ui::UiText;
use crate::{
    browser_policy::NavigationPreferences,
    browser_settings::{Request, Snapshot},
    protocol::Command,
};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;
pub struct Panel {
    pub id: u64,
    pub dialog: adw::PreferencesDialog,
    sender: mpsc::Sender<Command>,
    snapshot: RefCell<Option<Snapshot>>,
    rows: [adw::SwitchRow; 5],
    restore: adw::SwitchRow,
    save: gtk::Button,
    reload: gtk::Button,
    status: gtk::Label,
    busy: Cell<bool>,
    serial: Cell<u64>,
    updating: Cell<bool>,
    confirmation: RefCell<Option<adw::AlertDialog>>,
    pub on_applied: RefCell<Option<Box<dyn Fn(NavigationPreferences, bool)>>>,
}
impl Panel {
    #[cfg(debug_assertions)]
    pub async fn smoke_ready(&self) -> Result<(), String> {
        let start = std::time::Instant::now();
        while self.busy.get()
            || self.snapshot.borrow().is_none()
            || !self.rows[0].is_mapped()
            || self.rows[0].height() == 0
        {
            if start.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "Browser settings did not become visible: {}",
                    self.status.text()
                ));
            }
            gtk::glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub fn smoke_values(&self) -> NavigationPreferences {
        self.values()
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_restore(&self, restore: bool) -> Result<(), String> {
        self.smoke_ready().await?;
        self.restore.set_active(restore);
        self.smoke_save(self.values()).await?;
        if self
            .snapshot
            .borrow()
            .as_ref()
            .is_none_or(|s| s.restore_tabs != restore)
        {
            return Err("Browser restoration setting did not save".into());
        }
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_save(&self, values: NavigationPreferences) -> Result<(), String> {
        self.smoke_ready().await?;
        for (row, value) in self.rows.iter().zip([
            values.enabled,
            values.allow_http,
            values.allow_private_network,
            values.allow_localhost,
            values.preview_javascript,
        ]) {
            row.set_active(value);
        }
        if !self.dirty() {
            return Ok(());
        }
        if self.may_close() || !self.save.is_sensitive() {
            return Err("Dirty browser settings are not guarded".into());
        }
        self.save.emit_clicked();
        let start = std::time::Instant::now();
        while self.busy.get() {
            if start.elapsed() > std::time::Duration::from_secs(20) {
                return Err("Browser settings save timed out".into());
            }
            gtk::glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.dirty()
            || self
                .snapshot
                .borrow()
                .as_ref()
                .is_none_or(|snapshot| snapshot.values != values)
        {
            return Err(format!(
                "Browser settings save failed: {}",
                self.status.text()
            ));
        }
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_guards(
        self: &Rc<Self>,
        path: &std::path::Path,
        parent_guard: impl Fn() -> bool,
    ) -> Result<serde_json::Value, String> {
        async fn wait(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err("Browser settings guard did not settle".into());
                }
                gtk::glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        fn respond(dialog: &adw::AlertDialog, label: &str) -> Result<(), String> {
            fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
                if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                    if button.label().as_deref() == Some(label) {
                        return Some(button.clone());
                    }
                }
                let mut child = widget.first_child();
                while let Some(widget) = child {
                    child = widget.next_sibling();
                    if let Some(button) = find(&widget, label) {
                        return Some(button);
                    }
                }
                None
            }
            find(dialog.upcast_ref(), label)
                .ok_or("Browser discard button was not found")?
                .emit_clicked();
            Ok(())
        }
        async fn read(path: &std::path::Path) -> Result<Vec<u8>, String> {
            let path = path.to_owned();
            gtk::gio::spawn_blocking(move || std::fs::read(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Fixture file worker stopped")?
        }
        self.smoke_ready().await?;
        let original = read(path).await?;
        let initial = self.values();
        self.rows[2].set_active(!initial.allow_private_network);
        if self.may_close() || parent_guard() {
            return Err("Browser settings edits did not guard close".into());
        }
        if read(path).await? != original {
            return Err("Browser setting toggle saved without Save".into());
        }
        self.dialog.close();
        wait(|| self.confirmation.borrow().is_some()).await?;
        let confirmation = self.confirmation.borrow().as_ref().unwrap().clone();
        respond(&confirmation, "Cancel")?;
        if !self.dirty() {
            return Err("Cancelling browser close lost edits".into());
        }
        self.reload.emit_clicked();
        wait(|| self.confirmation.borrow().is_some()).await?;
        let confirmation = self.confirmation.borrow().as_ref().unwrap().clone();
        respond(&confirmation, "Cancel")?;
        if !self.dirty() {
            return Err("Cancelling browser reload lost edits".into());
        }
        let path_owned = path.to_owned();
        let foreign_initial = initial.clone();
        let foreign = gtk::gio::spawn_blocking(move || -> Result<Vec<u8>, String> {
            let mut value: serde_json::Value =
                serde_json::from_slice(&original).map_err(|e| e.to_string())?;
            value["browser"]["allowHttp"] = serde_json::json!(!foreign_initial.allow_http);
            let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
            std::fs::write(path_owned, &bytes).map_err(|e| e.to_string())?;
            Ok(bytes)
        })
        .await
        .map_err(|_| "Fixture edit worker stopped")??;
        let edited = self.values();
        self.save.emit_clicked();
        wait(|| !self.busy.get()).await?;
        if self.values() != edited
            || !self.dirty()
            || !self.status.text().contains("changed in another client")
            || read(path).await? != foreign
        {
            return Err(
                "Browser save conflict did not retain local edits and foreign bytes".into(),
            );
        }
        self.reload.emit_clicked();
        wait(|| self.confirmation.borrow().is_some()).await?;
        let confirmation = self.confirmation.borrow().as_ref().unwrap().clone();
        respond(&confirmation, "Discard")?;
        wait(|| !self.busy.get()).await?;
        if self.dirty() || self.values().allow_http == initial.allow_http {
            return Err("Browser reload did not install foreign settings".into());
        }
        self.smoke_save(initial).await?;
        let values = self.values();
        self.event(
            self.serial.get().wrapping_sub(1),
            Ok(Snapshot {
                revision: "f".repeat(64),
                restore_tabs: self.restore.is_active(),
                values: NavigationPreferences {
                    enabled: !values.enabled,
                    ..values.clone()
                },
            }),
        );
        if self.values() != values {
            return Err("Stale browser reply changed controls".into());
        }
        Ok(
            serde_json::json!({"explicitSave":true,"closeCancel":true,"reloadCancel":true,"conflictRetention":true,"parentGuard":true,"staleReply":true}),
        )
    }
    pub fn new(sender: mpsc::Sender<Command>) -> Rc<Self> {
        let dialog = adw::PreferencesDialog::builder()
            .title("Browser navigation")
            .content_width(660)
            .content_height(640)
            .build()
            .ui_text("title", "Browser navigation");
        let page = adw::PreferencesPage::new();
        let group=adw::PreferencesGroup::builder().title("Page navigation").description("Changes take effect after Save. Disabling a permission closes tabs that use it; JavaScript changes reload open pages.").build().ui_text("title","Page navigation").ui_text("description","Changes take effect after Save. Disabling a permission closes tabs that use it; JavaScript changes reload open pages.");
        let rows = [
            adw::SwitchRow::builder()
                .title("Enable Browser")
                .subtitle("Turning off Browser ends its private browsing session")
                .build()
                .ui_text("title", "Enable Browser")
                .ui_text(
                    "subtitle",
                    "Turning off Browser ends its private browsing session",
                ),
            adw::SwitchRow::builder()
                .title("Allow HTTP navigation")
                .subtitle("Allow addresses that use unencrypted HTTP")
                .build()
                .ui_text("title", "Allow HTTP navigation")
                .ui_text("subtitle", "Allow addresses that use unencrypted HTTP"),
            adw::SwitchRow::builder()
                .title("Allow private-network addresses")
                .subtitle("Allow navigation to private IP addresses")
                .build()
                .ui_text("title", "Allow private-network addresses")
                .ui_text("subtitle", "Allow navigation to private IP addresses"),
            adw::SwitchRow::builder()
                .title("Allow localhost")
                .subtitle("Allow navigation to services on this computer")
                .build()
                .ui_text("title", "Allow localhost")
                .ui_text("subtitle", "Allow navigation to services on this computer"),
            adw::SwitchRow::builder()
                .title("Allow page JavaScript")
                .subtitle("Reloads open pages when changed")
                .build()
                .ui_text("title", "Allow page JavaScript")
                .ui_text("subtitle", "Reloads open pages when changed"),
        ];
        for row in &rows {
            group.add(row);
        }
        let restore = adw::SwitchRow::builder()
            .title("Restore browser tabs")
            .subtitle(
                "Save addresses and reopen them when Browser is opened. Cookies remain private.",
            )
            .build()
            .ui_text("title", "Restore browser tabs")
            .ui_text(
                "subtitle",
                "Save addresses and reopen them when Browser is opened. Cookies remain private.",
            );
        group.add(&restore);
        page.add(&group);
        let actions = adw::PreferencesGroup::new();
        let row = adw::ActionRow::new();
        let status = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .build();
        row.add_prefix(&status);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_valign(gtk::Align::Center);
        let reload = gtk::Button::with_label("Reload").ui_text("label", "Reload");
        let save = gtk::Button::with_label("Save").ui_text("label", "Save");
        save.add_css_class("suggested-action");
        buttons.append(&reload);
        buttons.append(&save);
        row.add_suffix(&buttons);
        actions.add(&row);
        page.add(&actions);
        dialog.add(&page);
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let panel = Rc::new(Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            dialog,
            sender,
            snapshot: RefCell::new(None),
            rows,
            restore,
            save,
            reload,
            status,
            busy: Cell::new(false),
            serial: Cell::new(0),
            updating: Cell::new(false),
            confirmation: RefCell::new(None),
            on_applied: RefCell::new(None),
        });
        for row in panel.rows.iter().chain([&panel.restore]) {
            let weak = Rc::downgrade(&panel);
            row.connect_active_notify(move |_| {
                if let Some(panel) = weak.upgrade() {
                    if !panel.updating.get() {
                        panel.changed();
                    }
                }
            });
        }
        let weak = Rc::downgrade(&panel);
        panel.save.connect_clicked(move |_| {
            let Some(panel) = weak.upgrade() else { return };
            if panel.busy.get() || !panel.dirty() {
                return;
            }
            let revision = panel.snapshot.borrow().as_ref().unwrap().revision.clone();
            panel.request(Request::Save {
                revision,
                values: panel.values(),
                restore_tabs: panel.restore.is_active(),
            });
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            let Some(panel) = weak.upgrade() else { return };
            if panel.busy.get() {
                return;
            }
            if panel.dirty() {
                let p = panel.clone();
                panel.confirm(
                    "Reload browser settings?",
                    "Unsaved browser settings will be discarded.",
                    move || p.request(Request::Read),
                );
            } else {
                panel.request(Request::Read);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            let Some(panel) = weak.upgrade() else { return };
            if panel.busy.get() {
                panel.notice("Wait for browser settings to finish");
                return;
            }
            if panel.dirty() {
                let p = panel.clone();
                panel.confirm(
                    "Discard browser settings?",
                    "Your unsaved browser settings will be discarded.",
                    move || p.dialog.force_close(),
                );
            } else {
                panel.dialog.force_close();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(panel) = weak.upgrade() {
                if let Some(dialog) = panel.confirmation.borrow_mut().take() {
                    dialog.force_close();
                }
            }
        });
        panel.request(Request::Read);
        panel
    }
    fn values(&self) -> NavigationPreferences {
        NavigationPreferences {
            enabled: self.rows[0].is_active(),
            allow_http: self.rows[1].is_active(),
            allow_private_network: self.rows[2].is_active(),
            allow_localhost: self.rows[3].is_active(),
            preview_javascript: self.rows[4].is_active(),
        }
    }
    fn dirty(&self) -> bool {
        self.snapshot.borrow().as_ref().is_some_and(|snapshot| {
            snapshot.values != self.values() || snapshot.restore_tabs != self.restore.is_active()
        })
    }
    pub fn may_close(&self) -> bool {
        !self.busy.get() && !self.dirty()
    }
    fn controls(&self) {
        for row in self.rows.iter().chain([&self.restore]) {
            row.set_sensitive(self.snapshot.borrow().is_some() && !self.busy.get());
        }
        self.save.set_sensitive(!self.busy.get() && self.dirty());
        self.reload.set_sensitive(!self.busy.get());
        self.dialog.set_can_close(self.may_close());
    }
    fn changed(&self) {
        self.status.set_text(if self.dirty() {
            "Unsaved changes"
        } else {
            "Saved"
        });
        self.controls();
    }
    fn notice(&self, message: &str) {
        self.status.set_text(message);
        self.dialog.add_toast(adw::Toast::new(message));
    }
    fn confirm(self: &Rc<Self>, title: &str, body: &str, action: impl Fn() + 'static) {
        if let Some(dialog) = self.confirmation.borrow().as_ref() {
            dialog.present(Some(&self.dialog));
            return;
        }
        let dialog = adw::AlertDialog::builder()
            .heading(title)
            .body(body)
            .build();
        dialog.add_responses(&[("cancel", "Cancel"), ("discard", "Discard")]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if let Some(panel) = weak.upgrade() {
                panel.confirmation.borrow_mut().take();
                if response == "discard" {
                    action();
                }
            }
        });
        dialog.present(Some(&self.dialog));
        self.confirmation.replace(Some(dialog));
    }
    fn request(&self, request: Request) {
        let saving = matches!(&request, Request::Save { .. });
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        self.busy.set(true);
        self.status
            .set_text(if saving { "Saving…" } else { "Loading…" });
        self.controls();
        if self
            .sender
            .try_send(Command::BrowserSettings {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.busy.set(false);
            self.notice("Browser settings queue is unavailable");
            self.controls();
        }
    }
    pub fn event(&self, serial: u64, result: Result<Snapshot, String>) {
        if serial != self.serial.get() || !self.busy.replace(false) {
            return;
        }
        match result {
            Ok(snapshot) => {
                self.updating.set(true);
                let values = &snapshot.values;
                for (row, value) in self.rows.iter().zip([
                    values.enabled,
                    values.allow_http,
                    values.allow_private_network,
                    values.allow_localhost,
                    values.preview_javascript,
                ]) {
                    row.set_active(value);
                }
                self.restore.set_active(snapshot.restore_tabs);
                self.updating.set(false);
                if let Some(callback) = self.on_applied.borrow().as_ref() {
                    callback(snapshot.values.clone(), snapshot.restore_tabs);
                }
                self.snapshot.replace(Some(snapshot));
                self.changed();
            }
            Err(error) => {
                self.notice(&error);
                self.controls();
            }
        }
    }
}
