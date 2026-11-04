//! Native preference controls. Configuration remains owned by the shared gateway.
use crate::i18n_ui::UiText;
use crate::{environment::Profile, protocol::Command};
use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    rc::Rc,
};
use tokio::sync::mpsc;

struct Variable {
    row: adw::ExpanderRow,
    name: adw::EntryRow,
    value: gtk::TextBuffer,
}
struct Secret {
    name: String,
    row: adw::PasswordEntryRow,
    remove: gtk::CheckButton,
    existing: bool,
}
pub struct Settings {
    backend_config: RefCell<Option<Rc<crate::backend_settings_ui::Panel>>>,
    #[cfg(debug_assertions)]
    backend_config_row: adw::ActionRow,
    browser: RefCell<Option<Rc<crate::browser_settings_ui::Panel>>>,
    translation: RefCell<Option<Rc<crate::translation_settings_ui::Panel>>>,
    #[cfg(debug_assertions)]
    translation_row: adw::ActionRow,
    #[cfg(debug_assertions)]
    browser_row: adw::ActionRow,
    pub on_browser_changed:
        RefCell<Option<Box<dyn Fn(crate::browser_policy::NavigationPreferences, bool)>>>,
    notifications: Rc<crate::notifications_ui::Runtime>,
    notification_controls: Rc<crate::notifications_ui::Binding>,
    language_controls: Rc<crate::language_ui::Binding>,
    flow_controls: Rc<crate::message_flow_ui::Binding>,
    visibility: RefCell<Option<Rc<crate::session_visibility_ui::Panel>>>,
    pub on_visibility_changed: RefCell<Option<Box<dyn Fn(crate::session_visibility::Reply)>>>,
    #[cfg(debug_assertions)]
    visibility_row: adw::ActionRow,
    shared: RefCell<Option<Rc<crate::shared_directories_ui::Panel>>>,
    pub on_shared_changed: RefCell<Option<Box<dyn Fn()>>>,
    #[cfg(debug_assertions)]
    shared_row: adw::ActionRow,
    appearance: RefCell<Option<Rc<crate::appearance_ui::Panel>>>,
    #[cfg(debug_assertions)]
    appearance_row: adw::ActionRow,
    style_handler: RefCell<Option<glib::SignalHandlerId>>,
    id: u64,
    pub dialog: adw::PreferencesDialog,
    theme: adw::ComboRow,
    parent: adw::ApplicationWindow,
    sender: mpsc::Sender<Command>,
    serial: Cell<u64>,
    pending: RefCell<HashSet<String>>,
    loaded: Cell<bool>,
    updating: Cell<bool>,
    dirty: Cell<bool>,
    saving: Cell<bool>,
    root: RefCell<String>,
    revision: RefCell<String>,
    project: adw::ActionRow,
    choose: gtk::Button,
    reload: gtk::Button,
    save: gtk::Button,
    status: gtk::Label,
    controls: gtk::Box,
    policy: adw::ComboRow,
    hosts: adw::EntryRow,
    variables_group: adw::PreferencesGroup,
    cache_group: adw::PreferencesGroup,
    secrets_group: adw::PreferencesGroup,
    variables: RefCell<Vec<Rc<Variable>>>,
    cache: RefCell<Vec<Rc<Variable>>>,
    secrets: RefCell<Vec<Secret>>,
    backends: adw::PreferencesGroup,
    backend_rows: RefCell<Vec<adw::ActionRow>>,
}

impl Settings {
    #[cfg(debug_assertions)]
    pub fn smoke_language(&self) -> Rc<crate::language_ui::Binding> {
        self.language_controls.clone()
    }
    pub fn new(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        root: &str,
        backend: &str,
        appearance_runtime: Rc<crate::appearance_ui::Runtime>,
        notifications: Rc<crate::notifications_ui::Runtime>,
        language: Rc<crate::language_ui::Runtime>,
        message_flow: Rc<crate::message_flow_ui::Runtime>,
    ) -> Rc<Self> {
        let dialog = adw::PreferencesDialog::builder()
            .title("Settings")
            .content_width(680)
            .content_height(720)
            .build()
            .ui_text("title", "Settings");
        let appearance = adw::PreferencesPage::builder()
            .title("Appearance")
            .name("appearance")
            .icon_name("preferences-desktop-appearance-symbolic")
            .build()
            .ui_text("title", "Appearance");
        let group = adw::PreferencesGroup::new();
        let theme = adw::ComboRow::builder()
            .title("Appearance")
            .model(&gtk::StringList::new(&["Follow system", "Light", "Dark"]))
            .build()
            .ui_text("title", "Appearance");
        let style = adw::StyleManager::default();
        theme.set_selected(match style.color_scheme() {
            adw::ColorScheme::ForceLight => 1,
            adw::ColorScheme::ForceDark => 2,
            _ => 0,
        });
        let queue = sender.clone();
        let weak_dialog = dialog.downgrade();
        theme.connect_selected_notify(move |row| {
            let (name, scheme) = match row.selected() {
                1 => ("light", adw::ColorScheme::ForceLight),
                2 => ("dark", adw::ColorScheme::ForceDark),
                _ => ("system", adw::ColorScheme::Default),
            };
            if queue
                .try_send(Command::Preferences(json!({"theme":name})))
                .is_ok()
            {
                adw::StyleManager::default().set_color_scheme(scheme);
            } else if let Some(dialog) = weak_dialog.upgrade() {
                dialog.add_toast(adw::Toast::new(&crate::language::tr(
                    "Appearance was not saved; the settings queue is unavailable",
                )));
            }
        });
        group.add(&theme);
        let fonts = adw::ActionRow::builder()
            .title("Fonts and reading layout")
            .subtitle("Typography, Markdown layout and diagrams")
            .activatable(true)
            .build()
            .ui_text("title", "Fonts and reading layout")
            .ui_text("subtitle", "Typography, Markdown layout and diagrams");
        fonts.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        group.add(&fonts);
        appearance.add(&group);
        let language_controls = language.bind();
        appearance.add(&language_controls.group);
        dialog.add(&appearance);
        let sessions = adw::PreferencesPage::builder()
            .title("Sessions")
            .name("sessions")
            .icon_name("view-list-symbolic")
            .build()
            .ui_text("title", "Sessions");
        let session_group = adw::PreferencesGroup::new();
        let visibility = adw::ActionRow::builder()
            .title("Session visibility")
            .subtitle("Hidden folders and directory ignore rules")
            .activatable(true)
            .build()
            .ui_text("title", "Session visibility")
            .ui_text("subtitle", "Hidden folders and directory ignore rules");
        visibility.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        session_group.add(&visibility);
        sessions.add(&session_group);
        let flow_controls = message_flow.bind();
        sessions.add(&flow_controls.group);
        let notification_controls = notifications.bind();
        sessions.add(&notification_controls.group);
        dialog.add(&sessions);

        let files = adw::PreferencesPage::builder()
            .title("Files")
            .name("files")
            .icon_name("folder-symbolic")
            .build()
            .ui_text("title", "Files");
        let folders = adw::PreferencesGroup::new();
        let shared = adw::ActionRow::builder()
            .title("Shared folders")
            .subtitle("Read documents outside the current project")
            .activatable(true)
            .build()
            .ui_text("title", "Shared folders")
            .ui_text("subtitle", "Read documents outside the current project");
        shared.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        folders.add(&shared);
        files.add(&folders);
        dialog.add(&files);
        let browser_page = adw::PreferencesPage::builder()
            .title("Browser")
            .name("browser")
            .icon_name("web-browser-symbolic")
            .build()
            .ui_text("title", "Browser");
        let browser_group = adw::PreferencesGroup::new();
        let browser_row = adw::ActionRow::builder()
            .title("Navigation and JavaScript")
            .subtitle("Browser permissions and page scripting")
            .activatable(true)
            .build()
            .ui_text("title", "Navigation and JavaScript")
            .ui_text("subtitle", "Browser permissions and page scripting");
        browser_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        browser_group.add(&browser_row);
        browser_page.add(&browser_group);
        dialog.add(&browser_page);
        let translation_page = adw::PreferencesPage::builder()
            .title("Translation")
            .name("translation")
            .icon_name("preferences-desktop-locale-symbolic")
            .build()
            .ui_text("title", "Translation");
        let translation_group = adw::PreferencesGroup::new();
        let translation_row = adw::ActionRow::builder()
            .title("Translation settings")
            .subtitle("Backend models, reasoning effort and local Ollama")
            .activatable(true)
            .build()
            .ui_text("title", "Translation settings")
            .ui_text(
                "subtitle",
                "Backend models, reasoning effort and local Ollama",
            );
        translation_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        translation_group.add(&translation_row);
        translation_page.add(&translation_group);
        dialog.add(&translation_page);

        let environment = adw::PreferencesPage::builder()
            .title("Environment")
            .name("environment")
            .icon_name("preferences-system-symbolic")
            .build()
            .ui_text("title", "Environment");
        let project_group = adw::PreferencesGroup::builder().title("Project")
            .description("Saved profiles apply before the next Codex turn and to new terminals. OpenCode sessions keep their server environment.").build().ui_text("title","Project").ui_text("description","Saved profiles apply before the next Codex turn and to new terminals. OpenCode sessions keep their server environment.");
        let project = adw::ActionRow::builder()
            .title("Choose a project folder")
            .build()
            .ui_text("title", "Choose a project folder");
        project.set_subtitle_selectable(true);
        let choose = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Choose project folder")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("tooltip-text", "Choose project folder");
        project.add_suffix(&choose);
        project_group.add(&project);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let reload = gtk::Button::with_label("Reload").ui_text("label", "Reload");
        let save = gtk::Button::with_label("Save").ui_text("label", "Save");
        save.add_css_class("suggested-action");
        buttons.append(&reload);
        buttons.append(&save);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .build();
        let actions = adw::ActionRow::new();
        actions.add_prefix(&status);
        actions.add_suffix(&buttons);
        buttons.set_valign(gtk::Align::Center);
        project_group.add(&actions);
        environment.add(&project_group);
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 24);
        let network = adw::PreferencesGroup::builder().title("Network policy")
            .description("Passed to compatible tools as environment policy; this does not create an operating-system firewall.").build().ui_text("title","Network policy").ui_text("description","Passed to compatible tools as environment policy; this does not create an operating-system firewall.");
        let policy = adw::ComboRow::builder()
            .title("Network access")
            .model(&gtk::StringList::new(&["Restricted", "Enabled"]))
            .build()
            .ui_text("title", "Network access");
        let hosts = adw::EntryRow::builder()
            .title("Allowed hosts (comma-separated)")
            .build()
            .ui_text("title", "Allowed hosts (comma-separated)");
        network.add(&policy);
        network.add(&hosts);
        controls.append(&network);
        let variables_group = adw::PreferencesGroup::builder()
            .title("Variables")
            .build()
            .ui_text("title", "Variables");
        let cache_group = adw::PreferencesGroup::builder()
            .title("Cache paths")
            .description("Environment variables such as XDG_CACHE_HOME and CARGO_TARGET_DIR.")
            .build()
            .ui_text("title", "Cache paths")
            .ui_text(
                "description",
                "Environment variables such as XDG_CACHE_HOME and CARGO_TARGET_DIR.",
            );
        let secrets_group = adw::PreferencesGroup::builder().title("Secrets")
            .description("Stored values are hidden. Leave blank to keep a value, type to replace it, or select Remove.").build().ui_text("title","Secrets").ui_text("description","Stored values are hidden. Leave blank to keep a value, type to replace it, or select Remove.");
        controls.append(&variables_group);
        controls.append(&cache_group);
        controls.append(&secrets_group);
        let form = adw::PreferencesGroup::new();
        form.add(&controls);
        environment.add(&form);
        dialog.add(&environment);
        let backend_page = adw::PreferencesPage::builder()
            .title("Backends")
            .name("backends")
            .icon_name("network-server-symbolic")
            .build()
            .ui_text("title", "Backends");
        let configuration = adw::PreferencesGroup::new();
        let backend_config_row = adw::ActionRow::builder()
            .activatable(true)
            .build()
            .ui_text("title", "Edit backend configuration")
            .ui_text(
                "subtitle",
                "Validate and save backend instances; restart Studio to apply",
            );
        backend_config_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        configuration.add(&backend_config_row);
        let connections = adw::ActionRow::builder()
            .activatable(true)
            .build()
            .ui_text("title", "Connection diagnostics")
            .ui_text(
                "subtitle",
                "Executable, protocol, transport and connection status",
            );
        connections.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let window = parent.downgrade();
        connections.connect_activated(move |_| {
            if let Some(window) = window.upgrade() {
                let _ = gtk::prelude::WidgetExt::activate_action(&window, "win.connections", None);
            }
        });
        configuration.add(&connections);
        backend_page.add(&configuration);
        let backends = adw::PreferencesGroup::new()
            .ui_text("title", "Active backend registry")
            .ui_text(
                "description",
                "Loaded at startup. Restart Studio after saving configuration changes.",
            );
        backend_page.add(&backends);
        dialog.add(&backend_page);
        policy.set_factory(Some(&crate::i18n_ui::choice_factory()));
        policy.set_list_factory(Some(&crate::i18n_ui::choice_factory()));
        theme.set_factory(Some(&crate::i18n_ui::choice_factory()));
        theme.set_list_factory(Some(&crate::i18n_ui::choice_factory()));
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let ui = Rc::new(Self {
            backend_config: RefCell::new(None),
            #[cfg(debug_assertions)]
            backend_config_row: backend_config_row.clone(),
            language_controls,
            browser: RefCell::new(None),
            translation: RefCell::new(None),
            #[cfg(debug_assertions)]
            translation_row: translation_row.clone(),
            #[cfg(debug_assertions)]
            browser_row: browser_row.clone(),
            on_browser_changed: RefCell::new(None),
            notifications,
            notification_controls,
            visibility: RefCell::new(None),
            on_visibility_changed: RefCell::new(None),
            #[cfg(debug_assertions)]
            visibility_row: visibility.clone(),
            shared: RefCell::new(None),
            on_shared_changed: RefCell::new(None),
            #[cfg(debug_assertions)]
            shared_row: shared.clone(),
            appearance: RefCell::new(None),
            #[cfg(debug_assertions)]
            appearance_row: fonts.clone(),
            style_handler: RefCell::new(None),
            flow_controls,
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            dialog,
            theme,
            parent: parent.clone(),
            sender,
            serial: Cell::new(0),
            pending: RefCell::new(HashSet::new()),
            loaded: Cell::new(false),
            updating: Cell::new(false),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            root: RefCell::new(String::new()),
            revision: RefCell::new(String::new()),
            project,
            choose,
            reload,
            save,
            status,
            controls,
            policy,
            hosts,
            variables_group,
            cache_group,
            secrets_group,
            variables: RefCell::new(Vec::new()),
            cache: RefCell::new(Vec::new()),
            secrets: RefCell::new(Vec::new()),
            backends,
            backend_rows: RefCell::new(Vec::new()),
        });
        let weak = Rc::downgrade(&ui);
        backend_config_row.connect_activated(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(panel) = ui.backend_config.borrow().as_ref() {
                panel.dialog.present(Some(&ui.dialog));
                return;
            }
            let panel = crate::backend_settings_ui::Panel::new(ui.sender.clone());
            let weak = Rc::downgrade(&ui);
            *panel.on_changed.borrow_mut() = Some(Box::new(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.update_controls();
                }
            }));
            let weak = Rc::downgrade(&ui);
            panel.dialog.connect_closed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.backend_config.borrow_mut().take();
                    ui.update_controls();
                }
            });
            panel.dialog.present(Some(&ui.dialog));
            ui.backend_config.replace(Some(panel));
            ui.update_controls();
        });
        let weak = Rc::downgrade(&ui);
        *ui.language_controls.on_changed.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.update_controls();
            }
        }));
        let weak = Rc::downgrade(&ui);
        *ui.flow_controls.on_changed.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.update_controls();
            }
        }));
        let weak = Rc::downgrade(&ui);
        *ui.notification_controls.on_update.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.update_controls();
            }
        }));
        let weak = Rc::downgrade(&ui);
        visibility.connect_activated(move |_| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let existing = ui.visibility.borrow().clone();
            if let Some(panel) = existing {
                panel.dialog.present(Some(&ui.parent));
                return;
            }
            let panel = crate::session_visibility_ui::Panel::new(&ui.parent, ui.sender.clone());
            let weak = Rc::downgrade(&ui);
            *panel.on_applied.borrow_mut() = Some(Box::new(move |reply| {
                if let Some(ui) = weak.upgrade() {
                    if let Some(callback) = ui.on_visibility_changed.borrow().as_ref() {
                        callback(reply);
                    }
                }
            }));
            let weak = Rc::downgrade(&ui);
            panel.dialog.connect_closed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.visibility.borrow_mut().take();
                    ui.update_controls();
                }
            });
            panel.dialog.present(Some(&ui.parent));
            ui.visibility.replace(Some(panel));
            ui.update_controls();
        });
        let weak = Rc::downgrade(&ui);
        let backend = backend.to_owned();
        translation_row.connect_activated(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(panel) = ui.translation.borrow().clone() {
                panel.dialog.present(Some(&ui.dialog));
                return;
            }
            let panel =
                crate::translation_settings_ui::Panel::new(ui.sender.clone(), backend.clone());
            let weak = Rc::downgrade(&ui);
            panel.dialog.connect_closed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.translation.borrow_mut().take();
                }
            });
            panel.dialog.present(Some(&ui.dialog));
            ui.translation.replace(Some(panel));
        });
        let weak = Rc::downgrade(&ui);
        browser_row.connect_activated(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(panel) = ui.browser.borrow().as_ref() {
                panel.dialog.present(Some(&ui.dialog));
                return;
            }
            let panel = crate::browser_settings_ui::Panel::new(ui.sender.clone());
            let weak = Rc::downgrade(&ui);
            *panel.on_applied.borrow_mut() = Some(Box::new(move |values, restore| {
                if let Some(ui) = weak.upgrade() {
                    if let Some(callback) = ui.on_browser_changed.borrow().as_ref() {
                        callback(values, restore);
                    }
                }
            }));
            let weak = Rc::downgrade(&ui);
            panel.dialog.connect_closed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.browser.borrow_mut().take();
                    ui.update_controls();
                }
            });
            panel.dialog.present(Some(&ui.dialog));
            ui.browser.replace(Some(panel));
            ui.update_controls();
        });
        let weak = Rc::downgrade(&ui);
        shared.connect_activated(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            let existing = ui.shared.borrow().clone();
            if let Some(panel) = existing {
                panel.dialog.present(Some(&ui.parent));
                return;
            }
            let panel = crate::shared_directories_ui::Panel::new(&ui.parent, ui.sender.clone());
            let weak = Rc::downgrade(&ui);
            *panel.on_applied.borrow_mut() = Some(Box::new(move || {
                if let Some(ui) = weak.upgrade() {
                    if let Some(callback) = ui.on_shared_changed.borrow().as_ref() {
                        callback();
                    }
                }
            }));
            let weak = Rc::downgrade(&ui);
            panel.dialog.connect_closed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.shared.borrow_mut().take();
                    ui.update_controls();
                }
            });
            panel.dialog.present(Some(&ui.parent));
            ui.shared.replace(Some(panel));
            ui.update_controls();
        });
        let weak = Rc::downgrade(&ui);
        fonts.connect_activated(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            let existing = ui.appearance.borrow().clone();
            if let Some(panel) = existing {
                panel.dialog.present(Some(&ui.parent));
                return;
            }
            let panel = crate::appearance_ui::Panel::new(
                &ui.parent,
                ui.sender.clone(),
                appearance_runtime.clone(),
            );
            let weak = Rc::downgrade(&ui);
            let closed_id = panel.id;
            panel.dialog.connect_closed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    if ui
                        .appearance
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.id == closed_id)
                    {
                        ui.appearance.borrow_mut().take();
                    }
                    ui.update_controls();
                }
            });
            panel.dialog.present(Some(&ui.parent));
            ui.appearance.replace(Some(panel));
            ui.update_controls();
        });
        let weak = Rc::downgrade(&ui);
        ui.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                let backend = ui.backend_config.borrow().clone();
                if let Some(panel) = backend.filter(|panel| panel.may_close()) {
                    panel.dialog.force_close();
                }
                let visibility = ui.visibility.borrow().clone();
                if let Some(panel) = visibility.filter(|p| p.may_close()) {
                    panel.dialog.force_close();
                }
                let shared = ui.shared.borrow().clone();
                if let Some(panel) = shared.filter(|p| p.may_close()) {
                    panel.dialog.force_close();
                }
                let panel = ui.appearance.borrow().clone();
                if let Some(panel) = panel {
                    if panel.may_close() {
                        panel.dialog.force_close();
                    }
                }
            }
        });
        for (group, cache) in [(&ui.variables_group, false), (&ui.cache_group, true)] {
            let add = gtk::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text("Add variable")
                .build()
                .ui_text("tooltip-text", "Add variable");
            let weak = Rc::downgrade(&ui);
            add.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.add_variable(cache, "", "");
                    ui.changed();
                }
            });
            group.set_header_suffix(Some(&add));
        }
        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add secret")
            .build()
            .ui_text("tooltip-text", "Add secret");
        let weak = Rc::downgrade(&ui);
        add.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.new_secret();
            }
        });
        ui.secrets_group.set_header_suffix(Some(&add));
        let weak = Rc::downgrade(&ui);
        ui.policy.connect_selected_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.hosts.connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.save.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.save();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.reload.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let root = ui.root.borrow().clone();
                ui.replace_project(root);
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.choose.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.choose_project();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.dialog.connect_close_attempt(move |_| {
            if let Some(ui) = weak.upgrade() {
                let panel = ui.appearance.borrow().clone();
                let shared = ui.shared.borrow().clone();
                let visibility = ui.visibility.borrow().clone();
                let browser = ui.browser.borrow().clone();
                let translation = ui.translation.borrow().clone();
                let backend = ui.backend_config.borrow().clone();
                if let Some(panel) = backend.filter(|panel| !panel.may_close()) {
                    panel.dialog.present(Some(&ui.dialog));
                } else if let Some(panel) = translation {
                    panel.dialog.present(Some(&ui.dialog));
                } else if let Some(panel) = browser {
                    panel.dialog.present(Some(&ui.dialog));
                } else if ui.notifications.busy() {
                    ui.notice("Wait for notification settings to finish saving");
                } else if let Some(panel) = visibility.filter(|p| !p.may_close()) {
                    panel.dialog.present(Some(&ui.parent));
                    ui.notice("Save or discard session visibility changes before closing");
                } else if let Some(panel) = shared.filter(|p| !p.may_close()) {
                    panel.dialog.present(Some(&ui.parent));
                    ui.notice("Save or discard shared-folder changes before closing");
                } else if let Some(panel) = panel.filter(|p| !p.may_close()) {
                    panel.dialog.present(Some(&ui.parent));
                    ui.notice("Save or discard appearance changes before closing");
                } else if ui.flow_controls.saving() {
                    ui.notice(&crate::language::tr("Saving message flow…"));
                } else if ui.language_controls.saving() {
                    ui.notice(&crate::language::tr("Saving language…"));
                } else if ui.saving.get() {
                    ui.notice("Wait for the profile to finish saving");
                } else if ui.language_controls.dirty() || ui.flow_controls.dirty() {
                    let confirm = adw::AlertDialog::builder()
                        .heading(crate::language::tr("Discard settings changes?"))
                        .body(crate::language::tr(
                            "Message-flow, language or project settings have not been applied.",
                        ))
                        .build();
                    confirm.add_responses(&[
                        ("cancel", &crate::language::tr("Keep editing")),
                        ("discard", &crate::language::tr("Discard")),
                    ]);
                    confirm.set_close_response("cancel");
                    confirm.set_default_response(Some("cancel"));
                    confirm
                        .set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                    let weak = Rc::downgrade(&ui);
                    confirm.connect_response(None, move |_, response| {
                        if response == "discard" {
                            if let Some(ui) = weak.upgrade() {
                                ui.language_controls.discard();
                                ui.flow_controls.discard();
                                ui.dialog.force_close();
                            }
                        }
                    });
                    confirm.present(Some(&ui.dialog));
                } else if ui.dirty.get() {
                    let weak = Rc::downgrade(&ui);
                    ui.confirm_discard(move || {
                        if let Some(ui) = weak.upgrade() {
                            ui.dialog.force_close();
                        }
                    });
                } else {
                    ui.dialog.force_close();
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        let handler = adw::StyleManager::default().connect_color_scheme_notify(move |style| {
            if let Some(ui) = weak.upgrade() {
                let selected = match style.color_scheme() {
                    adw::ColorScheme::ForceLight => 1,
                    adw::ColorScheme::ForceDark => 2,
                    _ => 0,
                };
                if ui.theme.selected() != selected {
                    ui.theme.set_selected(selected);
                }
            }
        });
        ui.style_handler.replace(Some(handler));
        ui.load(root);
        ui.request(
            "backends",
            reqwest::Method::GET,
            "/studio/backends".into(),
            None,
        );
        ui
    }
    fn notice(&self, message: &str) {
        self.status.set_text(message);
        self.dialog.add_toast(adw::Toast::new(message));
    }
    fn changed(&self) {
        if !self.updating.get() && self.loaded.get() {
            self.dirty.set(true);
            crate::i18n_ui::bind(&self.status, "label", "Unsaved changes");
            self.update_controls();
        }
    }
    fn update_controls(&self) {
        self.controls
            .set_sensitive(self.loaded.get() && !self.saving.get());
        self.save
            .set_sensitive(self.loaded.get() && self.dirty.get() && !self.saving.get());
        self.reload
            .set_sensitive(!self.root.borrow().is_empty() && !self.saving.get());
        self.choose.set_sensitive(!self.saving.get());
        self.dialog.set_can_close(
            !self.saving.get()
                && !self.flow_controls.dirty()
                && !self.flow_controls.saving()
                && !self.language_controls.dirty()
                && !self.language_controls.saving()
                && !self.notifications.busy()
                && !self.dirty.get()
                && self.appearance.borrow().is_none()
                && self.shared.borrow().is_none()
                && self.visibility.borrow().is_none()
                && self.browser.borrow().is_none(),
        );
    }
    fn request(&self, kind: &str, method: reqwest::Method, path: String, body: Option<Value>) {
        let tag = format!("settings:{}:{}:{kind}", self.id, self.serial.get());
        if self
            .sender
            .try_send(Command::Http {
                tag: tag.clone(),
                method,
                path,
                body,
                document: None,
            })
            .is_ok()
        {
            self.pending.borrow_mut().insert(tag);
        } else {
            self.saving.set(false);
            self.notice("Settings request queue is unavailable");
            self.update_controls();
        }
    }
    pub fn event(self: &Rc<Self>, tag: &str, result: Result<Value, String>) {
        if !self.pending.borrow_mut().remove(tag) {
            return;
        }
        match result {
            Ok(value) if tag.ends_with(":backends") => self.show_backends(&value),
            Ok(_) => {}
            Err(error) => {
                self.saving.set(false);
                self.notice(&error);
                self.update_controls();
            }
        }
    }
    pub fn environment_event(self: &Rc<Self>, tag: &str, result: Result<Profile, String>) {
        if !self.pending.borrow_mut().remove(tag) {
            return;
        }
        match result {
            Ok(profile) => self.show_profile(profile),
            Err(error) => {
                self.saving.set(false);
                self.notice(&error);
                self.update_controls();
            }
        }
    }
    pub fn may_close(&self) -> bool {
        !self.saving.get()
            && self
                .translation
                .borrow()
                .as_ref()
                .is_none_or(|p| p.may_close())
            && self
                .backend_config
                .borrow()
                .as_ref()
                .is_none_or(|panel| panel.may_close())
            && !self.language_controls.dirty()
            && !self.language_controls.saving()
            && !self.flow_controls.dirty()
            && !self.flow_controls.saving()
            && self
                .browser
                .borrow()
                .as_ref()
                .is_none_or(|panel| panel.may_close())
            && !self.notifications.busy()
            && !self.dirty.get()
            && self
                .visibility
                .borrow()
                .as_ref()
                .is_none_or(|p| p.may_close())
            && self.shared.borrow().as_ref().is_none_or(|p| p.may_close())
            && self
                .appearance
                .borrow()
                .as_ref()
                .is_none_or(|p| p.may_close())
    }
    pub fn shared_event(
        &self,
        dialog: u64,
        serial: u64,
        result: Result<crate::shared_directories::Snapshot, String>,
    ) {
        let panel = self.shared.borrow().clone();
        if let Some(panel) = panel.filter(|p| p.id == dialog) {
            panel.event(serial, result);
        }
    }
    pub fn backend_config_event(
        &self,
        dialog: u64,
        serial: u64,
        result: Result<crate::backend_settings::Reply, String>,
    ) {
        let panel = self.backend_config.borrow().clone();
        if let Some(panel) = panel.filter(|panel| panel.id == dialog) {
            panel.event(serial, result);
        }
    }
    pub fn backend_config_failed(&self, error: &str) {
        if let Some(panel) = self.backend_config.borrow().as_ref() {
            panel.gateway_failed(error);
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_backend_config(&self) -> Rc<crate::backend_settings_ui::Panel> {
        self.dialog.set_visible_page_name("backends");
        self.backend_config_row.emit_by_name::<()>("activated", &[]);
        self.backend_config
            .borrow()
            .clone()
            .expect("backend configuration editor opened")
    }
    #[cfg(debug_assertions)]
    pub fn smoke_translation(&self) -> Rc<crate::translation_settings_ui::Panel> {
        self.dialog.set_visible_page_name("translation");
        self.translation_row.emit_by_name::<()>("activated", &[]);
        self.translation.borrow().as_ref().unwrap().clone()
    }
    pub fn translation_event(
        &self,
        dialog: u64,
        serial: u64,
        result: Result<crate::translation_settings::Reply, String>,
    ) {
        if let Some(panel) = self.translation.borrow().clone().filter(|p| p.id == dialog) {
            panel.event(serial, result);
        }
    }
    pub fn browser_event(
        &self,
        dialog: u64,
        serial: u64,
        result: Result<crate::browser_settings::Snapshot, String>,
    ) {
        let panel = self.browser.borrow().clone();
        if let Some(panel) = panel.filter(|panel| panel.id == dialog) {
            panel.event(serial, result);
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_browser(&self) -> Rc<crate::browser_settings_ui::Panel> {
        self.dialog.set_visible_page_name("browser");
        self.browser_row.emit_by_name::<()>("activated", &[]);
        self.browser.borrow().as_ref().unwrap().clone()
    }
    pub fn visibility_event(
        &self,
        dialog: u64,
        serial: u64,
        result: Result<crate::session_visibility::Reply, String>,
    ) {
        let panel = self.visibility.borrow().clone();
        if let Some(panel) = panel.filter(|p| p.id == dialog) {
            panel.event(serial, result);
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_visibility(&self) -> Rc<crate::session_visibility_ui::Panel> {
        self.dialog.set_visible_page_name("sessions");
        self.visibility_row.emit_by_name::<()>("activated", &[]);
        self.visibility.borrow().as_ref().unwrap().clone()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_notifications(&self) -> Rc<crate::notifications_ui::Binding> {
        self.dialog.set_visible_page_name("sessions");
        self.notification_controls.clone()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_shared(&self) -> Rc<crate::shared_directories_ui::Panel> {
        self.dialog.set_visible_page_name("files");
        self.shared_row.emit_by_name::<()>("activated", &[]);
        self.shared.borrow().as_ref().unwrap().clone()
    }
    pub fn appearance_event(
        &self,
        dialog: u64,
        serial: u64,
        result: Result<crate::appearance::Reply, String>,
    ) {
        let panel = self.appearance.borrow().clone();
        if let Some(panel) = panel.filter(|p| p.id == dialog) {
            panel.event(serial, result);
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_appearance(&self) -> Rc<crate::appearance_ui::Panel> {
        self.dialog.set_visible_page_name("appearance");
        self.appearance_row.emit_by_name::<()>("activated", &[]);
        self.appearance
            .borrow()
            .clone()
            .expect("appearance panel opened")
    }
    fn load(&self, root: &str) {
        self.serial.set(self.serial.get().wrapping_add(1));
        // A backend-list request is independent of project selection.
        self.pending
            .borrow_mut()
            .retain(|tag| tag.ends_with(":backends"));
        self.root.replace(root.into());
        self.loaded.set(false);
        self.dirty.set(false);
        crate::i18n_ui::bind(&self.project, "title", "Project folder");
        self.project.set_subtitle(root);
        self.status.set_text(if root.is_empty() {
            "Choose a project to edit its environment"
        } else {
            "Loading profile…"
        });
        self.update_controls();
        if root.is_empty() {
            return;
        }
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("root", root)
            .finish();
        self.request(
            "load",
            reqwest::Method::GET,
            format!("/studio/environment?{query}"),
            None,
        );
    }
    fn confirm_discard(&self, action: impl Fn() + 'static) {
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.parent)
            .heading("Discard environment changes?")
            .body("Changes to this project profile have not been saved.")
            .build();
        dialog.add_responses(&[("cancel", "Keep editing"), ("discard", "Discard")]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.connect_response(None, move |_, response| {
            if response == "discard" {
                action();
            }
        });
        dialog.present();
    }
    fn replace_project(self: &Rc<Self>, root: String) {
        if self.dirty.get() {
            let weak = Rc::downgrade(self);
            self.confirm_discard(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.load(&root);
                }
            });
        } else {
            self.load(&root);
        }
    }
    fn choose_project(self: &Rc<Self>) {
        let picker = gtk::FileDialog::builder()
            .title("Project environment folder")
            .modal(true)
            .build()
            .ui_text("title", "Project environment folder");
        let weak = Rc::downgrade(self);
        picker.select_folder(
            Some(&self.parent),
            None::<&gio::Cancellable>,
            move |result| {
                if let (Some(ui), Ok(folder)) = (weak.upgrade(), result) {
                    if let Some(path) = folder.path() {
                        ui.replace_project(path.to_string_lossy().into_owned());
                    }
                }
            },
        );
    }
    fn show_profile(self: &Rc<Self>, profile: Profile) {
        self.updating.set(true);
        for row in self.variables.borrow_mut().drain(..) {
            self.variables_group.remove(&row.row);
        }
        for row in self.cache.borrow_mut().drain(..) {
            self.cache_group.remove(&row.row);
        }
        for row in self.secrets.borrow_mut().drain(..) {
            self.secrets_group.remove(&row.row);
        }
        self.root.replace(profile.root.clone());
        self.revision.replace(profile.revision);
        self.project.set_subtitle(&profile.root);
        self.policy
            .set_selected(u32::from(profile.network_policy == "enabled"));
        self.hosts.set_text(&profile.allowed_hosts.join(", "));
        for (name, value) in profile.variables {
            self.add_variable(false, &name, &value);
        }
        for (name, value) in profile.cache_variables {
            self.add_variable(true, &name, &value);
        }
        for name in profile.secret_names {
            self.add_secret(&name, true);
        }
        self.loaded.set(true);
        self.saving.set(false);
        self.dirty.set(false);
        self.updating.set(false);
        self.status.set_text(if profile.configured {
            "Profile loaded"
        } else {
            "No saved profile"
        });
        self.update_controls();
    }
    fn add_variable(self: &Rc<Self>, cache: bool, name: &str, value: &str) {
        let (group, fields) = if cache {
            (&self.cache_group, &self.cache)
        } else {
            (&self.variables_group, &self.variables)
        };
        if fields.borrow().len() >= 128 {
            self.notice("A profile supports up to 128 variables per group");
            return;
        }
        let row = adw::ExpanderRow::builder()
            .title(if name.is_empty() {
                "New variable"
            } else {
                name
            })
            .expanded(name.is_empty())
            .build();
        let entry = adw::EntryRow::builder()
            .title("Name")
            .text(name)
            .build()
            .ui_text("title", "Name");
        row.add_row(&entry);
        let buffer = gtk::TextBuffer::new(None);
        buffer.set_text(value);
        let view = gtk::TextView::builder()
            .buffer(&buffer)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(12)
            .right_margin(12)
            .accepts_tab(false)
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .child(&view)
            .min_content_height(72)
            .max_content_height(144)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        row.add_row(&scroll);
        let remove = gtk::Button::builder()
            .icon_name("edit-delete-symbolic")
            .tooltip_text("Remove variable")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("tooltip-text", "Remove variable");
        row.add_suffix(&remove);
        let field = Rc::new(Variable {
            row,
            name: entry,
            value: buffer,
        });
        let weak = Rc::downgrade(self);
        let weak_field = Rc::downgrade(&field);
        field.name.connect_changed(move |entry| {
            if let Some(field) = weak_field.upgrade() {
                let text = entry.text();
                field.row.set_title(if text.is_empty() {
                    "New variable"
                } else {
                    text.as_str()
                });
            }
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        let weak = Rc::downgrade(self);
        field.value.connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        let weak = Rc::downgrade(self);
        let weak_field = Rc::downgrade(&field);
        remove.connect_clicked(move |_| {
            if let (Some(ui), Some(field)) = (weak.upgrade(), weak_field.upgrade()) {
                let (group, fields) = if cache {
                    (&ui.cache_group, &ui.cache)
                } else {
                    (&ui.variables_group, &ui.variables)
                };
                group.remove(&field.row);
                fields
                    .borrow_mut()
                    .retain(|other| !Rc::ptr_eq(other, &field));
                ui.changed();
            }
        });
        group.add(&field.row);
        fields.borrow_mut().push(field);
    }
    fn add_secret(self: &Rc<Self>, name: &str, existing: bool) {
        let row = adw::PasswordEntryRow::builder().title(name).build();
        let remove = gtk::CheckButton::with_label("Remove").ui_text("label", "Remove");
        remove.set_valign(gtk::Align::Center);
        row.add_suffix(&remove);
        let weak = Rc::downgrade(self);
        row.connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        let weak = Rc::downgrade(self);
        remove.connect_toggled(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        self.secrets_group.add(&row);
        self.secrets.borrow_mut().push(Secret {
            name: name.into(),
            row,
            remove,
            existing,
        });
    }
    fn new_secret(self: &Rc<Self>) {
        if self.secrets.borrow().len() >= 128 {
            self.notice("A profile supports up to 128 secrets");
            return;
        }
        let dialog = adw::MessageDialog::builder().transient_for(&self.parent).heading("Add secret").body("Enter its environment variable name. Set the value in the password field afterward.").build();
        let entry = gtk::Entry::builder()
            .placeholder_text("API_KEY")
            .activates_default(true)
            .build();
        dialog.set_extra_child(Some(&entry));
        dialog.add_responses(&[("cancel", "Cancel"), ("add", "Add")]);
        dialog.set_default_response(Some("add"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response == "add" {
                if let Some(ui) = weak.upgrade() {
                    let name = entry.text().trim().to_owned();
                    if !valid_name(&name) {
                        ui.notice("Use a valid environment variable name");
                        return;
                    }
                    if ui.secrets.borrow().iter().any(|row| row.name == name) {
                        ui.notice(
                            "This secret already exists; type its replacement value in that row",
                        );
                        return;
                    }
                    ui.add_secret(&name, false);
                    ui.changed();
                }
            }
        });
        dialog.present();
    }
    fn save(&self) {
        if !self.loaded.get() || self.saving.get() {
            return;
        }
        let result = (|| -> Result<Value, String> {
            let variables = collect_variables(&self.variables.borrow())?;
            let cache = collect_variables(&self.cache.borrow())?;
            let mut secrets = BTreeMap::new();
            let mut removed = Vec::new();
            for field in self.secrets.borrow().iter() {
                if field.remove.is_active() {
                    if field.existing {
                        removed.push(field.name.clone());
                    }
                    continue;
                }
                let value = field.row.text().to_string();
                if !value.is_empty() {
                    secrets.insert(field.name.clone(), value);
                } else if !field.existing {
                    return Err(format!("Enter a value for {} or select Remove", field.name));
                }
            }
            Ok(
                json!({"root":*self.root.borrow(),"expectedRevision":*self.revision.borrow(),"variables":variables,"cacheVariables":cache,
                "networkPolicy":if self.policy.selected() == 1 {"enabled"} else {"restricted"},
                "allowedHosts":self.hosts.text().split(',').map(str::trim).filter(|host| !host.is_empty()).collect::<Vec<_>>(),"secrets":secrets,"removeSecrets":removed}),
            )
        })();
        match result {
            Ok(body) => {
                self.saving.set(true);
                crate::i18n_ui::bind(&self.status, "label", "Saving profile…");
                self.update_controls();
                self.request(
                    "save",
                    reqwest::Method::PUT,
                    "/studio/environment".into(),
                    Some(body),
                );
            }
            Err(error) => self.notice(&error),
        }
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        capture: impl Fn(&adw::PreferencesDialog),
    ) -> Result<Value, String> {
        use gtk::glib;
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let started = std::time::Instant::now();
            while !test() {
                if started.elapsed() > std::time::Duration::from_secs(8) {
                    return Err("Native settings acceptance timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        until(|| self.loaded.get() && !self.backend_rows.borrow().is_empty()).await?;
        self.dialog.set_visible_page_name("environment");
        if self
            .variables
            .borrow()
            .iter()
            .all(|field| field.name.text() != "STUDIO_NATIVE_PROFILE_TEST")
        {
            return Err("Shared environment was not loaded".into());
        }
        if self.secrets.borrow().iter().all(|field| {
            field.name != "STUDIO_NATIVE_STORED_SECRET" || !field.row.text().is_empty()
        }) {
            return Err("Stored secret was not redacted".into());
        }
        self.add_variable(false, "STUDIO_NATIVE_UI_TEST", " 保留\n中文😀 ");
        self.changed();
        self.add_secret("STUDIO_NATIVE_NEW_SECRET", false);
        self.secrets
            .borrow()
            .iter()
            .find(|field| field.name == "STUDIO_NATIVE_NEW_SECRET")
            .unwrap()
            .row
            .set_text("native-private-secret");
        self.policy.set_selected(1);
        self.hosts.set_text("example.invalid, api.example.invalid");
        if !self.save.is_sensitive() {
            return Err("Environment changes did not enable Save".into());
        }
        self.save.emit_clicked();
        until(|| !self.saving.get()).await?;
        if self.dirty.get() {
            return Err(format!("Environment save failed: {}", self.status.text()));
        }
        self.reload.emit_clicked();
        until(|| self.loaded.get()).await?;
        let variables = collect_variables(&self.variables.borrow())?;
        if variables.get("STUDIO_NATIVE_UI_TEST").map(String::as_str) != Some(" 保留\n中文😀 ")
        {
            return Err("Multiline environment value did not round-trip exactly".into());
        }
        if self
            .secrets
            .borrow()
            .iter()
            .any(|field| !field.row.text().is_empty())
        {
            return Err("Saved secret was returned in clear text".into());
        }
        if self.secrets.borrow().len() != 2 {
            return Err("Saving a new secret removed the old one".into());
        }
        glib::timeout_future(std::time::Duration::from_millis(180)).await;
        capture(&self.dialog);
        self.dialog.set_visible_page_name("appearance");
        self.theme.set_selected(1);
        if adw::StyleManager::default().is_dark() {
            return Err("Appearance did not switch on the first selection".into());
        }
        self.theme.set_selected(2);
        if !adw::StyleManager::default().is_dark() {
            return Err("Dark appearance was not applied".into());
        }
        self.dialog.set_visible_page_name("environment");
        self.hosts.set_text("unsaved.invalid");
        self.dialog.close();
        until(|| {
            gtk::Window::list_toplevels()
                .iter()
                .any(|window| window.is::<adw::MessageDialog>())
        })
        .await?;
        let confirmation = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
            .find(|dialog| dialog.heading().as_deref() == Some("Discard environment changes?"))
            .ok_or("Discard confirmation was not shown")?;
        confirmation.response("cancel");
        if !self.dirty.get() || self.hosts.text() != "unsaved.invalid" {
            return Err("Cancel discarded the edited environment".into());
        }
        self.dialog.close();
        glib::timeout_future(std::time::Duration::from_millis(25)).await;
        let confirmation = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
            .find(|dialog| dialog.heading().as_deref() == Some("Discard environment changes?"))
            .ok_or("Second discard confirmation was not shown")?;
        confirmation.response("discard");
        Ok(
            json!({"ok":true,"environmentRoundTrip":true,"secretRedaction":true,"discardCancellation":true,"singleSelectionAppearance":true,"backendRegistry":true}),
        )
    }
    fn show_backends(&self, value: &Value) {
        for row in self.backend_rows.borrow_mut().drain(..) {
            self.backends.remove(&row);
        }
        let mut rows = Vec::new();
        let config = adw::ActionRow::builder()
            .title("Configuration file")
            .use_markup(false)
            .subtitle(value["configPath"].as_str().unwrap_or("Unavailable"))
            .subtitle_selectable(true)
            .build()
            .ui_text("title", "Configuration file");
        rows.push(config);
        if let Some(error) = value["configurationError"]
            .as_str()
            .filter(|value| !value.is_empty())
        {
            let row = adw::ActionRow::builder()
                .title("Configuration error")
                .use_markup(false)
                .subtitle(error)
                .build()
                .ui_text("title", "Configuration error");
            row.add_css_class("error");
            rows.push(row);
        }
        for backend in value["backends"].as_array().into_iter().flatten().take(32) {
            let row = adw::ActionRow::builder()
                .title(backend["name"].as_str().unwrap_or("Backend"))
                .use_markup(false)
                .subtitle(format!(
                    "{} · {}\n{}",
                    backend["id"].as_str().unwrap_or(""),
                    backend["kind"].as_str().unwrap_or(""),
                    backend["protocol"].as_str().unwrap_or("")
                ))
                .subtitle_selectable(true)
                .build();
            rows.push(row);
        }
        for row in &rows {
            self.backends.add(row);
        }
        self.backend_rows.replace(rows);
    }
}
impl Drop for Settings {
    fn drop(&mut self) {
        if let Some(handler) = self.style_handler.get_mut().take() {
            adw::StyleManager::default().disconnect(handler);
        }
    }
}
fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 128
        && bytes
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn collect_variables(fields: &[Rc<Variable>]) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    for field in fields {
        let name = field.name.text().trim().to_owned();
        if !valid_name(&name) {
            return Err("Use valid, unique environment variable names".into());
        }
        let value = field
            .value
            .text(&field.value.start_iter(), &field.value.end_iter(), false)
            .to_string();
        if value.len() > 16 * 1024 || value.contains('\0') {
            return Err(format!(
                "Value for {name} must be at most 16 KiB and contain no NUL"
            ));
        }
        if values.insert(name.clone(), value).is_some() {
            return Err(format!("Duplicate variable: {name}"));
        }
    }
    Ok(values)
}
