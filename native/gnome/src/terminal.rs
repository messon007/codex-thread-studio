//! Native terminal transport reuses the gateway's PTY, environment and cleanup.
use crate::{gateway::Api, protocol::Event};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};

#[derive(Debug)]
pub enum Control {
    Open {
        root: String,
        generation: u64,
        cols: u16,
        rows: u16,
    },
    Input {
        generation: u64,
        bytes: Vec<u8>,
    },
    Resize {
        generation: u64,
        cols: u16,
        rows: u16,
    },
    Close,
}

#[derive(Debug)]
pub enum Update {
    Bytes(Vec<u8>),
    Ready { shell: String, cwd: String },
    Exit(String),
    Error(String),
    Warning(String),
}

pub async fn connect(
    api: Api,
    root: String,
    generation: u64,
    cols: u16,
    rows: u16,
    mut commands: mpsc::Receiver<Control>,
    events: async_channel::Sender<Event>,
) -> Result<(), String> {
    let request = api.socket_request("/ws/terminal")?;
    let config = WebSocketConfig::default().max_message_size(Some(1024 * 1024));
    let (socket, _) = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        connect_async_with_config(request, Some(config), false),
    )
    .await
    .map_err(|_| "Terminal connection timed out")?
    .map_err(|e| e.to_string())?;
    let (mut output, mut input) = socket.split();
    output
        .send(Message::text(
            json!({"type":"start","root":root,"cols":cols,"rows":rows}).to_string(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    let mut exited = false;
    loop {
        tokio::select! {
            command=commands.recv()=>match command {
                Some(Control::Input{generation:current,bytes}) if current==generation=>{
                    if bytes.len()>1024*1024{return Err("Terminal input exceeds 1 MiB".into());}
                    output.send(Message::Binary(bytes.into())).await.map_err(|e|e.to_string())?;
                },
                Some(Control::Resize{generation:current,cols,rows}) if current==generation=>{
                    output.send(Message::text(json!({"type":"resize","cols":cols,"rows":rows}).to_string())).await.map_err(|e|e.to_string())?;
                },
                Some(Control::Close)|None=>{let _=output.close().await;return Ok(());},
                _=>{},
            },
            message=input.next()=>{
                let update=match message {
                    Some(Ok(Message::Binary(bytes)))=>Update::Bytes(bytes.to_vec()),
                    Some(Ok(Message::Text(text)))=>{
                        let value:serde_json::Value=serde_json::from_str(&text).map_err(|_|"Terminal sent invalid JSON")?;
                        match value["type"].as_str(){
                            Some("ready")=>Update::Ready{shell:value["shell"].as_str().unwrap_or("Shell").into(),cwd:value["cwd"].as_str().unwrap_or("").into()},
                            Some("exit")=>{exited=true;Update::Exit(format!("Shell exited ({})",value["code"]))},
                            Some("error")=>Update::Error(value["message"].as_str().unwrap_or("Terminal error").into()),
                            _=>continue,
                        }
                    },
                    Some(Ok(Message::Ping(bytes)))=>{output.send(Message::Pong(bytes)).await.map_err(|e|e.to_string())?;continue;},
                    Some(Ok(Message::Close(_)))|None=>{
                        if !exited{let _=events.send(Event::Terminal{generation,update:Update::Exit("Terminal connection closed".into())}).await;}
                        return Ok(());
                    },
                    Some(Err(error))=>return Err(error.to_string()),
                    _=>continue,
                };
                if events.send(Event::Terminal{generation,update}).await.is_err(){return Ok(());}
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use widget::Terminal;

#[cfg(target_os = "linux")]
mod widget {
    use super::*;
    use crate::protocol::Command;
    use adw::prelude::*;
    use gtk::{gdk, glib};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use vte::prelude::*;

    pub struct Terminal {
        pub widget: gtk::Box,
        view: vte::Terminal,
        status: gtk::Label,
        title: gtk::Label,
        sender: mpsc::Sender<Command>,
        generation: Cell<u64>,
        ready: Cell<bool>,
        root: RefCell<String>,
        size: Cell<(u16, u16)>,
        toast: adw::ToastOverlay,
        pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    }

    impl Terminal {
        #[cfg(debug_assertions)]
        pub(crate) fn smoke_ready(&self) -> bool {
            self.ready.get()
        }
        #[cfg(debug_assertions)]
        pub async fn smoke_test(
            &self,
            directory: &std::path::Path,
            capture: impl Fn(),
        ) -> Result<serde_json::Value, String> {
            async fn until(test: impl Fn() -> bool) -> Result<(), String> {
                let start = std::time::Instant::now();
                while !test() {
                    if start.elapsed() > std::time::Duration::from_secs(10) {
                        return Err("Native terminal acceptance timed out".into());
                    }
                    glib::timeout_future(std::time::Duration::from_millis(50)).await;
                }
                Ok(())
            }
            self.open(&directory.join("project").to_string_lossy());
            capture();
            until(|| self.ready.get())
                .await
                .map_err(|error| format!("{error}: {}", self.status.text()))?;
            let generation = self.generation.get();
            self.event(
                generation.saturating_sub(1),
                Update::Error("Stale terminal event".into()),
            );
            if !self.ready.get() {
                return Err("Stale terminal event changed the current session".into());
            }
            fn child_pid(text: &str) -> Option<u32> {
                text.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix("NATIVE_TERMINAL_PID=")?
                        .parse()
                        .ok()
                })
            }
            // Use VTE's actual paste/commit path, not a test-only transport write.
            self.view.paste_text("printf '\\033[32mNATIVE_TERMINAL_READY\\033[0m\\n原生终端\\n'; printf 'NATIVE_TERMINAL_PID=%s\\n' \"$$\"\n");
            until(|| {
                self.view
                    .text_format(vte::Format::Text)
                    .is_some_and(|text| {
                        text.lines()
                            .any(|line| line.trim() == "NATIVE_TERMINAL_READY")
                            && text.lines().any(|line| line.trim() == "原生终端")
                            && child_pid(&text).is_some()
                    })
            })
            .await
            .map_err(|error| {
                format!(
                    "{error}: status={}, columns={}, rows={}, text={:?}",
                    self.status.text(),
                    self.view.column_count(),
                    self.view.row_count(),
                    self.view.text_format(vte::Format::Text)
                )
            })?;
            let text = self
                .view
                .text_format(vte::Format::Text)
                .ok_or("Terminal text missing")?;
            let pid = child_pid(&text).ok_or("Terminal child PID missing")?;
            capture();
            self.close();
            until(|| !std::path::Path::new(&format!("/proc/{pid}")).exists()).await?;
            Ok(
                serde_json::json!({"ok":true,"nativeVte":true,"pasteCommit":true,"ansi":true,"unicode":true,"generationIsolation":true,"ptyChildReaped":true}),
            )
        }

        pub fn new(sender: mpsc::Sender<Command>, toast: &adw::ToastOverlay) -> Rc<Self> {
            let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            toolbar.set_margin_start(12);
            toolbar.set_margin_end(12);
            toolbar.set_margin_top(8);
            toolbar.set_margin_bottom(8);
            toolbar.add_css_class("toolbar");
            let title = gtk::Label::builder()
                .label("Terminal")
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .build();
            title.add_css_class("heading");
            crate::i18n_ui::bind(&title, "label", "Terminal");
            toolbar.append(&title);
            let copy = button("edit-copy-symbolic", "Copy selection (Ctrl+Shift+C)");
            let paste = button("edit-paste-symbolic", "Paste (Ctrl+Shift+V)");
            let restart = button("view-refresh-symbolic", "Restart terminal");
            let close = button("window-close-symbolic", "Close terminal");
            for button in [&copy, &paste, &restart, &close] {
                toolbar.append(button);
            }
            widget.append(&toolbar);
            widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let view = vte::Terminal::new();
            crate::appearance_ui::watch_terminal(&view);
            view.set_size(80, 24);
            crate::i18n_ui::accessible_label(&view, "Project terminal");
            view.set_scrollback_lines(10_000);
            view.set_mouse_autohide(true);
            view.set_audible_bell(false);
            view.set_input_enabled(false);
            view.set_font(Some(&gtk::pango::FontDescription::from_string(
                "Monospace 11",
            )));
            view.set_margin_start(8);
            view.set_margin_end(8);
            view.set_margin_top(8);
            view.set_margin_bottom(8);
            let scroll = gtk::ScrolledWindow::builder()
                .child(&view)
                .vexpand(true)
                .hexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build();
            widget.append(&scroll);
            let status = gtk::Label::builder()
                .label("Open a project terminal")
                .xalign(0.0)
                .margin_start(12)
                .margin_end(12)
                .margin_top(6)
                .margin_bottom(6)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            status.add_css_class("caption");
            crate::i18n_ui::bind(&status, "label", "Open a project terminal");
            widget.append(&status);
            let terminal = Rc::new(Self {
                widget,
                view,
                status,
                title,
                sender,
                generation: Cell::new(0),
                ready: Cell::new(false),
                root: RefCell::new(String::new()),
                size: Cell::new((80, 24)),
                toast: toast.clone(),
                on_close: RefCell::new(None),
            });
            let weak = Rc::downgrade(&terminal);
            terminal.view.connect_commit(move |_, text, length| {
                if let Some(terminal) = weak.upgrade() {
                    let bytes = if length == 1 && text.is_empty() {
                        vec![0]
                    } else {
                        text.as_bytes().to_vec()
                    };
                    terminal.write(bytes);
                }
            });
            let weak = Rc::downgrade(&terminal);
            copy.connect_clicked(move |_| {
                if let Some(terminal) = weak.upgrade() {
                    terminal.view.copy_clipboard_format(vte::Format::Text);
                }
            });
            let weak = Rc::downgrade(&terminal);
            paste.connect_clicked(move |_| {
                if let Some(terminal) = weak.upgrade() {
                    terminal.view.paste_clipboard();
                }
            });
            let weak = Rc::downgrade(&terminal);
            restart.connect_clicked(move |_| {
                if let Some(terminal) = weak.upgrade() {
                    let root = terminal.root.borrow().clone();
                    if !root.is_empty() {
                        terminal.open(&root);
                    }
                }
            });
            let weak = Rc::downgrade(&terminal);
            close.connect_clicked(move |_| {
                if let Some(terminal) = weak.upgrade() {
                    terminal.close();
                }
            });
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let weak = Rc::downgrade(&terminal);
            keys.connect_key_pressed(move |_, key, _, modifiers| {
                if modifiers
                    .contains(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK)
                {
                    if let Some(terminal) = weak.upgrade() {
                        if key == gdk::Key::C || key == gdk::Key::c {
                            terminal.view.copy_clipboard_format(vte::Format::Text);
                            return glib::Propagation::Stop;
                        }
                        if key == gdk::Key::V || key == gdk::Key::v {
                            terminal.view.paste_clipboard();
                            return glib::Propagation::Stop;
                        }
                    }
                }
                glib::Propagation::Proceed
            });
            terminal.view.add_controller(keys);
            let weak = Rc::downgrade(&terminal);
            terminal.view.add_tick_callback(move |_, _| {
                let Some(terminal) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if terminal.ready.get() {
                    let size = (
                        terminal.view.column_count().clamp(2, 1000) as u16,
                        terminal.view.row_count().clamp(1, 1000) as u16,
                    );
                    if terminal.size.get() != size {
                        if terminal.send(Control::Resize {
                            generation: terminal.generation.get(),
                            cols: size.0,
                            rows: size.1,
                        }) {
                            terminal.size.set(size);
                        }
                    }
                }
                glib::ControlFlow::Continue
            });
            terminal.theme();
            let weak = Rc::downgrade(&terminal);
            adw::StyleManager::default().connect_dark_notify(move |_| {
                if let Some(terminal) = weak.upgrade() {
                    terminal.theme();
                }
            });
            terminal
        }

        pub fn open(&self, root: &str) {
            self.generation.set(self.generation.get() + 1);
            self.ready.set(false);
            self.view.set_input_enabled(false);
            self.view.reset(true, true);
            *self.root.borrow_mut() = root.into();
            crate::i18n_ui::literal(&self.title, "label", root);
            crate::i18n_ui::literal(&self.title, "tooltip-text", root);
            crate::i18n_ui::bind(&self.status, "label", "Starting terminal…");
            self.send(Control::Open {
                root: root.into(),
                generation: self.generation.get(),
                cols: self.size.get().0,
                rows: self.size.get().1,
            });
            self.view.grab_focus();
        }

        fn send(&self, control: Control) -> bool {
            if self.sender.try_send(Command::Terminal(control)).is_err() {
                self.error("The terminal is busy or disconnected; input was not sent");
                false
            } else {
                true
            }
        }
        fn write(&self, bytes: Vec<u8>) {
            if self.ready.get() && !bytes.is_empty() {
                if bytes.len() > 1024 * 1024 {
                    self.error("Paste is limited to 1 MiB");
                    return;
                }
                self.send(Control::Input {
                    generation: self.generation.get(),
                    bytes,
                });
            }
        }
        pub fn event(&self, generation: u64, update: Update) {
            if generation != self.generation.get() {
                return;
            }
            match update {
                Update::Bytes(bytes) => self.view.feed(&bytes),
                Update::Ready { shell, cwd } => {
                    self.ready.set(true);
                    self.view.set_input_enabled(true);
                    crate::i18n_ui::literal(&self.status, "label", &format!("{shell} · {cwd}"));
                }
                Update::Exit(message) => {
                    self.ready.set(false);
                    self.view.set_input_enabled(false);
                    crate::i18n_ui::literal(&self.status, "label", &message);
                }
                Update::Error(error) => {
                    self.ready.set(false);
                    self.view.set_input_enabled(false);
                    self.error(&error);
                }
                Update::Warning(error) => self.error(&error),
            }
        }
        pub fn is_started(&self) -> bool {
            !self.root.borrow().is_empty()
        }
        pub fn gateway_failed(&self, error: &str) {
            self.event(self.generation.get(), Update::Error(error.into()));
        }
        pub fn close(&self) {
            self.send(Control::Close);
            self.generation.set(self.generation.get() + 1);
            self.ready.set(false);
            self.root.borrow_mut().clear();
            self.view.set_input_enabled(false);
            if let Some(callback) = self.on_close.borrow().as_ref() {
                callback();
            }
        }
        fn error(&self, error: &str) {
            crate::i18n_ui::literal(&self.status, "label", error);
            let toast = adw::Toast::new(error);
            toast.set_use_markup(false);
            self.toast.add_toast(toast);
        }
        fn theme(&self) {
            let dark = adw::StyleManager::default().is_dark();
            let foreground = gdk::RGBA::parse(if dark { "#deddda" } else { "#202020" }).unwrap();
            let background = gdk::RGBA::parse(if dark { "#202020" } else { "#ffffff" }).unwrap();
            self.view
                .set_colors(Some(&foreground), Some(&background), &[]);
        }
    }
    fn button(icon: &str, tooltip: &str) -> gtk::Button {
        let button = gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tooltip)
            .valign(gtk::Align::Center)
            .build();
        crate::i18n_ui::bind(&button, "tooltip-text", tooltip);
        button
    }
}
