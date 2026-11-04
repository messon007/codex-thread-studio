//! Native forms. Schema normalization and answer validation run off GTK.
use crate::i18n_ui::UiText;
use crate::{
    interaction::{Input, Kind, Request},
    protocol::ReplyError,
};
use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scope {
    pub backend: String,
    pub generation: u64,
    pub id: String,
}
impl Scope {
    pub fn new(backend: &str, generation: u64, id: &Value) -> Option<Self> {
        if !(id.is_i64()
            || id.is_u64()
            || id.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256))
        {
            return None;
        }
        Some(Self {
            backend: backend.into(),
            generation,
            id: id.to_string(),
        })
    }
}
pub type Book = HashMap<Scope, Rc<Panel>>;
pub struct Approval {
    pub dialog: adw::MessageDialog,
    pub finished: Rc<Cell<bool>>,
    pub pending: Rc<Cell<bool>>,
    pub notice: Option<String>,
}
pub type Approvals = HashMap<Scope, Approval>;
impl Scope {
    pub fn matches(
        &self,
        backend: Option<&str>,
        generation: Option<u64>,
        id: Option<&str>,
    ) -> bool {
        backend.is_none_or(|b| b == self.backend)
            && generation.is_none_or(|g| g == self.generation)
            && id.is_none_or(|i| i == self.id)
    }
}
enum Control {
    Text(gtk::Entry),
    Boolean(gtk::CheckButton),
    Choices(Vec<gtk::CheckButton>),
    Question {
        choices: Vec<gtk::CheckButton>,
        other: Option<gtk::Entry>,
        other_choice: Option<gtk::CheckButton>,
        multiple: bool,
    },
}
impl Control {
    fn input(&self) -> Input {
        let selected = |choices: &[gtk::CheckButton]| {
            choices
                .iter()
                .enumerate()
                .filter_map(|(i, c)| c.is_active().then_some(i))
                .collect()
        };
        match self {
            Self::Text(e) => Input::Text(e.text().to_string()),
            Self::Boolean(b) => Input::Boolean(b.is_active()),
            Self::Choices(c) => Input::Choices(selected(c)),
            Self::Question {
                choices,
                other,
                other_choice,
                multiple,
            } => {
                if *multiple {
                    return Input::Selected {
                        choices: selected(choices),
                        other: other
                            .as_ref()
                            .map(|e| e.text().to_string())
                            .unwrap_or_default(),
                    };
                }
                if let Some(e) = other {
                    if choices.is_empty()
                        || other_choice
                            .as_ref()
                            .is_some_and(|choice| choice.is_active())
                    {
                        return Input::Text(e.text().to_string());
                    }
                }
                Input::Choices(selected(choices))
            }
        }
    }
    fn focus(&self) {
        match self {
            Self::Text(e) => {
                e.grab_focus();
            }
            Self::Boolean(b) => {
                b.grab_focus();
            }
            Self::Choices(c) => {
                if let Some(c) = c.first() {
                    c.grab_focus();
                }
            }
            Self::Question {
                choices,
                other,
                other_choice,
                ..
            } => {
                if let Some(e) = other.as_ref().filter(|_| {
                    choices.is_empty()
                        || other_choice
                            .as_ref()
                            .is_some_and(|choice| choice.is_active())
                }) {
                    e.grab_focus();
                } else if let Some(c) = choices
                    .iter()
                    .find(|choice| choice.is_active())
                    .or(choices.first())
                {
                    c.grab_focus();
                }
            }
        }
    }
}
pub struct Panel {
    pub dialog: adw::Dialog,
    request: Result<Arc<Request>, String>,
    question: bool,
    opencode: bool,
    controls: Vec<Control>,
    content: gtk::Box,
    error: gtk::Label,
    busy: Cell<bool>,
    finished: Cell<bool>,
    launch: gio::Cancellable,
    cancel: gtk::Button,
    pub on_reply: RefCell<Option<Box<dyn Fn(Result<Value, ReplyError>) -> Result<(), String>>>>,
    pub on_closed: RefCell<Option<Box<dyn Fn()>>>,
}
fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .selectable(true)
        .build()
}
fn entry(secret: bool) -> gtk::Entry {
    let e = gtk::Entry::builder()
        .hexpand(true)
        .max_length(16384)
        .visibility(!secret)
        .build();
    if secret {
        e.set_input_purpose(gtk::InputPurpose::Password);
    }
    e
}
fn choices(
    container: &gtk::Box,
    source: &[crate::interaction::Choice],
    multiple: bool,
    selected: &[usize],
) -> Vec<gtk::CheckButton> {
    let mut controls: Vec<gtk::CheckButton> = Vec::new();
    for (i, choice) in source.iter().enumerate() {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 3);
        let title = label(&choice.label);
        title.set_selectable(false);
        body.append(&title);
        if !choice.description.is_empty() {
            let description = label(&choice.description);
            description.set_selectable(false);
            description.add_css_class("dim-label");
            body.append(&description);
        }
        let control = gtk::CheckButton::builder()
            .child(&body)
            .valign(gtk::Align::Start)
            .build();
        control.update_property(&[
            gtk::accessible::Property::Label(&choice.label),
            gtk::accessible::Property::Description(&choice.description),
        ]);
        if !multiple {
            if let Some(first) = controls.first() {
                control.set_group(Some(first));
            }
        }
        control.set_active(selected.contains(&i));
        container.append(&control);
        controls.push(control);
    }
    controls
}
impl Panel {
    pub fn new(
        parent: &impl IsA<gtk::Widget>,
        origin: &str,
        request: Result<Arc<Request>, String>,
        method: &str,
    ) -> Rc<Self> {
        let question = matches!(method, "item/tool/requestUserInput" | "opencode/question");
        let opencode = method == "opencode/question";
        let height = match request.as_deref() {
            Ok(Request::Questions(_) | Request::OpenCodeQuestions(_)) => 620,
            Ok(Request::Form { fields, .. }) if fields.len() > 4 => 640,
            Ok(Request::Url { .. }) => 360,
            Err(_) => 320,
            _ => 520,
        };
        let dialog = adw::Dialog::builder()
            .title("Agent needs your input")
            .content_width(620)
            .content_height(height)
            .can_close(false)
            .build()
            .ui_text("title", "Agent needs your input");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = adw::HeaderBar::new();
        let caption = match request.as_deref() {
            Ok(Request::Form { server, .. } | Request::Url { server, .. }) => {
                format!("{origin} · {server}")
            }
            _ => origin.into(),
        };
        header.set_title_widget(Some(
            &adw::WindowTitle::new("Agent needs your input", &caption)
                .ui_text("title", "Agent needs your input"),
        ));
        root.append(&header);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.set_margin_top(12);
        content.set_margin_bottom(20);
        let origin_label = label(origin);
        origin_label.set_selectable(false);
        origin_label.add_css_class("dim-label");
        content.append(&origin_label);
        let mut controls = Vec::new();
        match request.as_ref() {
            Ok(request) => match request.as_ref() {
                Request::Questions(questions) | Request::OpenCodeQuestions(questions) => {
                    for q in questions {
                        let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
                        if !q.header.is_empty() {
                            let h = label(&q.header);
                            h.add_css_class("heading");
                            group.append(&h);
                        }
                        group.append(&label(&q.text));
                        let options = choices(
                            &group,
                            &q.choices,
                            q.multiple,
                            if q.choices.is_empty() || q.multiple {
                                &[]
                            } else {
                                &[0]
                            },
                        );
                        let mut other_choice = None;
                        let other = if (!opencode && q.choices.is_empty()) || q.other {
                            let e = entry(q.secret);
                            e.update_property(&[
                                gtk::accessible::Property::Label(&q.text),
                                gtk::accessible::Property::Required(true),
                            ]);
                            crate::i18n_ui::bind(
                                &e,
                                "placeholder-text",
                                if q.choices.is_empty() {
                                    "Enter a response"
                                } else {
                                    "Other response"
                                },
                            );
                            if !q.multiple && !options.is_empty() {
                                let choice = gtk::CheckButton::with_label("Other response")
                                    .ui_text("label", "Other response");
                                choice.set_group(Some(&options[0]));
                                let weak = choice.downgrade();
                                e.connect_changed(move |_| {
                                    if let Some(choice) = weak.upgrade() {
                                        choice.set_active(true);
                                    }
                                });
                                group.append(&choice);
                                e.set_margin_start(24);
                                other_choice = Some(choice);
                            }
                            group.append(&e);
                            Some(e)
                        } else {
                            None
                        };
                        controls.push(Control::Question {
                            choices: options,
                            other,
                            other_choice,
                            multiple: q.multiple,
                        });
                        content.append(&group);
                    }
                }
                Request::Form {
                    server,
                    message,
                    fields,
                } => {
                    let name = label(server);
                    name.add_css_class("heading");
                    content.append(&name);
                    content.append(&label(message));
                    for field in fields {
                        let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
                        let title = label(&format!(
                            "{}{}",
                            field.title,
                            if field.required { " *" } else { "" }
                        ));
                        group.append(&title);
                        if !field.description.is_empty() {
                            let d = label(&field.description);
                            d.add_css_class("dim-label");
                            group.append(&d);
                        }
                        let control = match (&field.kind, &field.default) {
                            (Kind::Boolean, Input::Boolean(value)) => {
                                let b = gtk::CheckButton::with_label("Yes").ui_text("label", "Yes");
                                b.update_property(&[gtk::accessible::Property::Label(
                                    &field.title,
                                )]);
                                b.set_active(*value);
                                group.append(&b);
                                Control::Boolean(b)
                            }
                            (
                                Kind::Select {
                                    choices: source,
                                    multiple,
                                    ..
                                },
                                Input::Choices(default),
                            ) => Control::Choices(choices(&group, source, *multiple, default)),
                            (_, Input::Text(value)) => {
                                let e = entry(false);
                                e.update_property(&[
                                    gtk::accessible::Property::Label(&field.title),
                                    gtk::accessible::Property::Required(field.required),
                                ]);
                                if let Kind::Text { format, .. } = &field.kind {
                                    e.set_placeholder_text(match format.as_str() {
                                        "date" => Some("YYYY-MM-DD"),
                                        "date-time" => Some("YYYY-MM-DDTHH:MM:SS+00:00"),
                                        "email" => Some("name@example.com"),
                                        "uri" => Some("Absolute URI"),
                                        _ => None,
                                    });
                                }
                                e.set_text(value);
                                title.set_mnemonic_widget(Some(&e));
                                group.append(&e);
                                Control::Text(e)
                            }
                            _ => unreachable!("worker-produced default matches its field"),
                        };
                        controls.push(control);
                        content.append(&group);
                    }
                }
                Request::Url {
                    server,
                    message,
                    url,
                    host,
                } => {
                    let name = label(server);
                    name.add_css_class("heading");
                    content.append(&name);
                    content.append(&label(message));
                    let destination = label("");
                    crate::i18n_ui::bind_format(
                        &destination,
                        "label",
                        "Open in your browser: {host}",
                        &[("host", host)],
                    );
                    destination.add_css_class("heading");
                    content.append(&destination);
                    content.append(&label(url));
                }
            },
            Err(reason) => {
                let message = label(reason);
                message.add_css_class("error");
                content.append(&message);
            }
        }
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&content)
            .build();
        root.append(&scroll);
        let error = label("");
        error.add_css_class("error");
        error.set_visible(false);
        error.set_margin_start(20);
        error.set_margin_end(20);
        root.append(&error);
        let actions = crate::adaptive::toolbar(8);
        actions.set_margin_start(20);
        actions.set_margin_end(20);
        actions.set_margin_top(12);
        actions.set_margin_bottom(16);
        let cancel = gtk::Button::with_label("Cancel").ui_text("label", "Cancel");
        actions.append(&cancel);
        let decline = gtk::Button::with_label("Decline").ui_text("label", "Decline");
        if !question {
            actions.append(&decline);
        }
        let open = gtk::Button::with_label("Open link").ui_text("label", "Open link");
        let url_mode = matches!(request.as_deref(), Ok(Request::Url { .. }));
        if url_mode {
            actions.append(&open);
        }
        let submit = gtk::Button::with_label(if url_mode { "Completed" } else { "Submit" });
        crate::i18n_ui::bind(
            &submit,
            "label",
            if url_mode { "Completed" } else { "Submit" },
        );
        submit.add_css_class("suggested-action");
        if request.is_ok() {
            actions.append(&submit);
        }
        root.append(&actions);
        dialog.set_child(Some(&root));
        let panel = Rc::new(Self {
            dialog,
            request,
            question,
            opencode,
            controls,
            content: root,
            error,
            busy: Cell::new(false),
            finished: Cell::new(false),
            launch: gio::Cancellable::new(),
            cancel: cancel.clone(),
            on_reply: RefCell::new(None),
            on_closed: RefCell::new(None),
        });
        let weak = Rc::downgrade(&panel);
        submit.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.submit();
            }
        });
        let weak = Rc::downgrade(&panel);
        cancel.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.cancel("cancel");
            }
        });
        let weak = Rc::downgrade(&panel);
        decline.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.cancel("decline");
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(p) = weak.upgrade() {
                p.cancel("cancel");
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.launch.cancel();
                let closed = p.on_closed.borrow_mut().take();
                if let Some(closed) = closed {
                    closed();
                }
            }
        });
        for control in &panel.controls {
            let entry = match control {
                Control::Text(e) => Some(e),
                Control::Question { other, .. } => other.as_ref(),
                _ => None,
            };
            if let Some(entry) = entry {
                let weak = Rc::downgrade(&panel);
                entry.connect_changed(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.error.set_visible(false);
                    }
                });
            }
            let buttons = match control {
                Control::Boolean(b) => vec![b],
                Control::Choices(c) => c.iter().collect(),
                Control::Question {
                    choices,
                    other_choice,
                    ..
                } => choices.iter().chain(other_choice.iter()).collect(),
                _ => vec![],
            };
            for button in buttons {
                let weak = Rc::downgrade(&panel);
                button.connect_toggled(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.error.set_visible(false);
                    }
                });
            }
        }
        let weak = Rc::downgrade(&panel);
        let parent = parent.root().and_downcast::<gtk::Window>();
        open.connect_clicked(move |_| {
            let Some(panel) = weak.upgrade().filter(|p| !p.finished.get()) else {
                return;
            };
            let Ok(Request::Url { url, .. }) = panel.request.as_deref() else {
                return;
            };
            let weak = Rc::downgrade(&panel);
            gtk::UriLauncher::new(url).launch(
                parent.as_ref(),
                Some(&panel.launch),
                move |result| {
                    if let Some(panel) = weak.upgrade().filter(|p| !p.finished.get()) {
                        if let Err(error) = result {
                            if !error.matches(gio::IOErrorEnum::Cancelled) {
                                crate::i18n_ui::bind_format(
                                    &panel.error,
                                    "label",
                                    "Cannot open link: {error}",
                                    &[("error", &error.to_string())],
                                );
                                panel.error.set_visible(true);
                            }
                        }
                    }
                },
            );
        });
        panel
    }
    pub fn expire(&self) {
        self.finished.set(true);
        self.launch.cancel();
        self.dialog.force_close();
    }
    pub fn focus_initial(&self) {
        if let Some(control) = self.controls.first() {
            control.focus();
        } else {
            self.cancel.grab_focus();
        }
    }
    fn cancel(&self, action: &str) {
        if self.busy.get() || self.finished.get() {
            return;
        }
        self.finish(if self.opencode {
            Ok(json!({}))
        } else if self.question {
            Err(ReplyError {
                code: -32001,
                message: "User cancelled input".into(),
            })
        } else {
            Ok(json!({"action":action}))
        });
    }
    fn finish(&self, result: Result<Value, ReplyError>) {
        if self.finished.get() {
            return;
        }
        let result = self
            .on_reply
            .borrow()
            .as_ref()
            .ok_or("Input request is no longer connected".to_owned())
            .and_then(|reply| reply(result));
        match result {
            Ok(()) => {
                self.finished.set(true);
                self.dialog.force_close();
            }
            Err(error) => {
                self.error.set_text(&error);
                self.error.set_visible(true);
            }
        }
    }
    fn submit(self: &Rc<Self>) {
        if self.finished.get() || self.busy.replace(true) {
            return;
        }
        let Ok(request) = self.request.as_ref() else {
            self.busy.set(false);
            return;
        };
        let request = request.clone();
        let input = self.controls.iter().map(Control::input).collect::<Vec<_>>();
        self.error.set_visible(false);
        self.content.set_sensitive(false);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || request.answer(&input)).await;
            let Some(panel) = weak.upgrade().filter(|p| !p.finished.get()) else {
                return;
            };
            panel.busy.set(false);
            panel.content.set_sensitive(true);
            match result {
                Ok(Ok(value)) => panel.finish(Ok(value)),
                Ok(Err(error)) => {
                    panel.error.set_text(&error.message);
                    panel.error.set_visible(true);
                    if let Some(field) = panel.controls.get(error.field) {
                        field.focus();
                    }
                }
                Err(_) => {
                    crate::i18n_ui::bind(
                        &panel.error,
                        "label",
                        "Input validation stopped; your responses are retained",
                    );
                    panel.error.set_visible(true);
                }
            }
        });
    }
}

#[cfg(debug_assertions)]
impl Panel {
    pub fn smoke_inputs(&self) -> Vec<Input> {
        self.controls.iter().map(Control::input).collect()
    }
    pub fn smoke_set(&self, index: usize, value: Input) -> Result<(), String> {
        let control = self.controls.get(index).ok_or("No native input field")?;
        let set_choices = |choices: &[gtk::CheckButton], values: &[usize]| {
            for (i, c) in choices.iter().enumerate() {
                c.set_active(values.contains(&i));
            }
        };
        match (control, value) {
            (Control::Text(e), Input::Text(v)) => e.set_text(&v),
            (Control::Boolean(b), Input::Boolean(v)) => b.set_active(v),
            (Control::Choices(c), Input::Choices(v)) => set_choices(c, &v),
            (Control::Question { choices, .. }, Input::Choices(v)) => {
                set_choices(choices, &v);
            }
            (
                Control::Question {
                    other: Some(e),
                    other_choice,
                    ..
                },
                Input::Text(v),
            ) => {
                e.set_text(&v);
                if let Some(choice) = other_choice {
                    choice.set_active(true);
                }
            }
            (
                Control::Question { choices, other, .. },
                Input::Selected {
                    choices: indices,
                    other: text,
                },
            ) => {
                set_choices(choices, &indices);
                if let Some(e) = other {
                    e.set_text(&text);
                }
            }
            _ => return Err("Native input control has the wrong type".into()),
        }
        Ok(())
    }
    pub fn smoke_choose_other(&self, index: usize) -> Result<(), String> {
        let Some(Control::Question {
            other_choice: Some(choice),
            ..
        }) = self.controls.get(index)
        else {
            return Err("Question has no separate custom response choice".into());
        };
        choice.set_active(true);
        Ok(())
    }
    pub fn smoke_click(&self, name: &str) -> Result<(), String> {
        fn find(widget: &gtk::Widget, name: &str) -> Option<gtk::Button> {
            if let Some(button) = widget
                .downcast_ref::<gtk::Button>()
                .filter(|b| b.label().as_deref() == Some(name))
            {
                return Some(button.clone());
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(button) = find(&widget, name) {
                    return Some(button);
                }
            }
            None
        }
        find(self.content.upcast_ref(), &crate::language::tr(name))
            .ok_or("Native input action is missing")?
            .emit_clicked();
        Ok(())
    }
    pub fn smoke_error(&self) -> Option<String> {
        self.error
            .is_visible()
            .then(|| self.error.text().to_string())
    }
    pub fn smoke_secret(&self, index: usize) -> bool {
        matches!(self.controls.get(index),Some(Control::Question{other:Some(entry),..}) if !gtk::prelude::EntryExt::is_visible(entry))
    }
    pub fn smoke_busy(&self) -> bool {
        self.busy.get()
    }
}
