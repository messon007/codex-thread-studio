//! Isolated passive HTML content; all document controls are native widgets.
use crate::html_preview::Preview as Document;
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use webkit6::prelude::*;
const WORLD: &str = "studio-static-html-preview";
pub const MATCH_LIMIT: u32 = 10_000;
const CSP: &str = "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; media-src 'none'; worker-src 'none'; form-action 'none'; base-uri 'none'";
pub struct Preview {
    pub widget: gtk::Stack,
    pub contents: gtk::MenuButton,
    placeholder: adw::StatusPage,
    outline: gtk::StringList,
    popover: gtk::Popover,
    document: RefCell<Option<Arc<Document>>>,
    surface: RefCell<Option<webkit6::WebView>>,
    context: RefCell<Option<gtk::Popover>>,
    context_copy: RefCell<Option<gtk::gio::SimpleAction>>,
    epoch: Cell<u64>,
    failed: Cell<bool>,
    pending_target: Cell<Option<usize>>,
    pub ready: Cell<bool>,
    quote: RefCell<String>,
    complete: Cell<bool>,
    translate: gtk::gio::SimpleAction,
    heading: Cell<Option<usize>>,
    pub changed: RefCell<Option<Box<dyn Fn()>>>,
    pub navigate: RefCell<Option<Box<dyn Fn(usize)>>>,
    pub found: RefCell<Option<Box<dyn Fn(u32)>>>,
    pub loaded: RefCell<Option<Box<dyn Fn()>>>,
}
impl Preview {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Stack::builder().hexpand(true).vexpand(true).build();
        let placeholder = adw::StatusPage::builder()
            .icon_name("text-html-symbolic")
            .build();
        crate::i18n_ui::bind(&placeholder, "title", "Opening document…");
        widget.add_named(&placeholder, Some("notice"));
        let contents = gtk::MenuButton::builder()
            .icon_name("view-list-symbolic")
            .visible(false)
            .valign(gtk::Align::Center)
            .build();
        crate::i18n_ui::bind(&contents, "tooltip-text", "Table of contents");
        crate::i18n_ui::accessible_label(&contents, "Table of contents");
        let outline = gtk::StringList::new(&[]);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            item.set_child(Some(
                &gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .lines(3)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .margin_start(12)
                    .margin_end(12)
                    .margin_top(8)
                    .margin_bottom(8)
                    .build(),
            ));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(value) = item.item().and_downcast::<gtk::StringObject>() {
                item.child()
                    .and_downcast::<gtk::Label>()
                    .unwrap()
                    .set_text(&value.string());
            }
        });
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(outline.clone()))),
            Some(factory),
        );
        list.set_single_click_activate(true);
        let popover = gtk::Popover::new();
        popover.set_child(Some(
            &gtk::ScrolledWindow::builder()
                .width_request(280)
                .max_content_width(360)
                .height_request(240)
                .max_content_height(480)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&list)
                .build(),
        ));
        contents.set_popover(Some(&popover));
        let reader = Rc::new(Self {
            widget,
            contents,
            placeholder,
            outline,
            popover,
            document: RefCell::new(None),
            surface: RefCell::new(None),
            context: RefCell::new(None),
            context_copy: RefCell::new(None),
            epoch: Cell::new(0),
            failed: Cell::new(false),
            pending_target: Cell::new(None),
            ready: Cell::new(false),
            quote: RefCell::new(String::new()),
            complete: Cell::new(false),
            translate: gtk::gio::SimpleAction::new("translate", None),
            heading: Cell::new(None),
            changed: RefCell::new(None),
            navigate: RefCell::new(None),
            found: RefCell::new(None),
            loaded: RefCell::new(None),
        });
        reader.translate.set_enabled(false);
        let weak = Rc::downgrade(&reader);
        reader.translate.connect_activate(move |_, _| {
            if let Some(text) = weak
                .upgrade()
                .filter(|r| r.widget.is_mapped())
                .and_then(|r| r.selection())
            {
                crate::translation_ui::start_selected(text);
            }
        });
        let actions = gtk::gio::SimpleActionGroup::new();
        actions.add_action(&reader.translate);
        reader
            .widget
            .insert_action_group("translation", Some(&actions));
        let weak = Rc::downgrade(&reader);
        list.connect_activate(move |_, index| {
            if let Some(reader) = weak.upgrade() {
                reader.popover.popdown();
                if let Some(navigate) = reader.navigate.borrow().as_ref() {
                    navigate(index as usize);
                }
            }
        });
        reader
    }
    pub fn clear(&self) {
        self.retire_surface();
        self.document.borrow_mut().take();
        self.outline.splice(0, self.outline.n_items(), &[]);
        self.contents.set_visible(false);
        self.popover.popdown();
    }
    fn retire_surface(&self) {
        self.close_context();
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.ready.set(false);
        self.failed.set(false);
        self.pending_target.set(None);
        self.quote.borrow_mut().clear();
        self.complete.set(false);
        self.translate.set_enabled(false);
        self.heading.set(None);
        if let Some(web) = self.surface.borrow_mut().take() {
            web.stop_loading();
            self.widget.remove(&web);
        }
        self.widget.set_visible_child_name("notice");
    }
    pub fn document(self: &Rc<Self>, document: Arc<Document>) {
        if self
            .document
            .borrow()
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &document))
        {
            return;
        }
        self.clear();
        for h in &document.headings {
            self.outline
                .append(&format!("{}{}", "  ".repeat(h.depth), h.label));
        }
        self.contents.set_visible(!document.headings.is_empty());
        self.document.replace(Some(document));
    }
    pub fn open(self: &Rc<Self>, document: Arc<Document>, original: usize) {
        self.document(document);
        if self.failed.get() {
            self.retire_surface();
        }
        let document = self.document.borrow().clone().unwrap();
        let target = document
            .headings
            .iter()
            .filter(|h| h.original.is_some_and(|p| p <= original))
            .last()
            .map(|h| h.id);
        self.pending_target.set(target);
        if self.surface.borrow().is_some() {
            if let Some(target) = target {
                self.jump(target);
            }
            return;
        }
        let body = match &document.body {
            Ok(body) => body,
            Err(error) => {
                crate::i18n_ui::bind(&self.placeholder, "title", "Preview unavailable");
                crate::i18n_ui::bind(&self.placeholder, "description", error);
                return;
            }
        };
        crate::i18n_ui::bind(&self.placeholder, "title", "Opening document…");
        self.placeholder.set_description(None);
        let epoch = self.epoch.get();
        let manager = webkit6::UserContentManager::new();
        manager.register_script_message_handler("reader", Some(WORLD));
        manager.add_script(&webkit6::UserScript::for_world(
            include_str!("html_content.js"),
            webkit6::UserContentInjectedFrames::TopFrame,
            webkit6::UserScriptInjectionTime::End,
            WORLD,
            &[],
            &[],
        ));
        let weak = Rc::downgrade(self);
        manager.connect_script_message_received(Some("reader"), move |_, value| {
            let Some(reader) = weak.upgrade().filter(|r| r.epoch.get() == epoch) else {
                return;
            };
            if !value.is_string() {
                return;
            }
            let raw = value.to_str();
            if raw.len() > 128 * 1024 {
                return;
            }
            // Only our isolated world can access this handler. Its small fixed
            // messages contain DOM selection/layout, never project commands.
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&raw) else {
                return;
            };
            if message["epoch"].as_str() != Some(&epoch.to_string()) {
                return;
            }
            match message["kind"].as_str() {
                Some("ready") => {
                    reader.ready.set(true);
                    reader.widget.set_visible_child_name("html");
                    if let Some(target) = reader.pending_target.take() {
                        reader.jump(target);
                    }
                    reader.theme();
                    if let Some(loaded) = reader.loaded.borrow().as_ref() {
                        loaded();
                    }
                }
                Some("selection") => {
                    let text = message["text"].as_str().unwrap_or("");
                    if text.len() <= 64_000 {
                        reader.quote.replace(text.into());
                        if let Some(copy) = reader.context_copy.borrow().as_ref() {
                            copy.set_enabled(!text.is_empty());
                        }
                        reader
                            .complete
                            .set(message["complete"].as_bool().unwrap_or(false));
                    }
                }
                Some("heading") => {
                    reader
                        .heading
                        .set(message["id"].as_u64().and_then(|i| usize::try_from(i).ok()));
                }
                _ => return,
            }
            reader.translate.set_enabled(reader.selection().is_some());
            if let Some(changed) = reader.changed.borrow().as_ref() {
                changed();
            };
        });
        let settings = webkit6::Settings::builder()
            .enable_javascript(true)
            .enable_javascript_markup(false)
            .allow_file_access_from_file_urls(false)
            .allow_universal_access_from_file_urls(false)
            .enable_html5_local_storage(false)
            .enable_media(false)
            .enable_media_stream(false)
            .javascript_can_access_clipboard(false)
            .javascript_can_open_windows_automatically(false)
            .build();
        let mut pressure = webkit6::MemoryPressureSettings::new();
        pressure.set_memory_limit(256);
        pressure.set_kill_threshold(1.0);
        pressure.set_poll_interval(1.0);
        let context = webkit6::WebContext::builder()
            .memory_pressure_settings(&pressure)
            .build();
        context.set_cache_model(webkit6::CacheModel::DocumentViewer);
        let session = webkit6::NetworkSession::new_ephemeral();
        session.connect_download_started(|_, d| d.cancel());
        let web = webkit6::WebView::builder()
            .web_context(&context)
            .network_session(&session)
            .settings(&settings)
            .user_content_manager(&manager)
            .default_content_security_policy(CSP)
            .hexpand(true)
            .vexpand(true)
            .build();
        web.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        web.connect_create(|_, _| None);
        web.connect_run_file_chooser(|_, request| {
            request.cancel();
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_context_menu(move |web, menu, _| {
            menu.remove_all();
            if let Some(reader) = weak.upgrade().filter(|r| r.epoch.get() == epoch) {
                reader.context_menu(
                    web,
                    menu.event()
                        .and_then(|e| crate::action_menu::event_point(web, &e))
                        .unwrap_or((16.0, 16.0)),
                );
            }
            true
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        let weak_web = web.downgrade();
        keys.connect_key_pressed(move |_, key, _, mods| {
            if key == gtk::gdk::Key::Menu
                || (key == gtk::gdk::Key::F10 && mods.contains(gtk::gdk::ModifierType::SHIFT_MASK))
            {
                if let (Some(reader), Some(web)) = (
                    weak.upgrade().filter(|r| r.epoch.get() == epoch),
                    weak_web.upgrade(),
                ) {
                    reader.context_menu(&web, (16.0, 16.0));
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        web.add_controller(keys);
        let weak = Rc::downgrade(self);
        web.connect_unmap(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.close_context();
            }
        });
        web.connect_decide_policy(|_, decision, kind| {
            if matches!(
                kind,
                webkit6::PolicyDecisionType::NavigationAction
                    | webkit6::PolicyDecisionType::NewWindowAction
            ) {
                let uri = decision
                    .downcast_ref::<webkit6::NavigationPolicyDecision>()
                    .and_then(|d| d.navigation_action())
                    .and_then(|a| a.request())
                    .and_then(|r| r.uri());
                if kind == webkit6::PolicyDecisionType::NavigationAction
                    && uri.as_deref() == Some("about:blank")
                {
                    decision.use_();
                } else {
                    decision.ignore();
                }
                true
            } else {
                false
            }
        });
        let weak = Rc::downgrade(self);
        web.connect_web_process_terminated(move |_, _| {
            if let Some(reader) = weak.upgrade().filter(|r| r.epoch.get() == epoch) {
                reader.ready.set(false);
                reader.failed.set(true);
                reader.quote.borrow_mut().clear();
                reader.translate.set_enabled(false);
                crate::i18n_ui::bind(&reader.placeholder, "title", "Preview unavailable");
                crate::i18n_ui::bind(
                    &reader.placeholder,
                    "description",
                    "HTML renderer stopped. Switch to Source code, then back to Preview to retry.",
                );
                reader.widget.set_visible_child_name("notice");
                reader.translate.set_enabled(reader.selection().is_some());
                if let Some(changed) = reader.changed.borrow().as_ref() {
                    changed();
                }
            }
        });
        if let Some(find) = web.find_controller() {
            for failed in [false, true] {
                let weak = Rc::downgrade(self);
                if failed {
                    find.connect_failed_to_find_text(move |_| {
                        if let Some(r) = weak.upgrade().filter(|r| r.epoch.get() == epoch) {
                            r.found(0);
                        }
                    });
                } else {
                    find.connect_counted_matches(move |_, count| {
                        if let Some(r) = weak.upgrade().filter(|r| r.epoch.get() == epoch) {
                            r.found(count);
                        }
                    });
                }
            }
        }
        self.widget.add_named(&web, Some("html"));
        self.surface.replace(Some(web.clone()));
        let dark = if adw::StyleManager::default().is_dark() {
            "dark"
        } else {
            "light"
        };
        let html = format!("<!doctype html><html data-theme=\"{dark}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>{}</style></head><body data-studio-epoch=\"{epoch}\">{body}</body></html>", include_str!("html_content.css"));
        web.load_html(&html, Some("about:blank"));
    }
    fn found(&self, count: u32) {
        if let Some(found) = self.found.borrow().as_ref() {
            found(count);
        }
    }
    pub fn selection(&self) -> Option<String> {
        self.ready
            .get()
            .then(|| self.quote.borrow().clone())
            .filter(|s| !s.trim().is_empty())
    }
    fn close_context(&self) {
        self.context_copy.borrow_mut().take();
        if let Some(popup) = self.context.borrow_mut().take() {
            popup.popdown();
            if popup.parent().is_some() {
                popup.unparent();
            }
        }
    }
    fn context_menu(self: &Rc<Self>, web: &webkit6::WebView, point: (f64, f64)) {
        self.close_context();
        if !self.ready.get() || !self.widget.is_mapped() {
            return;
        }
        let actions = gtk::gio::SimpleActionGroup::new();
        for (name, command) in [("copy", "Copy"), ("select-all", "SelectAll")] {
            let action = gtk::gio::SimpleAction::new(name, None);
            action.set_enabled(name != "copy" || !self.quote.borrow().is_empty());
            if name == "copy" {
                self.context_copy.replace(Some(action.clone()));
            }
            let weak = web.downgrade();
            let owner = Rc::downgrade(self);
            let epoch = self.epoch.get();
            action.connect_activate(move |_, _| {
                if let (Some(web), Some(_)) = (
                    weak.upgrade(),
                    owner.upgrade().filter(|r| {
                        r.epoch.get() == epoch && r.ready.get() && r.widget.is_mapped()
                    }),
                ) {
                    web.execute_editing_command(command);
                }
            });
            actions.add_action(&action);
        }
        web.insert_action_group("html-preview", Some(&actions));
        let model = gtk::gio::Menu::new();
        for (label, action) in [
            ("Copy", "html-preview.copy"),
            ("Select all", "html-preview.select-all"),
            ("Translate selected text", "translation.translate"),
        ] {
            model.append(Some(label), Some(action));
            crate::i18n_ui::menu_label(&model, model.n_items() - 1, label);
        }
        let popup = crate::action_menu::popover(&model);
        popup.set_has_arrow(false);
        popup.set_parent(web);
        popup.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
            point.0 as i32,
            point.1 as i32,
            1,
            1,
        )));
        popup.connect_closed(|popup| {
            let weak = popup.downgrade();
            glib::idle_add_local_once(move || {
                if let Some(p) = weak.upgrade().filter(|p| p.parent().is_some()) {
                    p.unparent();
                }
            });
        });
        self.context.replace(Some(popup.clone()));
        popup.popup();
    }
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_context(&self) -> Option<gtk::Popover> {
        self.context.borrow().clone().filter(|p| p.is_mapped())
    }
    pub fn original(&self) -> Option<usize> {
        let document = self.document.borrow();
        document
            .as_ref()?
            .headings
            .get(self.heading.get()?)
            .and_then(|h| h.original)
    }
    pub fn comment_selection(&self) -> Option<String> {
        self.complete
            .get()
            .then(|| self.selection())
            .flatten()
            .filter(|s| s.len() <= 16384)
    }
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_web(&self) -> Option<webkit6::WebView> {
        self.surface.borrow().clone()
    }
    #[cfg(debug_assertions)]
    pub fn heading_original(&self, id: usize) -> Option<usize> {
        self.document
            .borrow()
            .as_ref()?
            .headings
            .get(id)
            .and_then(|h| h.original)
    }
    pub fn jump(&self, id: usize) {
        if self
            .document
            .borrow()
            .as_ref()
            .is_some_and(|d| id < d.headings.len())
        {
            if self.ready.get() {
                self.javascript(format!("StudioHtml.jump({id})"));
            } else {
                self.pending_target.set(Some(id));
            }
        }
    }
    pub fn theme(&self) {
        self.javascript(format!(
            "StudioHtml.theme({})",
            adw::StyleManager::default().is_dark()
        ));
    }
    pub fn focus(&self) {
        if let Some(web) = self.surface.borrow().as_ref() {
            web.grab_focus();
        }
    }
    pub fn stop_find(&self) {
        if let Some(find) = self
            .surface
            .borrow()
            .as_ref()
            .and_then(|w| w.find_controller())
        {
            find.search_finish();
        }
    }
    pub fn find(&self, query: &str, case: bool) {
        if let Some(find) = self
            .surface
            .borrow()
            .as_ref()
            .and_then(|w| w.find_controller())
        {
            if query.is_empty() {
                find.search_finish();
                self.found(0);
            } else {
                let mut options = webkit6::FindOptions::WRAP_AROUND;
                if !case {
                    options |= webkit6::FindOptions::CASE_INSENSITIVE;
                }
                find.search(query, options.bits(), MATCH_LIMIT);
                find.count_matches(query, options.bits(), MATCH_LIMIT);
            }
        }
    }
    pub fn step(&self, forward: bool) {
        if let Some(find) = self
            .surface
            .borrow()
            .as_ref()
            .and_then(|w| w.find_controller())
        {
            if forward {
                find.search_next();
            } else {
                find.search_previous();
            }
        }
    }
    fn javascript(&self, script: String) {
        if let Some(web) = self.surface.borrow().clone() {
            glib::spawn_future_local(async move {
                let _ = web
                    .evaluate_javascript_future(&script, Some(WORLD), None)
                    .await;
            });
        }
    }
}
