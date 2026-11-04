//! Native selected-text translation. Gtk owns presentation and selection only.
use crate::{
    i18n_ui::UiText,
    protocol::Command,
    translation::{Context, Update},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
thread_local! {static ACTIVE:RefCell<Weak<Runtime>>=RefCell::new(Weak::new());}
pub type CapturedAction = Box<dyn FnOnce(String)>;
/// Freeze the current source context before an asynchronous document selection.
pub fn capture() -> Option<CapturedAction> {
    ACTIVE.with(|active| active.borrow().upgrade().and_then(|r| r.capture()))
}
pub fn start_selected(text: String) {
    if let Some(action) = capture() {
        action(text);
    }
}
struct Pending {
    context: Context,
    engine: Option<String>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
struct Audio {
    id: u64,
    ticket: u64,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
struct View {
    ticket: u64,
    dialog: adw::Dialog,
    source: gtk::TextBuffer,
    output: gtk::TextBuffer,
    profile: gtk::Label,
    status: gtk::Label,
    ipa: gtk::Label,
    pinyin: gtk::Label,
    spinner: gtk::Spinner,
    copy: gtk::Button,
    listen_source: gtk::Button,
    listen_output: gtk::Button,
    stop_audio: gtk::Button,
    audio_status: gtk::Label,
}
pub struct Runtime {
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    pending: RefCell<BTreeMap<u64, Pending>>,
    audio: RefCell<Option<Audio>>,
    view: RefCell<Option<View>>,
    selection: RefCell<Option<glib::WeakRef<gtk::Widget>>>,
    closing: Rc<Cell<bool>>,
    pub context: RefCell<Option<Box<dyn Fn() -> Option<Context>>>>,
    pub error: RefCell<Option<Box<dyn Fn(&str)>>>,
}
impl Runtime {
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let r = Rc::new(Self {
            parent: parent.downgrade(),
            sender,
            pending: RefCell::new(BTreeMap::new()),
            audio: RefCell::new(None),
            view: RefCell::new(None),
            selection: RefCell::new(None),
            closing: Rc::new(Cell::new(false)),
            context: RefCell::new(None),
            error: RefCell::new(None),
        });
        ACTIVE.with(|a| a.replace(Rc::downgrade(&r)));
        r
    }
    fn fail(&self, message: &str) {
        if let Some(error) = self.error.borrow().as_ref() {
            error(message);
        }
    }
    fn current(&self) -> Option<Context> {
        self.context.borrow().as_ref().and_then(|f| f())
    }
    fn capture(self: &Rc<Self>) -> Option<CapturedAction> {
        let weak = Rc::downgrade(self);
        let context = self.current()?;
        Some(Box::new(move |text| {
            if let Some(runtime) = weak.upgrade() {
                if runtime.current().is_some_and(|now| {
                    now.backend.id == context.backend.id && now.socket == context.socket
                }) {
                    runtime.start_with(context, text);
                } else {
                    runtime.fail("Translation stopped because the current backend changed");
                }
            }
        }))
    }
    pub fn start(self: &Rc<Self>, text: String) {
        if let Some(context) = self.current() {
            self.start_with(context, text);
        } else {
            self.fail("Select a session before translating");
        }
    }
    fn cancel(&self, ticket: u64) {
        if let Some(pending) = self.pending.borrow().get(&ticket) {
            pending
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        if let Some(audio) = self
            .audio
            .borrow()
            .as_ref()
            .filter(|audio| audio.ticket == ticket)
        {
            audio
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
    fn start_with(self: &Rc<Self>, context: Context, text: String) {
        if self.closing.get() {
            return;
        }
        if text.trim().is_empty() {
            self.fail("Select text to translate first");
            return;
        }
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let previous = self.view.borrow_mut().take();
        if let Some(previous) = previous {
            self.cancel(previous.ticket);
            previous.dialog.force_close();
        }
        let ticket = crate::translation::next_ticket();
        let dialog = adw::Dialog::builder()
            .title("Translate to Chinese")
            .content_width(800)
            .content_height(720)
            .build()
            .ui_text("title", "Translate to Chinese");
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_top(12);
        body.set_margin_bottom(12);
        let profile = gtk::Label::builder().xalign(0.0).wrap(true).build();
        profile.add_css_class("dim-label");
        body.append(&profile);
        fn text_section(body: &gtk::Box, title: &str) -> gtk::TextBuffer {
            body.append(
                &gtk::Label::builder()
                    .label(title)
                    .xalign(0.0)
                    .css_classes(["heading"])
                    .build()
                    .ui_text("label", title),
            );
            let view = gtk::TextView::builder()
                .editable(false)
                .cursor_visible(false)
                .wrap_mode(gtk::WrapMode::WordChar)
                .left_margin(12)
                .right_margin(12)
                .top_margin(12)
                .bottom_margin(12)
                .build();
            view.add_css_class("studio-content");
            crate::i18n_ui::accessible_label(&view, title);
            let scroll = gtk::ScrolledWindow::builder()
                .child(&view)
                .min_content_height(100)
                .max_content_height(260)
                .propagate_natural_height(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vexpand(true)
                .build();
            scroll.add_css_class("card");
            body.append(&scroll);
            view.buffer()
        }
        let source = text_section(&body, "Selected text");
        source.set_text(&text);
        let ipa = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .visible(false)
            .build();
        ipa.add_css_class("dim-label");
        body.append(&ipa);
        let output = text_section(&body, "Chinese translation");
        let pinyin = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .visible(false)
            .build();
        pinyin.add_css_class("dim-label");
        body.append(&pinyin);
        let audio_bar = crate::adaptive::toolbar(6);
        let listen_source =
            gtk::Button::with_label("Listen to English").ui_text("label", "Listen to English");
        let listen_output =
            gtk::Button::with_label("Listen to Chinese").ui_text("label", "Listen to Chinese");
        let stop_audio = gtk::Button::with_label("Stop").ui_text("label", "Stop");
        stop_audio.set_visible(false);
        let audio_status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .build();
        audio_bar.append(&listen_source);
        audio_bar.append(&listen_output);
        audio_bar.append(&stop_audio);
        audio_bar.append(&audio_status);
        body.append(&audio_bar);
        let weak = Rc::downgrade(self);
        listen_source.connect_clicked(move |_| {
            if let Some(runtime) = weak.upgrade() {
                runtime.speak(false);
            }
        });
        let weak = Rc::downgrade(self);
        listen_output.connect_clicked(move |_| {
            if let Some(runtime) = weak.upgrade() {
                runtime.speak(true);
            }
        });
        let weak = Rc::downgrade(self);
        stop_audio.connect_clicked(move |_| {
            if let Some(runtime) = weak.upgrade() {
                runtime.stop_audio();
            }
        });
        let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let spinner = gtk::Spinner::new();
        spinner.start();
        bottom.append(&spinner);
        let status = gtk::Label::builder()
            .label("Reading translation settings…")
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .build()
            .ui_text("label", "Reading translation settings…");
        bottom.append(&status);
        let copy = gtk::Button::with_label("Copy translation").ui_text("label", "Copy translation");
        copy.set_sensitive(false);
        bottom.append(&copy);
        body.append(&bottom);
        toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .child(&body)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        ));
        dialog.set_child(Some(&toolbar));
        let buffer = output.clone();
        copy.connect_clicked(move |button| {
            let start = buffer.start_iter();
            let end = buffer.end_iter();
            button
                .clipboard()
                .set_text(&buffer.text(&start, &end, true));
        });
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            if let Some(r) = weak.upgrade() {
                r.cancel(ticket);
                if r.view.borrow().as_ref().is_some_and(|v| v.ticket == ticket) {
                    r.view.borrow_mut().take();
                }
            }
        });
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.pending.borrow_mut().insert(
            ticket,
            Pending {
                context: context.clone(),
                engine: None,
                cancel: cancel.clone(),
            },
        );
        self.view.replace(Some(View {
            ticket,
            dialog: dialog.clone(),
            source,
            output,
            profile,
            status,
            ipa,
            pinyin,
            spinner,
            copy,
            listen_source,
            listen_output,
            stop_audio,
            audio_status,
        }));
        self.audio_controls();
        dialog.present(Some(&parent));
        if self
            .sender
            .try_send(Command::Translate {
                ticket,
                context,
                text,
                cancel,
            })
            .is_err()
        {
            self.event(
                ticket,
                Update::Done(Err("Translation worker queue is unavailable".into())),
            );
        }
    }
    pub fn translate_selection(self: &Rc<Self>) {
        // Application accelerators run before a content widget's key controller.
        // Prefer its scoped action, including isolated HTML selections that are
        // not GtkTextBuffers. Header/menu activation can still use the last
        // mapped native text selection below.
        if self
            .parent
            .upgrade()
            .and_then(|w| gtk::prelude::GtkWindowExt::focus(&w))
            .is_some_and(|w| w.activate_action("translation.translate", None).is_ok())
        {
            return;
        }
        let selected = self
            .selection
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
            .filter(|w| w.is_mapped() && has_selection(w));
        if let Some(widget) = selected {
            // Projected buffers may need asynchronous table reads. Dispatch
            // their bound action rather than exposing synthetic GTK anchors.
            let _ = widget.activate_action("translation.translate", None);
        } else {
            self.fail("Select text to translate first");
        }
    }
    pub fn refresh_context(&self) {
        if self.pending.borrow().is_empty() {
            return;
        }
        let now = self.current();
        let changed = self
            .pending
            .borrow()
            .iter()
            .filter(|(_, p)| {
                p.engine.as_deref() != Some("ollama")
                    && now.as_ref().is_none_or(|now| {
                        now.backend.id != p.context.backend.id
                            || now.socket != p.context.socket
                            || !now.ready
                    })
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in changed {
            self.cancel(id);
        }
    }
    pub fn event(&self, ticket: u64, update: Update) {
        match update {
            Update::Profile(profile) => {
                if let Some(p) = self.pending.borrow_mut().get_mut(&ticket) {
                    p.engine = Some(profile.engine.clone());
                } else {
                    return;
                }
                if let Some(v) = self.view.borrow().as_ref().filter(|v| v.ticket == ticket) {
                    v.source.set_text(&profile.source);
                    v.profile.set_text(&format!(
                        "{} · {} · {}",
                        profile.backend,
                        if profile.model.is_empty() {
                            crate::language::tr("Model default")
                        } else {
                            profile.model.clone()
                        },
                        if profile.effort.is_empty() {
                            crate::language::tr("Model default")
                        } else {
                            profile.effort.clone()
                        }
                    ));
                    crate::i18n_ui::bind(
                        &v.status,
                        "label",
                        if profile.engine == "ollama" {
                            "Translating with local Ollama…"
                        } else {
                            "Translating with the current backend…"
                        },
                    );
                }
            }
            Update::Done(result) => {
                let pending = self.pending.borrow_mut().remove(&ticket);
                let cleanup_failed = match &result {
                    Ok(value) => value.warning.is_some(),
                    Err(error) => error.contains("Translation task cleanup was not confirmed"),
                };
                if cleanup_failed
                    && self
                        .view
                        .borrow()
                        .as_ref()
                        .is_none_or(|v| v.ticket != ticket)
                {
                    self.fail("Translation task cleanup was not confirmed");
                }
                let now = self.current();
                let relevant = pending.as_ref().is_some_and(|p| {
                    p.engine.as_deref() == Some("ollama")
                        || now.as_ref().is_some_and(|now| {
                            now.backend.id == p.context.backend.id && now.socket == p.context.socket
                        })
                });
                if let Some(v) = self.view.borrow().as_ref().filter(|v| v.ticket == ticket) {
                    v.spinner.stop();
                    v.spinner.set_visible(false);
                    if relevant {
                        match result {
                            Ok(value) => {
                                v.output.set_text(&value.content.translation);
                                v.ipa.set_text(&value.content.source_pronunciation);
                                v.ipa
                                    .set_visible(!value.content.source_pronunciation.is_empty());
                                v.pinyin.set_text(&value.content.translation_pronunciation);
                                v.pinyin.set_visible(
                                    !value.content.translation_pronunciation.is_empty(),
                                );
                                v.copy.set_sensitive(true);
                                crate::i18n_ui::bind(
                                    &v.status,
                                    "label",
                                    value.warning.as_deref().unwrap_or(if value.cached {
                                        "Loaded from translation cache"
                                    } else {
                                        "Translation complete"
                                    }),
                                );
                            }
                            Err(error) => v.status.set_text(&error),
                        }
                    } else {
                        crate::i18n_ui::bind(
                            &v.status,
                            "label",
                            "Translation stopped because the current backend changed",
                        );
                    }
                }
                self.audio_controls();
                self.retry_close();
            }
        }
    }
    pub fn allow_window_close(&self) -> bool {
        if self.pending.borrow().is_empty() && self.audio.borrow().is_none() {
            // A manual close can reach another panel's Cancel guard before our
            // deferred retry runs. Do not close again after that cancellation.
            self.closing.set(false);
            return true;
        }
        self.closing.set(true);
        self.stop_audio();
        let ids = self.pending.borrow().keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.cancel(id);
        }
        let view = self.view.borrow_mut().take();
        if let Some(v) = view {
            v.dialog.force_close();
        }
        self.fail(if self.pending.borrow().is_empty() {
            "Waiting for speech to stop…"
        } else {
            "Waiting for translation cleanup…"
        });
        false
    }
    fn retry_close(&self) {
        if self.closing.get() && self.pending.borrow().is_empty() && self.audio.borrow().is_none() {
            if let Some(parent) = self.parent.upgrade() {
                let closing = self.closing.clone();
                glib::idle_add_local_once(move || {
                    if closing.replace(false) {
                        parent.close();
                    }
                });
            }
        }
    }
    fn audio_controls(&self) {
        let audio = self.audio.borrow();
        if let Some(view) = self.view.borrow().as_ref() {
            view.listen_source.set_sensitive(audio.is_none());
            view.listen_output
                .set_sensitive(audio.is_none() && view.copy.is_sensitive());
            view.stop_audio
                .set_visible(audio.as_ref().is_some_and(|a| a.ticket == view.ticket));
        }
    }
    fn stop_audio(&self) {
        if let Some(audio) = self.audio.borrow().as_ref() {
            audio
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
    fn speak(self: &Rc<Self>, translated: bool) {
        if self.audio.borrow().is_some() || self.closing.get() {
            return;
        }
        let Some((ticket, text, language)) = self.view.borrow().as_ref().map(|view| {
            let (text, language) = if translated && !view.pinyin.text().is_empty() {
                (
                    view.pinyin
                        .text()
                        .chars()
                        .take(crate::translation_text::MAX_SOURCE)
                        .collect(),
                    crate::speech::Language::Pinyin,
                )
            } else {
                let buffer = if translated {
                    &view.output
                } else {
                    &view.source
                };
                let mut end = buffer.start_iter();
                end.forward_chars(crate::translation_text::MAX_SOURCE as i32);
                (
                    buffer.text(&buffer.start_iter(), &end, true).to_string(),
                    if translated {
                        crate::speech::Language::Mandarin
                    } else {
                        crate::speech::Language::English
                    },
                )
            };
            (view.ticket, text, language)
        }) else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.audio.replace(Some(Audio {
            id,
            ticket,
            cancel: cancel.clone(),
        }));
        if let Some(view) = self.view.borrow().as_ref() {
            crate::i18n_ui::bind(&view.audio_status, "label", "Playing speech…");
        }
        self.audio_controls();
        if self
            .sender
            .try_send(Command::Speech {
                id,
                text,
                language,
                cancel,
            })
            .is_err()
        {
            self.speech_event(id, Err("Speech worker queue is unavailable".into()));
        }
    }
    pub fn speech_event(&self, id: u64, result: Result<crate::speech::Outcome, String>) {
        if !self
            .audio
            .borrow()
            .as_ref()
            .is_some_and(|audio| audio.id == id)
        {
            return;
        }
        let audio = self.audio.borrow_mut().take().unwrap();
        if let Some(view) = self
            .view
            .borrow()
            .as_ref()
            .filter(|view| view.ticket == audio.ticket)
        {
            match result {
                Ok(outcome) => crate::i18n_ui::bind(
                    &view.audio_status,
                    "label",
                    match outcome {
                        crate::speech::Outcome::Finished => "Playback complete",
                        crate::speech::Outcome::Stopped => "Playback stopped",
                    },
                ),
                Err(ref error) => view.audio_status.set_text(error),
            }
        } else if result
            .as_ref()
            .is_err_and(|e| e.contains("stop was not confirmed"))
        {
            self.fail("Speech playback stop was not confirmed");
        }
        self.audio_controls();
        self.retry_close();
    }
}
/// Extract only the first bounded selection window, without copying a long
/// TextView's whole selection into GTK/RPC before truncating it on a worker.
pub fn selection(widget: &gtk::Widget) -> Option<String> {
    let limit = crate::translation_text::MAX_SOURCE;
    if let Some(value) =
        crate::text_flow::quick_selection(widget, studio_markdown_native::flow::RICH_TEXT_BYTES)
    {
        return value
            .map(|s| s.chars().take(limit).collect::<String>())
            .filter(|s| !s.trim().is_empty());
    }
    let value = if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        let (a, b) = label.selection_bounds()?;
        label
            .text()
            .chars()
            .skip(a.min(b) as usize)
            .take((a - b).unsigned_abs() as usize)
            .take(limit)
            .collect::<String>()
    } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        let buffer = view.buffer();
        let (a, b) = buffer.selection_bounds()?;
        let mut end = a;
        end.forward_chars((b.offset() - a.offset()).min(limit as i32));
        buffer.text(&a, &end, false).to_string()
    } else {
        return None;
    };
    (!value.trim().is_empty()).then_some(value)
}
fn has_selection(widget: &gtk::Widget) -> bool {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        label.selection_bounds().is_some()
    } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        view.buffer().has_selection()
    } else {
        false
    }
}
pub fn bind(widget: &impl IsA<gtk::Widget>) {
    bind_with(
        widget,
        Rc::new(|widget| {
            if let Some(text) = selection(widget) {
                ACTIVE.with(|a| {
                    if let Some(runtime) = a.borrow().upgrade() {
                        runtime.start(text);
                    }
                });
            }
        }),
    );
}
/// Custom, snapshot-aware capture shares ordinary menus and shortcut ownership.
pub fn bind_with(widget: &impl IsA<gtk::Widget>, capture: Rc<dyn Fn(&gtk::Widget)>) {
    let widget = widget.as_ref();
    let action = gio::SimpleAction::new("translate", None);
    action.set_enabled(has_selection(widget));
    let weak = widget.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(widget) = weak.upgrade() {
            capture(&widget);
        }
    });
    let group = gio::SimpleActionGroup::new();
    group.add_action(&action);
    widget.insert_action_group("translation", Some(&group));
    let update = {
        let action = action.clone();
        let weak = widget.downgrade();
        Rc::new(move || {
            if let Some(widget) = weak.upgrade() {
                let selected = has_selection(&widget);
                action.set_enabled(selected);
                if selected && widget.is_mapped() {
                    ACTIVE.with(|a| {
                        if let Some(runtime) = a.borrow().upgrade() {
                            runtime.selection.replace(Some(widget.downgrade()));
                        }
                    });
                }
            }
        })
    };
    let menu = gio::Menu::new();
    menu.append(
        Some(&crate::language::tr("Translate selected text")),
        Some("translation.translate"),
    );
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        let click = gtk::GestureClick::new();
        click.set_button(0);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let change = update.clone();
        click.connect_pressed(move |_, _, _, _| change());
        let change = update.clone();
        click.connect_released(move |_, _, _, _| {
            let change = change.clone();
            glib::idle_add_local_once(move || change());
        });
        label.add_controller(click);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let change = update.clone();
        keys.connect_key_released(move |_, _, _, _| {
            let change = change.clone();
            glib::idle_add_local_once(move || change());
        });
        label.add_controller(keys);
        if let Some(existing) = label.extra_menu() {
            menu.prepend_section(None, &existing);
        }
        crate::i18n_ui::menu_label(&menu, menu.n_items() - 1, "Translate selected text");
        label.set_extra_menu(Some(&menu));
    } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        let changed = update.clone();
        view.buffer()
            .connect_has_selection_notify(move |_| update());
        view.buffer().connect_mark_set(move |buffer, _, mark| {
            if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                changed();
            }
        });
        if let Some(existing) = view.extra_menu() {
            menu.prepend_section(None, &existing);
        }
        crate::i18n_ui::menu_label(&menu, menu.n_items() - 1, "Translate selected text");
        view.set_extra_menu(Some(&menu));
    }
}

#[cfg(debug_assertions)]
impl Runtime {
    pub fn smoke_listen(&self, translated: bool) {
        if let Some(view) = self.view.borrow().as_ref() {
            let button = if translated {
                view.listen_output.clone()
            } else {
                view.listen_source.clone()
            };
            glib::idle_add_local_once(move || button.emit_clicked());
        }
    }
    pub fn smoke_stop_audio(&self) {
        if let Some(view) = self.view.borrow().as_ref() {
            view.stop_audio.emit_clicked();
        }
    }
    pub fn smoke_audio(&self) -> (bool, String) {
        (
            self.audio.borrow().is_some(),
            self.view
                .borrow()
                .as_ref()
                .map(|v| v.audio_status.text().to_string())
                .unwrap_or_default(),
        )
    }
    pub fn smoke_view(&self) -> Option<(adw::Dialog, String, String, String, String, bool)> {
        self.view.borrow().as_ref().map(|v| {
            (
                v.dialog.clone(),
                v.source
                    .text(&v.source.start_iter(), &v.source.end_iter(), true)
                    .into(),
                v.output
                    .text(&v.output.start_iter(), &v.output.end_iter(), true)
                    .into(),
                v.profile.text().into(),
                v.status.text().into(),
                v.copy.is_sensitive(),
            )
        })
    }
    pub fn smoke_copy(&self) {
        if let Some(v) = self.view.borrow().as_ref() {
            v.copy.emit_clicked();
        }
    }
    pub fn smoke_close(&self) {
        let dialog = self.view.borrow().as_ref().map(|v| v.dialog.clone());
        if let Some(dialog) = dialog {
            dialog.close();
        }
    }
    pub fn smoke_idle(&self) -> bool {
        self.pending.borrow().is_empty()
    }
}
