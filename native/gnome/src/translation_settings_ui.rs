//! Explicit native editor for the shared translation profile; no model is run here.
use crate::{
    i18n_ui::UiText,
    protocol::Command,
    translation_settings::{Preferences, Reply, Request, Snapshot},
};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;
const EFFORTS: [&str; 6] = ["", "minimal", "low", "medium", "high", "xhigh"];
pub struct Panel {
    pub id: u64,
    pub dialog: adw::PreferencesDialog,
    sender: mpsc::Sender<Command>,
    original: RefCell<Option<Snapshot>>,
    draft: RefCell<Option<Preferences>>,
    backend: Cell<usize>,
    initial_backend: String,
    engine: adw::ComboRow,
    backend_row: adw::ComboRow,
    model: adw::EntryRow,
    effort: adw::ComboRow,
    local: adw::EntryRow,
    local_choices: adw::ComboRow,
    refresh: gtk::Button,
    save: gtk::Button,
    reload: gtk::Button,
    status: gtk::Label,
    busy: Cell<bool>,
    serial: Cell<u64>,
    updating: Cell<bool>,
    guard: RefCell<Option<adw::AlertDialog>>,
}
impl Panel {
    pub fn new(sender: mpsc::Sender<Command>, initial_backend: String) -> Rc<Self> {
        let dialog = adw::PreferencesDialog::builder()
            .title("Translation")
            .content_width(660)
            .content_height(690)
            .build()
            .ui_text("title", "Translation");
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder()
            .title("Translation engine")
            .description(
                "Translation runs in an isolated task and does not alter the chat composer.",
            )
            .build()
            .ui_text("title", "Translation engine")
            .ui_text(
                "description",
                "Translation runs in an isolated task and does not alter the chat composer.",
            );
        let engine = adw::ComboRow::builder()
            .title("Translation engine")
            .model(&gtk::StringList::new(&[
                "Current session backend",
                "Local Ollama",
            ]))
            .build()
            .ui_text("title", "Translation engine");
        engine.set_factory(Some(&crate::i18n_ui::choice_factory()));
        engine.set_list_factory(Some(&crate::i18n_ui::choice_factory()));
        group.add(&engine);
        page.add(&group);
        let backend_group=adw::PreferencesGroup::builder().title("Backend settings").description("Each backend keeps its own translation model and effort. Leave the model blank to use the session model; Codex defaults to Low.").build().ui_text("title","Backend settings").ui_text("description","Each backend keeps its own translation model and effort. Leave the model blank to use the session model; Codex defaults to Low.");
        let backend_row = adw::ComboRow::builder()
            .title("Backend")
            .build()
            .ui_text("title", "Backend");
        let model = adw::EntryRow::builder()
            .title("Translation model")
            .build()
            .ui_text("title", "Translation model");
        model.set_max_length(256);
        let effort = adw::ComboRow::builder()
            .title("Translation reasoning effort")
            .model(&gtk::StringList::new(&[
                "Model default",
                "Minimal",
                "Low",
                "Medium",
                "High",
                "Extra high",
            ]))
            .build()
            .ui_text("title", "Translation reasoning effort");
        effort.set_factory(Some(&crate::i18n_ui::choice_factory()));
        effort.set_list_factory(Some(&crate::i18n_ui::choice_factory()));
        backend_group.add(&backend_row);
        backend_group.add(&model);
        backend_group.add(&effort);
        page.add(&backend_group);
        let local_group=adw::PreferencesGroup::builder().title("Local Ollama").description("Uses local Ollama independently of session backends. Listing models does not run or download them.").build().ui_text("title","Local Ollama").ui_text("description","Uses local Ollama independently of session backends. Listing models does not run or download them.");
        let local = adw::EntryRow::builder()
            .title("Local Ollama model")
            .build()
            .ui_text("title", "Local Ollama model");
        local.set_max_length(256);
        let refresh = gtk::Button::with_label("Load models").ui_text("label", "Load models");
        refresh.set_valign(gtk::Align::Center);
        local.add_suffix(&refresh);
        let local_choices = adw::ComboRow::builder()
            .title("Installed local models")
            .visible(false)
            .build()
            .ui_text("title", "Installed local models");
        local_group.add(&local);
        local_group.add(&local_choices);
        page.add(&local_group);
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
            original: RefCell::new(None),
            draft: RefCell::new(None),
            backend: Cell::new(0),
            initial_backend,
            engine,
            backend_row,
            model,
            effort,
            local,
            local_choices,
            refresh,
            save,
            reload,
            status,
            busy: Cell::new(false),
            serial: Cell::new(0),
            updating: Cell::new(false),
            guard: RefCell::new(None),
        });
        for row in [&panel.model, &panel.local] {
            let weak = Rc::downgrade(&panel);
            row.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.changed();
                }
            });
        }
        for row in [&panel.engine, &panel.effort] {
            let weak = Rc::downgrade(&panel);
            row.connect_selected_notify(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.changed();
                }
            });
        }
        let weak = Rc::downgrade(&panel);
        panel.backend_row.connect_selected_notify(move |row| {
            if let Some(p) = weak.upgrade() {
                if p.updating.get() {
                    return;
                }
                p.commit_fields();
                p.backend.set(row.selected() as usize);
                p.show_backend();
                p.controls();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.local_choices.connect_selected_notify(move |row| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() {
                    if let Some(value) = row.selected_item().and_downcast::<gtk::StringObject>() {
                        p.local.set_text(&value.string());
                    }
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.refresh.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.busy.get() {
                    p.request(Request::Models);
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.commit_fields();
                if p.busy.get() || !p.dirty() {
                    return;
                }
                let revision = p.original.borrow().as_ref().unwrap().revision.clone();
                let values = p.draft.borrow().clone().unwrap();
                p.request(Request::Save { revision, values });
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.busy.get() {
                    return;
                }
                if p.dirty() {
                    let weak = Rc::downgrade(&p);
                    p.confirm("Reload translation settings?", move || {
                        if let Some(p) = weak.upgrade() {
                            p.request(Request::Read);
                        }
                    });
                } else {
                    p.request(Request::Read);
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.busy.get() {
                    p.notice("Wait for translation settings to finish");
                } else if p.dirty() {
                    let weak = Rc::downgrade(&p);
                    p.confirm("Discard translation settings?", move || {
                        if let Some(p) = weak.upgrade() {
                            p.dialog.force_close();
                        }
                    });
                } else {
                    p.dialog.force_close();
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Some(g) = p.guard.borrow_mut().take() {
                    g.force_close();
                }
            }
        });
        panel.request(Request::Read);
        panel
    }
    fn commit_fields(&self) {
        if self.updating.get() {
            return;
        }
        let Some(mut draft) = self.draft.borrow().clone() else {
            return;
        };
        draft.engine = if self.engine.selected() == 1 {
            "ollama"
        } else {
            "backend"
        }
        .into();
        draft.ollama_model = self.local.text().trim().into();
        if let Some(backend) = self
            .original
            .borrow()
            .as_ref()
            .and_then(|s| s.backends.get(self.backend.get()))
        {
            let model = self.model.text().trim().to_string();
            if model.is_empty() {
                draft.models.remove(&backend.id);
            } else {
                draft.models.insert(backend.id.clone(), model);
            }
            if backend.kind == "codex" {
                let effort = EFFORTS[self.effort.selected().min(5) as usize];
                if effort.is_empty() {
                    draft.efforts.remove(&backend.id);
                } else {
                    draft.efforts.insert(backend.id.clone(), effort.into());
                }
            }
        }
        self.draft.replace(Some(draft));
    }
    fn show_backend(&self) {
        self.updating.set(true);
        if let (Some(original), Some(draft)) = (
            self.original.borrow().as_ref(),
            self.draft.borrow().as_ref(),
        ) {
            if let Some(backend) = original.backends.get(self.backend.get()) {
                self.model.set_text(
                    draft
                        .models
                        .get(&backend.id)
                        .map(String::as_str)
                        .unwrap_or(""),
                );
                let selected = draft
                    .efforts
                    .get(&backend.id)
                    .and_then(|e| EFFORTS.iter().position(|v| v == e))
                    .unwrap_or(0);
                self.effort.set_selected(selected as u32);
            }
        }
        self.updating.set(false);
    }
    fn dirty(&self) -> bool {
        self.original
            .borrow()
            .as_ref()
            .zip(self.draft.borrow().as_ref())
            .is_some_and(|(s, d)| &s.values != d)
    }
    pub fn may_close(&self) -> bool {
        !self.busy.get() && !self.dirty()
    }
    fn controls(&self) {
        let ready = self.original.borrow().is_some() && !self.busy.get();
        self.engine.set_sensitive(ready);
        self.backend_row.set_sensitive(ready);
        self.model.set_sensitive(ready);
        let supports_effort = self
            .original
            .borrow()
            .as_ref()
            .and_then(|s| s.backends.get(self.backend.get()))
            .is_some_and(|b| b.kind == "codex");
        self.effort.set_sensitive(ready && supports_effort);
        self.local.set_sensitive(ready);
        self.local_choices.set_sensitive(ready);
        self.refresh.set_sensitive(ready);
        self.save.set_sensitive(ready && self.dirty());
        self.reload.set_sensitive(!self.busy.get());
        self.dialog.set_can_close(self.may_close());
    }
    fn changed(&self) {
        if !self.updating.get() {
            self.commit_fields();
            self.status.set_text(&crate::language::tr(if self.dirty() {
                "Unsaved changes"
            } else {
                "Saved"
            }));
            self.controls();
        }
    }
    fn notice(&self, message: &str) {
        self.status.set_text(message);
        self.dialog.add_toast(adw::Toast::new(message));
    }
    fn confirm(self: &Rc<Self>, title: &str, action: impl Fn() + 'static) {
        if let Some(g) = self.guard.borrow().as_ref() {
            g.present(Some(&self.dialog));
            return;
        }
        let dialog = adw::AlertDialog::builder()
            .heading(crate::language::tr(title))
            .body(crate::language::tr(
                "Your unsaved translation settings will be discarded.",
            ))
            .build();
        dialog.add_responses(&[
            ("cancel", &crate::language::tr("Cancel")),
            ("discard", &crate::language::tr("Discard")),
        ]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if let Some(p) = weak.upgrade() {
                p.guard.borrow_mut().take();
                if response == "discard" {
                    action();
                }
            }
        });
        dialog.present(Some(&self.dialog));
        self.guard.replace(Some(dialog));
    }
    fn request(&self, request: Request) {
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        self.busy.set(true);
        self.status.set_text(&crate::language::tr("Loading…"));
        self.controls();
        if self
            .sender
            .try_send(Command::TranslationSettings {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.busy.set(false);
            self.notice("Translation settings queue is unavailable");
            self.controls();
        }
    }
    pub fn event(&self, serial: u64, result: Result<Reply, String>) {
        if serial != self.serial.get() || !self.busy.replace(false) {
            return;
        }
        match result {
            Ok(Reply::Settings(snapshot)) => {
                self.updating.set(true);
                self.engine
                    .set_selected(u32::from(snapshot.values.engine == "ollama"));
                self.local.set_text(&snapshot.values.ollama_model);
                let names = snapshot
                    .backends
                    .iter()
                    .map(|b| format!("{} ({})", b.name, b.id))
                    .collect::<Vec<_>>();
                self.backend_row.set_model(Some(&gtk::StringList::new(
                    &names.iter().map(String::as_str).collect::<Vec<_>>(),
                )));
                let index = snapshot
                    .backends
                    .iter()
                    .position(|b| b.id == self.initial_backend)
                    .unwrap_or(0);
                self.backend.set(index);
                self.backend_row.set_selected(index as u32);
                self.draft.replace(Some(snapshot.values.clone()));
                self.original.replace(Some(snapshot));
                self.updating.set(false);
                self.show_backend();
                self.changed();
            }
            Ok(Reply::Models(models)) => {
                self.updating.set(true);
                self.local_choices.set_model(Some(&gtk::StringList::new(
                    &models.iter().map(String::as_str).collect::<Vec<_>>(),
                )));
                self.local_choices.set_visible(!models.is_empty());
                self.local_choices.set_selected(
                    models
                        .iter()
                        .position(|s| s == self.local.text().as_str())
                        .map(|i| i as u32)
                        .unwrap_or(gtk::INVALID_LIST_POSITION),
                );
                self.updating.set(false);
                self.changed();
            }
            Err(error) => self.notice(&error),
        }
        self.controls();
    }
}

#[cfg(debug_assertions)]
impl Panel {
    pub async fn smoke_ready(&self) -> Result<(), String> {
        let start = std::time::Instant::now();
        while self.busy.get() || self.original.borrow().is_none() {
            if start.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "Translation settings did not load: {}",
                    self.status.text()
                ));
            }
            gtk::glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        Ok(())
    }
    pub async fn smoke_save(
        &self,
        engine: &str,
        model: &str,
        effort: &str,
        local: &str,
    ) -> Result<(), String> {
        self.smoke_ready().await?;
        self.engine.set_selected(u32::from(engine == "ollama"));
        self.model.set_text(model);
        self.effort
            .set_selected(EFFORTS.iter().position(|e| *e == effort).unwrap_or(0) as u32);
        self.local.set_text(local);
        if self.dirty() {
            self.save.emit_clicked();
        }
        self.smoke_ready().await?;
        if self.dirty() {
            return Err(format!(
                "Translation settings did not save: {}",
                self.status.text()
            ));
        }
        Ok(())
    }
    pub async fn smoke_models(&self) -> Result<(), String> {
        self.refresh.emit_clicked();
        self.smoke_ready().await?;
        if !self.local_choices.is_visible() {
            return Err(format!(
                "Local model catalog did not load: {}",
                self.status.text()
            ));
        }
        Ok(())
    }
}
