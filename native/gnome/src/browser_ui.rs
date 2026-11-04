//! GTK browser workspace. Remote pages have no Studio IPC or document capabilities.
use crate::{
    browser_policy::{sanitize_download_filename, validate_browser_url, BrowserPreferences},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gio, glib};
#[cfg(debug_assertions)]
use serde_json::Value;
#[cfg(debug_assertions)]
use std::sync::{atomic::AtomicBool, Arc};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::Ordering,
};
use tokio::sync::mpsc;
use webkit6::prelude::*;

#[cfg(debug_assertions)]
#[path = "browser_file_acceptance.rs"]
mod file_acceptance;
#[cfg(debug_assertions)]
#[path = "browser_restore_acceptance.rs"]
mod restore_acceptance;

const MAX_TABS: i32 = 8;
const MAX_DOWNLOADS: usize = 100;
const WORLD: &str = "studio-native-browser-selection";
#[derive(Clone, Copy)]
enum TabTask {
    Read { generation: u64, open: Option<u64> },
    Save { generation: u64 },
}
impl TabTask {
    fn generation(self) -> u64 {
        match self {
            Self::Read { generation, .. } | Self::Save { generation } => generation,
        }
    }
}
struct Engine {
    context: webkit6::WebContext,
    session: webkit6::NetworkSession,
}
enum FileOwner {
    Upload {
        page: glib::WeakRef<webkit6::WebView>,
    },
    Download(glib::WeakRef<webkit6::Download>),
}
struct FileChoice {
    token: gio::Cancellable,
    owner: FileOwner,
}
struct Transfer {
    download: webkit6::Download,
    row: gtk::ListBoxRow,
    complete: Rc<Cell<bool>>,
    network_done: Rc<Cell<bool>>,
    destination_pending: Rc<Cell<bool>>,
    stop: Rc<dyn Fn()>,
    target: Rc<RefCell<Option<crate::download_file::Target>>>,
}
pub struct Browser {
    pub widget: adw::ToolbarView,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    tabs: adw::TabView,
    address: gtk::Entry,
    back: gtk::Button,
    forward: gtk::Button,
    reload: gtk::Button,
    progress: gtk::ProgressBar,
    status: gtk::Label,
    search: gtk::SearchEntry,
    engine: RefCell<Option<Engine>>,
    preferences: RefCell<Option<BrowserPreferences>>,
    pending: RefCell<Option<String>>,
    request: Cell<u64>,
    loading_preferences: Cell<bool>,
    exiting: Cell<bool>,
    tab_serial: Cell<u64>,
    tab_generation: Cell<u64>,
    tab_task: Cell<Option<TabTask>>,
    tab_revision: RefCell<Option<String>>,
    tab_pending: RefCell<Option<crate::browser_tabs::Tabs>>,
    tab_saving: RefCell<Option<crate::browser_tabs::Tabs>>,
    tab_last: RefCell<Option<crate::browser_tabs::Tabs>>,
    tab_refresh: Cell<bool>,
    tab_muted: Cell<bool>,
    tab_error: Cell<bool>,
    tab_closing: Rc<Cell<bool>>,
    tab_abandon: Cell<bool>,
    tab_alert: RefCell<Option<adw::AlertDialog>>,
    downloads: RefCell<Vec<Transfer>>,
    download_list: gtk::ListBox,
    dialogs: RefCell<Vec<adw::Dialog>>,
    file_dialogs: RefCell<Vec<FileChoice>>,
    file_io: Rc<crate::file_io::Jobs>,
    pub on_hide: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
    pub comments: RefCell<Option<Rc<crate::annotations_ui::CaptureTarget>>>,
    epochs: RefCell<std::collections::HashMap<usize, Rc<Cell<u64>>>>,
    tab_targets: RefCell<std::collections::HashMap<usize, String>>,
    quote: RefCell<
        Option<(
            String,
            String,
            Option<(glib::WeakRef<webkit6::WebView>, u64)>,
        )>,
    >,
    search_row: gtk::Box,
}
impl Browser {
    #[cfg(debug_assertions)]
    pub async fn smoke_navigation(
        self: &Rc<Self>,
        panel: &Rc<crate::browser_settings_ui::Panel>,
        url: &str,
    ) -> Result<Value, String> {
        async fn wait(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(20) {
                    return Err("Live browser settings did not settle".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        async fn inspect(web: &webkit6::WebView, script: &str) -> Result<Value, String> {
            web.evaluate_javascript_future(script, Some(WORLD), None)
                .await
                .map_err(|e| e.to_string())?
                .to_json(0)
                .map(|value| serde_json::from_str(value.as_str()).unwrap_or(Value::Null))
                .ok_or("Browser policy inspection returned no result".into())
        }
        if self.engine.borrow().is_some() {
            return Err("Browser settings eagerly started an engine".into());
        }
        let mut values = panel.smoke_values();
        values.preview_javascript = false;
        panel.smoke_save(values.clone()).await?;
        if self.engine.borrow().is_some() {
            return Err("Saving settings eagerly started Browser".into());
        }
        self.open(Some(url));
        wait(|| {
            self.active().is_some_and(|web| {
                !web.is_loading()
                    && web.title().is_some()
                    && web.uri().is_some_and(|uri| uri.as_str() == url)
            })
        })
        .await?;
        let web = self.active().unwrap();
        if webkit6::prelude::WebViewExt::settings(&web)
            .unwrap()
            .enables_javascript()
        {
            return Err("Page JavaScript remained enabled".into());
        }
        // With JavaScript disabled WebKit also disables evaluate_javascript.
        // The page's own script changes its title, so the unchanged title proves
        // that script did not run without temporarily enabling it for inspection.
        if web.title().as_deref() != Some("Private browser policy /one") {
            return Err("Disabled page JavaScript changed the document title".into());
        }
        let epoch = self.epochs.borrow()[&(web.as_ptr() as usize)].get();
        values.preview_javascript = true;
        panel.smoke_save(values.clone()).await?;
        wait(|| {
            self.epochs
                .borrow()
                .get(&(web.as_ptr() as usize))
                .is_some_and(|current| current.get() > epoch)
                && !web.is_loading()
        })
        .await?;
        if inspect(&web, "({script:document.documentElement.dataset.scriptRan||null,bridge:!!window.__TAURI_INTERNALS__})").await? != serde_json::json!({"script":"yes","bridge":false}) {
            return Err("JavaScript change did not reload the open page".into());
        }
        let epoch = self.epochs.borrow()[&(web.as_ptr() as usize)].get();
        values.allow_private_network = true;
        panel.smoke_save(values.clone()).await?;
        if self.active().as_ref() != Some(&web)
            || self.epochs.borrow()[&(web.as_ptr() as usize)].get() != epoch
            || self.allowed("http://10.1.2.3").is_err()
        {
            return Err(
                "Loosening navigation replaced an allowed tab or did not apply policy".into(),
            );
        }
        values.allow_private_network = false;
        panel.smoke_save(values.clone()).await?;
        if self.allowed("http://10.1.2.3").is_ok() {
            return Err("Private-address navigation was not revoked".into());
        }
        self.add_tab(Some(url.replace("/one", "/two")));
        wait(|| {
            self.tabs.n_pages() == 2
                && self
                    .active()
                    .is_some_and(|web| !web.is_loading() && web.title().is_some())
        })
        .await?;
        values.allow_http = false;
        panel.smoke_save(values.clone()).await?;
        wait(|| self.tabs.n_pages() == 0 && self.engine.borrow().is_none()).await?;
        values.allow_http = true;
        panel.smoke_save(values.clone()).await?;
        self.open(Some(url));
        wait(|| {
            self.active()
                .is_some_and(|web| !web.is_loading() && web.title().is_some())
        })
        .await?;
        values.allow_localhost = false;
        panel.smoke_save(values.clone()).await?;
        wait(|| self.tabs.n_pages() == 0 && self.engine.borrow().is_none()).await?;
        values.allow_localhost = true;
        panel.smoke_save(values.clone()).await?;
        self.open(Some(url));
        wait(|| {
            self.active()
                .is_some_and(|web| !web.is_loading() && web.title().is_some())
        })
        .await?;
        values.enabled = false;
        panel.smoke_save(values.clone()).await?;
        if self.engine.borrow().is_some() || self.tabs.n_pages() != 0 {
            return Err("Disabling Browser retained its session".into());
        }
        self.open(Some(url));
        wait(|| !self.loading_preferences.get()).await?;
        if self.engine.borrow().is_some() {
            return Err("Disabled Browser could reopen".into());
        }
        values.enabled = true;
        panel.smoke_save(values).await?;
        if self.engine.borrow().is_some() {
            return Err("Enabling Browser started it without Open".into());
        }
        Ok(
            serde_json::json!({"lazy":true,"javascriptReload":true,"allowedTabRetention":true,"httpRevocation":true,"localhostRevocation":true,"privateAddressPolicy":true,"disableExit":true,"noStudioBridge":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        url: &str,
        capture: impl Fn(&str),
    ) -> Result<Value, String> {
        async fn wait(b: &Rc<Browser>, suffix: &str) -> Result<(), String> {
            let start = std::time::Instant::now();
            loop {
                if b.active().is_some_and(|w| {
                    !w.is_loading()
                        && w.uri().is_some_and(|u| u.ends_with(suffix))
                        && w.title().is_some()
                }) {
                    return Ok(());
                }
                if start.elapsed() > std::time::Duration::from_secs(20) {
                    return Err(format!("Browser loading timed out: {}", b.status.text()));
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
        }
        async fn inspect(web: &webkit6::WebView, script: &str) -> Result<Value, String> {
            let value = web
                .evaluate_javascript_future(&format!("JSON.stringify({script})"), Some(WORLD), None)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::from_str(&value.to_str()).map_err(|e| e.to_string())
        }
        if self.engine.borrow().is_some() || self.tabs.n_pages() != 0 {
            return Err("Browser eagerly started before Open".into());
        }
        let start = std::time::Instant::now();
        self.open(Some(url));
        wait(self, "/one").await?;
        let open_ms = start.elapsed().as_millis();
        let web = self.active().ok_or("Missing browser page")?;
        let _ = web
            .snapshot_future(
                webkit6::SnapshotRegion::Visible,
                webkit6::SnapshotOptions::NONE,
            )
            .await;
        let content=inspect(&web,"({text:document.body.textContent,script:document.documentElement.dataset.scriptRan,bridge:typeof window.webkit?.messageHandlers,tauri:typeof window.__TAURI__,storage:localStorage.getItem('native-test')})").await?;
        if !content["text"]
            .as_str()
            .is_some_and(|s| s.contains("Native browser 你好"))
            || content["script"] != "yes"
            || content["bridge"] != "undefined"
            || content["tauri"] != "undefined"
            || !content["storage"].is_null()
        {
            return Err(format!("Browser content/isolation failed: {content}"));
        }
        capture("browser-one.png");
        inspect(&web,"(()=>{localStorage.setItem('native-test','private');let r=document.createRange();r.selectNodeContents(document.getElementById('quote'));let s=getSelection();s.removeAllRanges();s.addRange(r);return true})()").await?;
        let previous = self.comments.borrow_mut().take();
        let captured = Rc::new(Cell::new(false));
        let flag = captured.clone();
        let expected = url.to_owned();
        self.comments
            .replace(Some(Rc::new(crate::annotations_ui::CaptureTarget {
                owner: Box::new(|| Some("private-browser-fixture".into())),
                capture: Box::new(move |_, capture| {
                    if capture.excerpt != "Selected native browser text."
                        || capture.source["provider"] != "browser"
                        || capture.source["anchor"]["url"] != expected
                    {
                        return Err("Browser comment selection/anchor mismatch".into());
                    }
                    flag.set(true);
                    Ok(())
                }),
            })));
        self.comment();
        let started = std::time::Instant::now();
        while !captured.get() && started.elapsed() < std::time::Duration::from_secs(3) {
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        self.comments.replace(previous);
        if !captured.get() {
            return Err("Browser selection did not reach its native comment target".into());
        }
        let matches = Rc::new(Cell::new(0));
        let value = matches.clone();
        let finder = web.find_controller().ok_or("No page search controller")?;
        finder.connect_counted_matches(move |_, n| value.set(n));
        finder.count_matches("needle", 0, 1000);
        let start = std::time::Instant::now();
        while matches.get() == 0 && start.elapsed() < std::time::Duration::from_secs(3) {
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        if matches.get() != 2 {
            return Err("Native browser search did not find both exact matches".into());
        }
        let original = web.uri().unwrap().to_string();
        for blocked in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "studio-action://exit-browser",
            "http://192.168.0.1/",
        ] {
            self.navigate(blocked);
            if web.uri().as_deref() != Some(original.as_str()) {
                return Err("Blocked browser navigation changed the page".into());
            }
        }
        let second = url.trim_end_matches("/one").to_owned() + "/two";
        self.navigate(&second);
        wait(self, "/two").await?;
        self.back.emit_clicked();
        wait(self, "/one").await?;
        self.forward.emit_clicked();
        wait(self, "/two").await?;
        self.add_tab(Some(url.to_owned()));
        wait(self, "/one").await?;
        if self.tabs.n_pages() != 2 {
            return Err("Native browser second tab missing".into());
        }
        let second_page = self.tabs.selected_page().unwrap();
        let background = self.tabs.nth_page(0);
        self.tabs.set_selected_page(&background);
        if self.active().unwrap().uri().as_deref() != Some(second.as_str()) {
            return Err("Tab activation lost background page".into());
        }
        self.tabs.close_page(&second_page);
        if self.tabs.n_pages() != 1 {
            return Err("Background tab did not close".into());
        }
        let content = inspect(
            &web,
            "({storage:localStorage.getItem('native-test'),text:document.body.textContent})",
        )
        .await?;
        if content["storage"] != "private" {
            return Err("Browser navigation lost its private session".into());
        }
        web.set_zoom_level(1.2);
        if (web.zoom_level() - 1.2).abs() > 0.01 {
            return Err("Browser zoom did not change".into());
        }
        web.terminate_web_process();
        glib::timeout_future(std::time::Duration::from_millis(150)).await;
        let start = std::time::Instant::now();
        while !self.status.text().contains("Reload")
            && start.elapsed() < std::time::Duration::from_secs(3)
        {
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if !self.status.text().contains("Reload") {
            return Err("Browser process failure was not surfaced".into());
        }
        web.reload();
        wait(self, "/two").await?;
        capture("browser-tabs.png");
        drop(web);
        self.exit();
        if self.engine.borrow().is_some()
            || self.tabs.n_pages() != 0
            || !self.file_dialogs.borrow().is_empty()
        {
            return Err("Browser exit retained owned workspace state".into());
        }
        self.address.set_text(url);
        self.address.emit_activate();
        wait(self, "/one").await?;
        let web = self.active().unwrap();
        let content = inspect(&web, "localStorage.getItem('native-test')").await?;
        if !content.is_null() {
            return Err("Exited browser session persisted local storage".into());
        }
        drop(web);
        self.exit();
        Ok(
            serde_json::json!({"ok":true,"openMs":open_ms,"lazyEngine":true,"content":true,"isolatedStorage":true,"noStudioBridge":true,"urlPolicy":true,"history":true,"tabs":true,"search":true,"zoom":true,"selectionComment":true,"crashRecovery":true,"exitCleanup":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_comment(self: &Rc<Self>, url: &str) -> Result<(), String> {
        self.open(Some(url));
        let started = std::time::Instant::now();
        loop {
            if self
                .active()
                .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(url))
            {
                break;
            }
            if started.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "Comment browser did not open: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        let web = self.active().unwrap();
        let _ = web
            .snapshot_future(
                webkit6::SnapshotRegion::Visible,
                webkit6::SnapshotOptions::NONE,
            )
            .await;
        web.evaluate_javascript_future("(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('quote'));const s=getSelection();s.removeAllRanges();s.addRange(r)})()",Some(WORLD),None).await.map_err(|e|e.to_string())?;
        glib::timeout_future(std::time::Duration::from_millis(40)).await;
        // GTK menus can collapse the live selection. The isolated range survives.
        web.evaluate_javascript_future("getSelection().removeAllRanges()", Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        self.comment();
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_comment_reopened(&self, url: &str, quote: &str) -> Result<(), String> {
        let started = std::time::Instant::now();
        while !self
            .active()
            .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(url))
            || self.search.text() != quote
        {
            if started.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "Browser comment did not reopen with its quote: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if !self.search_row.is_visible() {
            return Err("Browser source search was hidden".into());
        }
        let web = self.active().ok_or("Browser source page disappeared")?;
        let finder = web
            .find_controller()
            .ok_or("Missing source search controller")?;
        let matches = Rc::new(Cell::new(None));
        let found = finder.connect_found_text({
            let matches = matches.clone();
            move |_, count| matches.set(Some(count))
        });
        let failed = finder.connect_failed_to_find_text({
            let matches = matches.clone();
            move |_| matches.set(Some(0))
        });
        self.find();
        while matches.get().is_none() && started.elapsed() < std::time::Duration::from_secs(25) {
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        finder.disconnect(found);
        finder.disconnect(failed);
        if matches.get().is_none_or(|count| count == 0) {
            return Err("Browser source quote was not found in the reopened page".into());
        }
        // A covered Wayland content surface can finish loading before painting.
        web.snapshot_future(
            webkit6::SnapshotRegion::Visible,
            webkit6::SnapshotOptions::NONE,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let widget = adw::ToolbarView::new();
        let header = adw::HeaderBar::builder()
            .show_title(true)
            .show_start_title_buttons(false)
            .show_end_title_buttons(false)
            .build();
        let back = button("go-previous-symbolic", "Back");
        let forward = button("go-next-symbolic", "Forward");
        let reload = button("view-refresh-symbolic", "Reload or stop loading");
        header.pack_start(&back);
        header.pack_start(&forward);
        header.pack_start(&reload);
        let address = gtk::Entry::builder()
            .placeholder_text("Enter an HTTP or HTTPS address")
            .hexpand(true)
            .width_chars(24)
            .max_length(8192)
            .build();
        address.set_accessible_role(gtk::AccessibleRole::TextBox);
        header.set_title_widget(Some(&address));
        let new = button("list-add-symbolic", "New tab");
        header.pack_end(&new);
        let hide = button("window-close-symbolic", "Hide browser workspace");
        header.pack_end(&hide);
        let popover = gtk::Popover::new();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 4);
        menu.set_margin_top(8);
        menu.set_margin_bottom(8);
        menu.set_margin_start(8);
        menu.set_margin_end(8);
        let mut actions = Vec::new();
        for (name, label) in [
            ("find", "Find in page"),
            ("zoom-in", "Zoom in"),
            ("zoom-out", "Zoom out"),
            ("zoom-reset", "Reset zoom"),
            ("copy", "Copy address"),
            ("external", "Open in system browser"),
            ("comment", "Comment on selection"),
            ("translate", "Translate selected text"),
            ("downloads", "Downloads…"),
            ("exit", "Exit browser and clear session"),
        ] {
            let b = gtk::Button::with_label(label);
            b.add_css_class("flat");
            b.set_halign(gtk::Align::Fill);
            menu.append(&b);
            actions.push((name, b));
        }
        popover.set_child(Some(&menu));
        header.pack_end(
            &gtk::MenuButton::builder()
                .icon_name("open-menu-symbolic")
                .popover(&popover)
                .build(),
        );
        widget.add_top_bar(&header);
        let tabs = adw::TabView::new();
        let bar = adw::TabBar::builder()
            .view(&tabs)
            .autohide(true)
            .expand_tabs(false)
            .build();
        widget.add_top_bar(&bar);
        let progress = gtk::ProgressBar::new();
        progress.set_visible(false);
        widget.add_top_bar(&progress);
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Find in page")
            .hexpand(true)
            .build();
        let find_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        find_row.set_margin_start(8);
        find_row.set_margin_end(8);
        find_row.append(&search);
        let previous = button("go-up-symbolic", "Previous match");
        let next = button("go-down-symbolic", "Next match");
        let close_find = button("window-close-symbolic", "Close search");
        find_row.append(&previous);
        find_row.append(&next);
        find_row.append(&close_find);
        find_row.set_visible(false);
        widget.add_top_bar(&find_row);
        widget.set_content(Some(&tabs));
        let status = gtk::Label::builder()
            .label("Browser is stopped")
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .margin_start(12)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .build();
        status.add_css_class("caption");
        widget.add_bottom_bar(&status);
        let this = Rc::new(Self {
            widget,
            parent: parent.downgrade(),
            sender,
            tabs,
            address,
            back,
            forward,
            reload,
            progress,
            status,
            search,
            engine: RefCell::new(None),
            preferences: RefCell::new(None),
            pending: RefCell::new(None),
            request: Cell::new(0),
            loading_preferences: Cell::new(false),
            exiting: Cell::new(false),
            tab_serial: Cell::new(0),
            tab_generation: Cell::new(0),
            tab_task: Cell::new(None),
            tab_revision: RefCell::new(None),
            tab_pending: RefCell::new(None),
            tab_saving: RefCell::new(None),
            tab_last: RefCell::new(None),
            tab_refresh: Cell::new(false),
            tab_muted: Cell::new(false),
            tab_error: Cell::new(false),
            tab_closing: Rc::new(Cell::new(false)),
            tab_abandon: Cell::new(false),
            tab_alert: RefCell::new(None),
            downloads: RefCell::new(Vec::new()),
            download_list: gtk::ListBox::new(),
            dialogs: RefCell::new(Vec::new()),
            file_dialogs: RefCell::new(Vec::new()),
            file_io: crate::file_io::Jobs::new(parent),
            on_hide: RefCell::new(None),
            on_open: RefCell::new(None),
            comments: RefCell::new(None),
            epochs: RefCell::new(std::collections::HashMap::new()),
            tab_targets: RefCell::new(std::collections::HashMap::new()),
            quote: RefCell::new(None),
            search_row: find_row.clone(),
        });
        let weak = Rc::downgrade(&this);
        this.address.connect_activate(move |entry| {
            if let Some(b) = weak.upgrade() {
                b.navigate(&entry.text());
            }
        });
        let weak = Rc::downgrade(&this);
        this.back.connect_clicked(move |_| {
            if let Some(web) = weak.upgrade().and_then(|b| b.active()) {
                web.go_back();
            }
        });
        let weak = Rc::downgrade(&this);
        this.forward.connect_clicked(move |_| {
            if let Some(web) = weak.upgrade().and_then(|b| b.active()) {
                web.go_forward();
            }
        });
        let weak = Rc::downgrade(&this);
        this.reload.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                if let Some(web) = b.active() {
                    if web.is_loading() {
                        web.stop_loading();
                    } else {
                        web.reload();
                    }
                }
            }
        });
        let weak = Rc::downgrade(&this);
        new.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.add_tab(None);
                b.address.grab_focus();
            }
        });
        let weak = Rc::downgrade(&this);
        hide.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                if let Some(f) = b.on_hide.borrow().as_ref() {
                    f();
                }
            }
        });
        let weak = Rc::downgrade(&this);
        this.tabs.connect_selected_page_notify(move |_| {
            if let Some(b) = weak.upgrade() {
                b.sync();
                b.remember_tabs();
                b.find();
                if let Some(web) = b.active().filter(|web| !web.is_loading()) {
                    b.source_loaded(&web);
                }
            }
        });
        let weak = Rc::downgrade(&this);
        this.tabs.connect_page_reordered(move |_, _, _| {
            if let Some(b) = weak.upgrade() {
                b.remember_tabs();
            }
        });
        let weak = Rc::downgrade(&this);
        this.tabs.connect_close_page(move |tabs, page| {
            if let Some(b) = weak.upgrade() {
                if let Ok(web) = page.child().downcast::<webkit6::WebView>() {
                    b.cancel_page_file_dialogs(&web);
                    b.epochs.borrow_mut().remove(&(web.as_ptr() as usize));
                    b.tab_targets.borrow_mut().remove(&(web.as_ptr() as usize));
                    web.stop_loading();
                }
                tabs.close_page_finish(page, true);
                b.remember_tabs();
                if tabs.n_pages() == 0 && !b.exiting.get() {
                    b.exit();
                }
            }
            glib::Propagation::Stop
        });
        let weak = Rc::downgrade(&this);
        this.search.connect_search_changed(move |_| {
            if let Some(b) = weak.upgrade() {
                b.find();
            }
        });
        let weak = Rc::downgrade(&this);
        previous.connect_clicked(move |_| {
            if let Some(web) = weak.upgrade().and_then(|b| b.active()) {
                if let Some(c) = web.find_controller() {
                    c.search_previous();
                }
            }
        });
        let weak = Rc::downgrade(&this);
        next.connect_clicked(move |_| {
            if let Some(web) = weak.upgrade().and_then(|b| b.active()) {
                if let Some(c) = web.find_controller() {
                    c.search_next();
                }
            }
        });
        let weak = Rc::downgrade(&this);
        let row = find_row.clone();
        close_find.connect_clicked(move |_| {
            row.set_visible(false);
            if let Some(b) = weak.upgrade() {
                if let Some(c) = b.active().and_then(|w| w.find_controller()) {
                    c.search_finish();
                }
            }
        });
        for (name, action) in actions {
            let weak = Rc::downgrade(&this);
            let popover = popover.clone();
            let find_row = find_row.clone();
            action.connect_clicked(move |_| {
                popover.popdown();
                if let Some(b) = weak.upgrade() {
                    match name {
                        "find" => {
                            find_row.set_visible(true);
                            b.search.grab_focus();
                        }
                        "zoom-in" | "zoom-out" | "zoom-reset" => {
                            if let Some(web) = b.active() {
                                web.set_zoom_level(if name == "zoom-reset" {
                                    1.0
                                } else {
                                    (web.zoom_level() + if name == "zoom-in" { 0.1 } else { -0.1 })
                                        .clamp(0.5, 2.0)
                                });
                            }
                        }
                        "copy" => {
                            if let Some(uri) = b.active().and_then(|w| w.uri()) {
                                b.address.clipboard().set_text(&uri);
                            }
                        }
                        "external" => {
                            if let Some(uri) = b.active().and_then(|w| w.uri()) {
                                if b.allowed(&uri).is_ok() {
                                    if let Err(e) = gio::AppInfo::launch_default_for_uri(
                                        &uri,
                                        gio::AppLaunchContext::NONE,
                                    ) {
                                        b.error(&e.to_string());
                                    }
                                }
                            }
                        }
                        "comment" => b.comment(),
                        "translate" => b.translate_selection(),
                        "downloads" => b.show_downloads(),
                        "exit" => b.exit(),
                        _ => {}
                    }
                }
            });
        }
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&this);
        key.connect_key_pressed(move |_, key, _, modifiers| {
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !modifiers.intersects(
                    gtk::gdk::ModifierType::ALT_MASK | gtk::gdk::ModifierType::SUPER_MASK,
                )
            {
                if let Some(b) = weak.upgrade() {
                    match key {
                        gtk::gdk::Key::l | gtk::gdk::Key::L => {
                            b.address.grab_focus();
                            b.address.select_region(0, -1);
                        }
                        gtk::gdk::Key::f | gtk::gdk::Key::F => {
                            find_row.set_visible(true);
                            b.search.grab_focus();
                        }
                        gtk::gdk::Key::t | gtk::gdk::Key::T => {
                            b.add_tab(None);
                        }
                        gtk::gdk::Key::w | gtk::gdk::Key::W => {
                            if let Some(p) = b.tabs.selected_page() {
                                b.tabs.close_page(&p);
                            }
                        }
                        _ => return glib::Propagation::Proceed,
                    }
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        this.widget.add_controller(key);
        this.sync();
        this
    }
    pub fn open_comment(self: &Rc<Self>, url: &str, excerpt: &str) -> Result<(), String> {
        if self.preferences.borrow().is_some() {
            self.allowed(url)?;
        }
        self.quote.replace(Some((url.into(), excerpt.into(), None)));
        self.open(Some(url));
        Ok(())
    }
    fn bind_quote(&self, web: &webkit6::WebView, url: &str) {
        let mut target = self.quote.borrow_mut();
        if let Some((expected, _, binding)) = target.as_mut() {
            if expected == url {
                let epoch = self
                    .epochs
                    .borrow()
                    .get(&(web.as_ptr() as usize))
                    .map_or(0, |e| e.get());
                *binding = Some((web.downgrade(), epoch + 1));
            }
        }
    }
    fn source_loaded(&self, web: &webkit6::WebView) {
        if self.active().as_ref() != Some(web) {
            return;
        }
        let quote = {
            let mut target = self.quote.borrow_mut();
            let matching = target.as_ref().is_some_and(|(_, _, binding)| {
                binding.as_ref().is_some_and(|(page, epoch)| {
                    page.upgrade().as_ref() == Some(web)
                        && self
                            .epochs
                            .borrow()
                            .get(&(web.as_ptr() as usize))
                            .is_some_and(|e| e.get() == *epoch)
                })
            });
            if matching {
                target.take().map(|(_, quote, _)| quote)
            } else {
                None
            }
        };
        if let Some(quote) = quote {
            self.search_row.set_visible(true);
            self.search.set_text(&quote);
            self.find();
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_tabs_idle(&self) -> bool {
        self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_preferences_ready(&self) -> bool {
        !self.loading_preferences.get()
    }
    pub fn open(self: &Rc<Self>, url: Option<&str>) {
        if self.tab_closing.get() {
            return;
        }
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
        if self.engine.borrow().is_some() {
            if let Some(url) = url {
                self.navigate(url);
            }
            if self.tab_error.replace(false) {
                self.tab_revision.borrow_mut().take();
                self.tab_refresh.set(true);
                self.pump_tabs();
            }
            return;
        }
        self.pending.replace(Some(url.unwrap_or("").to_owned()));
        self.request.set(self.request.get().wrapping_add(1));
        self.loading_preferences.set(true);
        if self.tab_error.replace(false) {
            self.tab_revision.borrow_mut().take();
        }
        self.status.set_text("Loading browser preferences…");
        self.pump_tabs();
    }
    /// Capture only browser-owned addresses. No DOM, history, cookies or titles
    /// enter the metadata file; blanks and policy-blocked addresses are omitted.
    fn current_tabs(&self) -> crate::browser_tabs::Tabs {
        let mut selection = crate::browser_tabs::Tabs::default();
        let active = self.tabs.selected_page();
        for i in 0..self.tabs.n_pages() {
            let page = self.tabs.nth_page(i);
            let Ok(web) = page.child().downcast::<webkit6::WebView>() else {
                continue;
            };
            let uri = web
                .uri()
                .filter(|u| u.as_str() != "about:blank")
                .map(|u| u.to_string());
            let Some(url) = uri.or_else(|| {
                self.tab_targets
                    .borrow()
                    .get(&(web.as_ptr() as usize))
                    .cloned()
            }) else {
                continue;
            };
            if url.len() > 8192 {
                continue;
            }
            if let Ok(url) = self.allowed(&url) {
                if active.as_ref() == Some(&page) {
                    selection.active = selection.tabs.len();
                }
                selection.tabs.push(url);
            }
        }
        selection
    }
    fn remember_tabs(&self) {
        if self.tab_muted.get()
            || self.engine.borrow().is_none()
            || self.exiting.get()
            || self.tab_abandon.get()
            || !self
                .preferences
                .borrow()
                .as_ref()
                .is_some_and(|p| p.restore_tabs)
        {
            return;
        }
        let tabs = self.current_tabs();
        if self.tab_last.borrow().as_ref() == Some(&tabs) {
            return;
        }
        self.tab_last.replace(Some(tabs.clone()));
        self.tab_pending.replace(Some(tabs));
        self.pump_tabs();
    }
    /// One in-flight request and one replaceable latest snapshot: URI changes
    /// and tab selection cannot build an unbounded queue or reorder Exit saves.
    fn pump_tabs(&self) {
        if self.tab_task.get().is_some() || self.tab_error.get() || self.tab_abandon.get() {
            return;
        }
        let generation = self.tab_generation.get();
        let save = self
            .tab_pending
            .borrow()
            .clone()
            .zip(self.tab_revision.borrow().clone());
        let (task, request) = if let Some((selection, revision)) = save {
            self.tab_pending.borrow_mut().take();
            self.tab_saving.replace(Some(selection.clone()));
            (
                TabTask::Save { generation },
                crate::browser_tabs::Request::Save {
                    revision,
                    selection,
                },
            )
        } else if self.loading_preferences.get()
            || self.tab_refresh.get()
            || self.tab_pending.borrow().is_some()
        {
            self.tab_refresh.set(false);
            (
                TabTask::Read {
                    generation,
                    open: self.loading_preferences.get().then(|| self.request.get()),
                },
                crate::browser_tabs::Request::Read,
            )
        } else {
            self.finish_tab_close();
            return;
        };
        let serial = self.tab_serial.get().wrapping_add(1);
        self.tab_serial.set(serial);
        self.tab_task.set(Some(task));
        if self
            .sender
            .try_send(Command::BrowserTabs { serial, request })
            .is_err()
        {
            self.tabs_event_failed(task, "Browser tab queue is unavailable");
        }
    }
    fn tabs_event_failed(&self, task: TabTask, error: &str) {
        self.tab_task.set(None);
        if matches!(task, TabTask::Save { .. }) {
            let saving = self.tab_saving.borrow_mut().take();
            if self.tab_pending.borrow().is_none() {
                self.tab_pending.replace(saving);
            }
        }
        self.tab_error.set(true);
        self.loading_preferences.set(false);
        self.pending.borrow_mut().take();
        self.error(error);
        if self.tab_closing.get() {
            if let Some(parent) = self.parent.upgrade() {
                let closing = self.tab_closing.clone();
                glib::idle_add_local_once(move || {
                    if closing.get() {
                        parent.close();
                    }
                });
            }
        }
    }
    fn finish_tab_close(&self) {
        if self.tab_closing.get()
            && self.tab_task.get().is_none()
            && self.tab_pending.borrow().is_none()
        {
            if let Some(parent) = self.parent.upgrade() {
                let closing = self.tab_closing.clone();
                glib::idle_add_local_once(move || {
                    if closing.get() {
                        parent.close();
                    }
                });
            }
        }
    }
    pub fn tabs_event(
        self: &Rc<Self>,
        serial: u64,
        result: Result<crate::browser_tabs::Snapshot, String>,
    ) {
        if serial != self.tab_serial.get() {
            return;
        }
        let Some(task) = self.tab_task.take() else {
            return;
        };
        if task.generation() != self.tab_generation.get() {
            let saved = self.tab_saving.borrow_mut().take();
            if self.tab_pending.borrow().is_none()
                && self
                    .preferences
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.restore_tabs)
            {
                self.tab_pending.replace(saved);
            }
            self.tab_revision.borrow_mut().take();
            self.pump_tabs();
            return;
        }
        let snapshot = match result {
            Ok(v) => v,
            Err(e) => {
                self.tabs_event_failed(task, &e);
                return;
            }
        };
        self.tab_revision.replace(Some(snapshot.revision));
        match task {
            TabTask::Save { .. } => {
                self.tab_saving.borrow_mut().take();
            }
            TabTask::Read { open, .. } => {
                // Read policy and addresses in one backend transaction. Settings
                // callbacks invalidate the generation before a stale read lands.
                let p = snapshot.preferences;
                let values = crate::browser_policy::NavigationPreferences::from_browser(&p);
                self.apply_policy(values, p.restore_tabs, false);
                self.preferences.replace(Some(p.clone()));
                if !p.restore_tabs {
                    self.tab_pending.borrow_mut().take();
                    self.tab_last.borrow_mut().take();
                } else {
                    let pending = self.tab_pending.borrow_mut().take();
                    if let Some(pending) = pending {
                        self.tab_pending.replace(Some(pending.permitted(&p)));
                    }
                }
                if open == Some(self.request.get()) && self.loading_preferences.replace(false) {
                    let pending = self.pending.borrow_mut().take();
                    if p.enabled {
                        self.tab_muted.set(true);
                        let saved = self
                            .tab_pending
                            .borrow()
                            .clone()
                            .unwrap_or(snapshot.selection);
                        if pending.as_deref() == Some("")
                            && p.restore_tabs
                            && !saved.tabs.is_empty()
                        {
                            for url in saved.tabs {
                                self.add_tab(Some(url));
                            }
                            if self.tabs.n_pages() > 0 {
                                self.tabs.set_selected_page(&self.tabs.nth_page(
                                    saved.active.min(self.tabs.n_pages() as usize - 1) as i32,
                                ));
                            }
                        } else {
                            self.add_tab(pending);
                        }
                        self.tab_muted.set(false);
                        self.remember_tabs();
                    } else {
                        self.error("Browser is disabled in Studio preferences");
                    }
                }
            }
        }
        self.pump_tabs();
    }
    fn allowed(&self, input: &str) -> Result<String, String> {
        let preferences = self.preferences.borrow();
        let p = preferences
            .as_ref()
            .ok_or("Browser preferences have not loaded")?;
        validate_browser_url(input, p)
            .map(|u| u.to_string())
            .map_err(|e| e.to_string())
    }
    fn active(&self) -> Option<webkit6::WebView> {
        self.tabs.selected_page()?.child().downcast().ok()
    }
    fn page_for(&self, web: &webkit6::WebView) -> Option<adw::TabPage> {
        (0..self.tabs.n_pages())
            .map(|i| self.tabs.nth_page(i))
            .find(|page| page.child() == *web.upcast_ref::<gtk::Widget>())
    }
    fn add_tab(self: &Rc<Self>, url: Option<String>) {
        if self.preferences.borrow().is_none() {
            self.open(url.as_deref());
            return;
        }
        if self.tabs.n_pages() >= MAX_TABS {
            self.error("Close a tab before opening another (maximum 8)");
            return;
        }
        let url = match url.filter(|s| !s.is_empty()) {
            Some(s) => match self.allowed(&s) {
                Ok(s) => Some(s),
                Err(e) => {
                    self.error(&e);
                    return;
                }
            },
            None => None,
        };
        if self.engine.borrow().is_none() {
            let context = webkit6::WebContext::new();
            let session = webkit6::NetworkSession::new_ephemeral();
            session.set_tls_errors_policy(webkit6::TLSErrorsPolicy::Fail);
            let weak = Rc::downgrade(self);
            session.connect_download_started(move |_, d| {
                if let Some(b) = weak.upgrade() {
                    b.download(d);
                } else {
                    d.cancel();
                }
            });
            self.engine.replace(Some(Engine { context, session }));
        }
        let manager = webkit6::UserContentManager::new();
        manager.add_script(&webkit6::UserScript::for_world(
            include_str!("browser_selection.js"),
            webkit6::UserContentInjectedFrames::TopFrame,
            webkit6::UserScriptInjectionTime::End,
            WORLD,
            &[],
            &[],
        ));
        let web = {
            let engine = self.engine.borrow();
            let e = engine.as_ref().unwrap();
            webkit6::WebView::builder()
                .web_context(&e.context)
                .user_content_manager(&manager)
                .network_session(&e.session)
                .settings(
                    &webkit6::Settings::builder()
                        .enable_javascript(
                            self.preferences
                                .borrow()
                                .as_ref()
                                .unwrap()
                                .preview_javascript,
                        )
                        .allow_file_access_from_file_urls(false)
                        .allow_universal_access_from_file_urls(false)
                        .allow_top_navigation_to_data_urls(false)
                        .javascript_can_access_clipboard(false)
                        .javascript_can_open_windows_automatically(false)
                        .enable_media_stream(false)
                        .enable_webrtc(false)
                        .enable_developer_extras(false)
                        .build(),
                )
                .build()
        };
        let epoch = Rc::new(Cell::new(0));
        self.epochs
            .borrow_mut()
            .insert(web.as_ptr() as usize, epoch.clone());
        let page = self.tabs.append(&web);
        page.set_title("New Tab");
        self.tabs.set_selected_page(&page);
        let weak = Rc::downgrade(self);
        web.connect_decide_policy(move |web, decision, kind| {
            let Some(b) = weak.upgrade() else {
                decision.ignore();
                return true;
            };
            if matches!(
                kind,
                webkit6::PolicyDecisionType::NavigationAction
                    | webkit6::PolicyDecisionType::NewWindowAction
            ) {
                let action = decision
                    .downcast_ref::<webkit6::NavigationPolicyDecision>()
                    .and_then(|d| d.navigation_action());
                let uri = action
                    .as_ref()
                    .and_then(|a| a.request())
                    .and_then(|r| r.uri());
                if let Some(uri) = uri {
                    match b.allowed(&uri) {
                        Ok(_) => {
                            if kind == webkit6::PolicyDecisionType::NewWindowAction {
                                decision.ignore();
                                if action.is_some_and(|a| a.is_user_gesture()) {
                                    let weak = Rc::downgrade(&b);
                                    glib::idle_add_local_once(move || {
                                        if let Some(b) = weak.upgrade() {
                                            b.add_tab(Some(uri.to_string()));
                                        }
                                    });
                                }
                                return true;
                            }
                            decision.use_();
                            return true;
                        }
                        Err(e) => {
                            decision.ignore();
                            b.error(&e);
                            return true;
                        }
                    }
                }
                decision.ignore();
                return true;
            }
            if kind == webkit6::PolicyDecisionType::Response {
                if let Some(response) = decision.downcast_ref::<webkit6::ResponsePolicyDecision>() {
                    if !response.is_mime_type_supported() && response.is_main_frame_main_resource()
                    {
                        decision.download();
                        return true;
                    }
                }
            }
            let _ = web;
            false
        });
        web.connect_create(|_, _| None);
        let weak = Rc::downgrade(self);
        web.connect_permission_request(move |_, request| {
            request.deny();
            if let Some(b) = weak.upgrade() {
                b.error("Page permission denied");
            }
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_run_file_chooser(move |web, request| {
            if let Some(b) = weak.upgrade() {
                b.upload(web, request);
            } else {
                request.cancel();
            }
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_script_dialog(move |_, dialog| {
            dialog.confirm_set_confirmed(false);
            if let Some(b) = weak.upgrade() {
                b.error("Page dialog dismissed");
            }
            true
        });
        web.connect_enter_fullscreen(|_| true);
        let weak = Rc::downgrade(self);
        web.connect_load_failed_with_tls_errors(move |_, _, _, _| {
            if let Some(b) = weak.upgrade() {
                b.error("TLS validation failed; this page was not opened");
            }
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_load_failed(move |_, _, _, error| {
            if !error.matches(webkit6::NetworkError::Cancelled) {
                if let Some(b) = weak.upgrade() {
                    b.error(&format!("Page could not load: {error}"));
                }
            }
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_web_process_terminated(move |web, _| {
            if let Some(b) = weak.upgrade() {
                b.cancel_page_file_dialogs(web);
                {
                    let Some(page) = b.page_for(web) else {
                        return;
                    };
                    page.set_indicator_icon(Some(&gio::ThemedIcon::new("dialog-warning-symbolic")));
                    page.set_tooltip("Page process stopped; reload to recover");
                }
                b.error("Page process stopped. Reload to recover.");
            }
        });
        let weak = Rc::downgrade(self);
        web.connect_load_changed(move |web, event| {
            if event == webkit6::LoadEvent::Started {
                epoch.set(epoch.get() + 1);
            }

            if let Some(b) = weak.upgrade() {
                if event == webkit6::LoadEvent::Started {
                    b.cancel_page_file_dialogs(web);
                }
                {
                    let Some(page) = b.page_for(web) else {
                        return;
                    };
                    page.set_loading(web.is_loading());
                    if event == webkit6::LoadEvent::Committed {
                        page.set_indicator_icon(gio::Icon::NONE);
                    }
                }
                b.sync();
                if event == webkit6::LoadEvent::Committed {
                    b.remember_tabs();
                }
                if event == webkit6::LoadEvent::Finished {
                    b.source_loaded(web);
                }
            }
        });
        let weak = Rc::downgrade(self);
        web.connect_title_notify(move |web| {
            if let Some(b) = weak.upgrade() {
                {
                    let Some(page) = b.page_for(web) else {
                        return;
                    };
                    page.set_title(
                        &web.title()
                            .map(|s| s.chars().take(256).collect::<String>())
                            .unwrap_or_else(|| "New Tab".into()),
                    );
                }
                b.sync();
            }
        });
        for signal in [
            "notify::uri",
            "notify::estimated-load-progress",
            "notify::is-loading",
        ] {
            let weak = Rc::downgrade(self);
            let address_changed = signal == "notify::uri";
            web.connect_local(signal, false, move |_| {
                if let Some(b) = weak.upgrade() {
                    b.sync();
                    if address_changed {
                        b.remember_tabs();
                    }
                }
                None
            });
        }
        if let Some(url) = url {
            self.tab_targets
                .borrow_mut()
                .insert(web.as_ptr() as usize, url.clone());
            self.bind_quote(&web, &url);
            web.load_uri(&url);
        } else {
            self.address.set_text("");
            self.address.grab_focus();
            self.status.set_text("Enter an address to start browsing");
        }
    }
    fn navigate(self: &Rc<Self>, input: &str) {
        if self.preferences.borrow().is_none() {
            self.open(Some(input));
            return;
        }
        match self.allowed(input) {
            Ok(uri) => {
                if let Some(web) = self.active() {
                    self.tab_targets
                        .borrow_mut()
                        .insert(web.as_ptr() as usize, uri.clone());
                    self.bind_quote(&web, &uri);
                    web.load_uri(&uri);
                } else {
                    self.add_tab(Some(uri));
                }
            }
            Err(e) => self.error(&e),
        }
    }
    fn sync(&self) {
        let web = self.active();
        self.back
            .set_sensitive(web.as_ref().is_some_and(|w| w.can_go_back()));
        self.forward
            .set_sensitive(web.as_ref().is_some_and(|w| w.can_go_forward()));
        self.reload.set_sensitive(web.is_some());
        if let Some(w) = web {
            if !self.address.has_focus() {
                self.address.set_text(w.uri().as_deref().unwrap_or(""));
            }
            self.progress.set_visible(w.is_loading());
            self.progress.set_fraction(w.estimated_load_progress());
            if self.tabs.page(&w).indicator_icon().is_some() {
                self.status
                    .set_text("Page process stopped. Reload to recover.");
            } else if !w.is_loading() {
                self.status
                    .set_text(w.uri().as_deref().unwrap_or("New tab · private session"));
            }
        } else {
            self.address.set_text("");
            self.progress.set_visible(false);
        }
    }
    fn find(&self) {
        if let Some(controller) = self.active().and_then(|w| w.find_controller()) {
            let query = self.search.text();
            if query.is_empty() {
                controller.search_finish();
            } else {
                controller.search(
                    &query,
                    (webkit6::FindOptions::CASE_INSENSITIVE | webkit6::FindOptions::WRAP_AROUND)
                        .bits(),
                    1000,
                );
            }
        }
    }
    fn error(&self, message: &str) {
        self.status.set_text(message);
    }
    pub fn apply_navigation(
        self: &Rc<Self>,
        values: crate::browser_policy::NavigationPreferences,
        restore: bool,
    ) {
        self.apply_policy(values, restore, true);
    }
    fn apply_policy(
        self: &Rc<Self>,
        values: crate::browser_policy::NavigationPreferences,
        restore: bool,
        refresh: bool,
    ) {
        if refresh {
            self.tab_generation
                .set(self.tab_generation.get().wrapping_add(1));
            self.tab_revision.borrow_mut().take();
            self.tab_error.set(false);
        }
        let previous = self.preferences.borrow().clone().unwrap_or_default();
        let mut next = previous.clone();
        values.apply(&mut next);
        next.restore_tabs = restore;
        self.preferences.replace(Some(next.clone()));
        if !restore {
            self.tab_refresh.set(false);
            self.tab_pending.borrow_mut().take();
            self.tab_last.borrow_mut().take();
        }
        if !values.enabled {
            self.exit();
            self.status
                .set_text("Browser is disabled in Studio preferences");
            return;
        }
        self.tab_muted.set(true);
        let tabs = (0..self.tabs.n_pages())
            .filter_map(|index| {
                let page = self.tabs.nth_page(index);
                let web = page.child().downcast::<webkit6::WebView>().ok()?;
                Some((page, web))
            })
            .collect::<Vec<_>>();
        let tightened = previous.allow_http && !next.allow_http
            || previous.allow_localhost && !next.allow_localhost
            || previous.allow_private_network && !next.allow_private_network;
        let cancelled_destinations =
            if tightened || previous.preview_javascript != next.preview_javascript {
                self.cancel_file_dialogs(|_| true)
            } else {
                Vec::new()
            };
        let downloads = self
            .downloads
            .borrow()
            .iter()
            .filter(|transfer| !transfer.complete.get() && !transfer.network_done.get())
            .map(|transfer| {
                (
                    transfer.download.clone(),
                    transfer.destination_pending.get(),
                    transfer.stop.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (download, destination_pending, stop) in downloads {
            if cancelled_destinations.contains(&download)
                || ((tightened || previous.preview_javascript != next.preview_javascript)
                    && destination_pending)
                || download
                    .request()
                    .and_then(|request| request.uri())
                    .is_some_and(|uri| validate_browser_url(&uri, &next).is_err())
            {
                stop();
            }
        }
        let mut closed = 0;
        for (page, web) in tabs {
            if tightened {
                web.stop_loading();
            }
            if web
                .uri()
                .filter(|uri| uri.as_str() != "about:blank")
                .is_some_and(|uri| validate_browser_url(&uri, &next).is_err())
            {
                self.tabs.close_page(&page);
                closed += 1;
                continue;
            }
            if let Some(settings) = webkit6::prelude::WebViewExt::settings(&web) {
                settings.set_enable_javascript(next.preview_javascript);
            }
            if previous.preview_javascript != next.preview_javascript && web.uri().is_some() {
                web.reload();
            }
        }
        self.tab_muted.set(false);
        self.sync();
        if refresh
            && restore
            && (self.engine.borrow().is_some() || self.tab_pending.borrow().is_some())
        {
            self.tab_refresh.set(true);
            self.tab_last.borrow_mut().take();
            self.remember_tabs();
            self.pump_tabs();
        }
        if closed > 0 {
            self.status
                .set_text("Pages blocked by browser settings were closed");
        }
    }
    pub fn exit(&self) {
        if self.engine.borrow().is_some() {
            self.remember_tabs();
        }
        if self.exiting.replace(true) {
            return;
        }
        self.request.set(self.request.get().wrapping_add(1));
        self.loading_preferences.set(false);
        self.pending.borrow_mut().take();
        self.quote.borrow_mut().take();
        self.preferences.borrow_mut().take();
        let _ = self.cancel_file_dialogs(|_| true);
        let transfers = std::mem::take(&mut *self.downloads.borrow_mut());
        for transfer in transfers {
            (transfer.stop)();
            if let Some(target) = transfer.target.borrow_mut().take() {
                self.file_io.discard(target);
            }
            self.download_list.remove(&transfer.row);
        }
        let dialogs = std::mem::take(&mut *self.dialogs.borrow_mut());
        for dialog in dialogs {
            dialog.force_close();
        }
        let engine = self.engine.borrow_mut().take();
        while self.tabs.n_pages() > 0 {
            let page = self.tabs.nth_page(0);
            if let Ok(web) = page.child().downcast::<webkit6::WebView>() {
                web.stop_loading();
                web.terminate_web_process();
            }
            self.tabs.close_page(&page);
        }
        drop(engine);
        self.sync();
        self.status
            .set_text("Browser stopped · private session cleared");
        self.exiting.set(false);
    }
    pub fn allow_window_close(self: &Rc<Self>) -> bool {
        self.tab_closing.set(true);
        // Do not destroy live page/form state before a failed save can offer
        // Cancel. Invalidate an unfinished Open while flushing existing tabs.
        self.request.set(self.request.get().wrapping_add(1));
        self.loading_preferences.set(false);
        self.pending.borrow_mut().take();
        if self.engine.borrow().is_some() {
            self.remember_tabs();
        }
        if self.tab_error.get() && self.tab_pending.borrow().is_some() && !self.tab_abandon.get() {
            if self.tab_alert.borrow().is_none() {
                let dialog = adw::AlertDialog::builder()
                    .heading(crate::language::tr("Browser tabs could not be saved"))
                    .body(crate::language::tr(
                        "Retry saving your browser addresses, or close without saving them.",
                    ))
                    .build();
                dialog.add_responses(&[
                    ("cancel", &crate::language::tr("Cancel")),
                    ("discard", &crate::language::tr("Close without saving")),
                    ("retry", &crate::language::tr("Retry")),
                ]);
                dialog.set_default_response(Some("retry"));
                dialog.set_close_response("cancel");
                dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                let weak = Rc::downgrade(self);
                dialog.connect_response(None, move |_, response| {
                    if let Some(b) = weak.upgrade() {
                        b.tab_alert.borrow_mut().take();
                        match response {
                            "discard" => {
                                b.tab_abandon.set(true);
                                b.tab_pending.borrow_mut().take();
                                b.finish_tab_close();
                            }
                            "retry" => {
                                b.tab_error.set(false);
                                b.tab_revision.borrow_mut().take();
                                b.tab_refresh.set(true);
                                b.pump_tabs();
                            }
                            _ => {
                                b.tab_closing.set(false);
                                b.file_io.cancel_close();
                            }
                        }
                    }
                });
                if let Some(parent) = self.parent.upgrade() {
                    dialog.present(Some(&parent));
                }
                self.tab_alert.replace(Some(dialog));
            }
            return false;
        }
        let tabs_ready = self.tab_abandon.get()
            || (self.tab_task.get().is_none() && self.tab_pending.borrow().is_none());
        if !tabs_ready {
            self.error(&crate::language::tr("Saving browser tabs before closing…"));
            return false;
        }
        self.exit();
        let ready = self.file_io.allow_close();
        if !ready {
            self.error(&crate::language::tr(
                "Waiting for browser downloads to finish closing…",
            ));
        }
        ready
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_translation(self: &Rc<Self>, url: &str) -> Result<(), String> {
        self.open(Some(url));
        let start = std::time::Instant::now();
        loop {
            if self
                .active()
                .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(url))
            {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(25) {
                return Err(format!(
                    "Translation browser timed out: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        let web = self.active().ok_or("Missing translation browser page")?;
        web.evaluate_javascript_future("(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('quote'));const s=getSelection();s.removeAllRanges();s.addRange(r)})()",Some(WORLD),None).await.map_err(|e|e.to_string())?;
        glib::timeout_future(std::time::Duration::from_millis(50)).await;
        web.evaluate_javascript_future("getSelection().removeAllRanges()", Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        self.translate_selection();
        Ok(())
    }
    fn translate_selection(self: &Rc<Self>) {
        let Some(action) = crate::translation_ui::capture() else {
            self.error("Select a session before translating");
            return;
        };
        let Some(web) = self.active().filter(|w| !w.is_loading()) else {
            return;
        };
        let uri = web.uri();
        if uri.as_deref().is_none_or(|uri| self.allowed(uri).is_err()) {
            return;
        }
        let Some(epoch) = self.epochs.borrow().get(&(web.as_ptr() as usize)).cloned() else {
            return;
        };
        let generation = epoch.get();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result = web
                .evaluate_javascript_future("StudioBrowserSelection.text()", Some(WORLD), None)
                .await;
            let Some(browser) = weak.upgrade() else {
                return;
            };
            if browser.active().as_ref() != Some(&web)
                || web.uri() != uri
                || epoch.get() != generation
                || web.is_loading()
            {
                return;
            }
            match result {
                Ok(value) if value.is_string() => action(value.to_str().to_string()),
                Ok(_) => browser.error("Select a smaller passage to translate"),
                Err(error) => browser.error(&error.to_string()),
            }
        });
    }
    fn comment(self: &Rc<Self>) {
        let Some(target) = self.comments.borrow().clone() else {
            return;
        };
        let Some(owner) = (target.owner)() else {
            self.error("Select a session before commenting");
            return;
        };
        let Some(web) = self.active().filter(|web| !web.is_loading()) else {
            return;
        };
        let uri = web.uri().map(|s| s.to_string()).unwrap_or_default();
        if let Err(error) = self.allowed(&uri) {
            self.error(&error);
            return;
        }
        let epoch = self
            .epochs
            .borrow()
            .get(&(web.as_ptr() as usize))
            .cloned()
            .unwrap_or_else(|| Rc::new(Cell::new(0)));
        let generation = epoch.get();
        let title = self
            .page_for(&web)
            .map(|p| crate::annotations::bounded(&p.title(), 512))
            .unwrap_or_default();
        let weak = Rc::downgrade(self);
        let page = web.downgrade();
        glib::spawn_future_local(async move {
            let result = web
                .evaluate_javascript_future("StudioBrowserSelection.text()", Some(WORLD), None)
                .await;
            let Some(browser) = weak.upgrade() else {
                return;
            };
            if browser.active().as_ref() != page.upgrade().as_ref()
                || page.upgrade().and_then(|p| p.uri()).as_deref() != Some(uri.as_str())
                || epoch.get() != generation
                || page.upgrade().is_none_or(|p| p.is_loading())
            {
                return;
            }
            let result=result.map_err(|e|e.to_string()).and_then(|value|{if value.is_null(){return Err("Select a passage of at most 16 KiB to comment".into());}
                crate::annotations::Capture{excerpt:value.to_str().to_string(),source:serde_json::json!({"provider":"browser","version":1,"anchor":{"url":uri,"title":title,"targetId":null,"selector":null}})}.validate()
            }).and_then(|capture|(target.capture)(owner,capture));
            match result {
                Ok(()) => browser.status.set_text("Comment added"),
                Err(error) => browser.error(&error),
            }
        });
    }
    fn upload(self: &Rc<Self>, web: &webkit6::WebView, request: &webkit6::FileChooserRequest) {
        let Some(parent) = self.parent.upgrade() else {
            request.cancel();
            return;
        };
        let dialog = gtk::FileDialog::builder()
            .title(format!(
                "Choose files for {}",
                web.uri()
                    .and_then(|s| url::Url::parse(&s).ok())
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_default()
            ))
            .modal(true)
            .build();
        if let Some(filter) = request.mime_types_filter() {
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            dialog.set_default_filter(Some(&filter));
        }
        let Some(epoch) = self
            .epochs
            .borrow()
            .get(&(web.as_ptr() as usize))
            .map(|e| e.get())
        else {
            request.cancel();
            return;
        };
        if web.is_loading() || web.uri().is_none_or(|uri| self.allowed(&uri).is_err()) {
            request.cancel();
            return;
        }
        let Some(cancellable) = self.start_file_dialog(FileOwner::Upload {
            page: web.downgrade(),
        }) else {
            request.cancel();
            return;
        };
        let multiple = request.selects_multiple();
        let request = request.clone();
        let weak = Rc::downgrade(self);
        let web = web.downgrade();
        let uri = web.upgrade().and_then(|w| w.uri());
        let token = cancellable.clone();
        let finish = move |result: Result<Vec<gio::File>, glib::Error>| {
            let Some(b) = weak.upgrade() else {
                request.cancel();
                return;
            };
            if !b.finish_file_dialog(&token)
                || b.engine.borrow().is_none()
                || web.upgrade().is_none_or(|w| {
                    w.uri() != uri
                        || w.parent().is_none()
                        || b.epochs
                            .borrow()
                            .get(&(w.as_ptr() as usize))
                            .is_none_or(|e| e.get() != epoch)
                })
            {
                request.cancel();
                return;
            }
            match result {
                Ok(files) if !files.is_empty() && files.len() <= 16 => {
                    let names: Option<Vec<String>> = files
                        .iter()
                        .map(|f| f.path().and_then(|p| p.to_str().map(str::to_owned)))
                        .collect();
                    if let Some(names) =
                        names.filter(|v| v.iter().map(|s| s.len()).sum::<usize>() <= 32768)
                    {
                        request.select_files(&names.iter().map(String::as_str).collect::<Vec<_>>());
                    } else {
                        request.cancel();
                    }
                }
                _ => request.cancel(),
            }
        };
        if multiple {
            dialog.open_multiple(Some(&parent), Some(&cancellable), move |result| {
                finish(result.map(|list| {
                    if list.n_items() > 16 {
                        return Vec::new();
                    }
                    (0..list.n_items())
                        .filter_map(|i| list.item(i).and_downcast::<gio::File>())
                        .collect()
                }))
            });
        } else {
            dialog.open(Some(&parent), Some(&cancellable), move |result| {
                finish(result.map(|f| vec![f]))
            });
        }
    }
    fn start_file_dialog(&self, owner: FileOwner) -> Option<gio::Cancellable> {
        if self.file_dialogs.borrow().len() >= 4 {
            self.error("Close an existing file dialog first");
            return None;
        }
        let token = gio::Cancellable::new();
        self.file_dialogs.borrow_mut().push(FileChoice {
            token: token.clone(),
            owner,
        });
        Some(token)
    }
    fn finish_file_dialog(&self, token: &gio::Cancellable) -> bool {
        let mut pending = self.file_dialogs.borrow_mut();
        let Some(at) = pending.iter().position(|choice| &choice.token == token) else {
            return false;
        };
        pending.remove(at);
        !token.is_cancelled()
    }
    fn cancel_file_dialogs(&self, matches: impl Fn(&FileOwner) -> bool) -> Vec<webkit6::Download> {
        let mut pending = self.file_dialogs.borrow_mut();
        let mut cancelled = Vec::new();
        let mut downloads = Vec::new();
        pending.retain(|choice| {
            if matches(&choice.owner) {
                if let FileOwner::Download(download) = &choice.owner {
                    if let Some(download) = download.upgrade() {
                        downloads.push(download);
                    }
                }
                cancelled.push(choice.token.clone());
                false
            } else {
                true
            }
        });
        drop(pending); // Invalidate ownership before cancellation can invoke a callback.
        for token in cancelled {
            token.cancel();
        }
        downloads
    }
    fn cancel_page_file_dialogs(&self, web: &webkit6::WebView) {
        let _ = self.cancel_file_dialogs(|owner| matches!(owner, FileOwner::Upload { page, .. } if page.upgrade().as_ref() == Some(web)));
    }
    fn cancel_download_file_dialogs(&self, download: &webkit6::Download) {
        let _ = self.cancel_file_dialogs(|owner| matches!(owner, FileOwner::Download(pending) if pending.upgrade().as_ref() == Some(download)));
    }
    fn download(self: &Rc<Self>, download: &webkit6::Download) {
        let active = self
            .downloads
            .borrow()
            .iter()
            .filter(|t| !t.complete.get())
            .count();
        let allowed = download
            .request()
            .and_then(|r| r.uri())
            .is_some_and(|uri| self.allowed(&uri).is_ok());
        if !allowed || active >= 4 || self.downloads.borrow().len() >= MAX_DOWNLOADS {
            download.cancel();
            self.error("Download rejected by URL policy or transfer limit");
            return;
        }
        // Only an owned staging file is passed to WebKit. Rust publishes to the
        // selected destination atomically without replacing existing files.
        download.set_allow_overwrite(true);
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        let label = gtk::Label::builder()
            .label("Choose a download destination…")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        let cancel = button("process-stop-symbolic", "Cancel download");
        content.append(&label);
        content.append(&cancel);
        row.set_child(Some(&content));
        self.download_list.append(&row);
        let complete = Rc::new(Cell::new(false));
        let network_done = Rc::new(Cell::new(false));
        let destination_pending = Rc::new(Cell::new(true));
        let cancelled = crate::download_file::cancellation();
        // WebKitGTK may still deliver terminal signals after Cancel. Every
        // owned cancellation path shares one gate, invalidated before entering
        // the engine. After network completion only Rust publication is stopped.
        let stopped = cancelled.clone();
        let terminal = network_done.clone();
        let engine_download = download.downgrade();
        let stop: Rc<dyn Fn()> = Rc::new(move || {
            if !stopped.swap(true, Ordering::AcqRel) && !terminal.get() {
                if let Some(download) = engine_download.upgrade() {
                    download.cancel();
                }
            }
        });
        let target: Rc<RefCell<Option<crate::download_file::Target>>> = Rc::new(RefCell::new(None));
        let filename = Rc::new(RefCell::new(String::from("Download")));
        let failure: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        self.downloads.borrow_mut().push(Transfer {
            download: download.clone(),
            row,
            complete: complete.clone(),
            network_done: network_done.clone(),
            destination_pending: destination_pending.clone(),
            stop: stop.clone(),
            target: target.clone(),
        });
        let d = download.downgrade();
        let owner = Rc::downgrade(self);
        let stop_button = stop.clone();
        cancel.connect_clicked(move |_| {
            if let Some(d) = d.upgrade() {
                if let Some(b) = owner.upgrade() {
                    b.cancel_download_file_dialogs(&d);
                }
            }
            stop_button();
        });
        let weak = Rc::downgrade(self);
        let label2 = label.clone();
        let pending_target = target.clone();
        let pending_cancelled = cancelled.clone();
        let pending_complete = complete.clone();
        let pending_state = destination_pending.clone();
        let pending_name = filename.clone();
        let pending_failure = failure.clone();
        let pending_stop = stop.clone();
        download.connect_decide_destination(move |download, suggestion| {
            if let Some(b) = weak.upgrade() {
                if let Some(parent) = b.parent.upgrade() {
                    let name = sanitize_download_filename(suggestion);
                    label2.set_text(&name);
                    pending_name.replace(name.clone());
                    let dialog = gtk::FileDialog::builder()
                        .title("Save browser download")
                        .initial_name(&name)
                        .modal(true)
                        .build();
                    let Some(cancellable) =
                        b.start_file_dialog(FileOwner::Download(download.downgrade()))
                    else {
                        pending_stop();
                        return true;
                    };
                    let d = download.clone();
                    let weak = Rc::downgrade(&b);
                    let token = cancellable.clone();
                    let target = pending_target.clone();
                    let cancelled = pending_cancelled.clone();
                    let complete = pending_complete.clone();
                    let state = pending_state.clone();
                    let filename = pending_name.clone();
                    let failure = pending_failure.clone();
                    let stop = pending_stop.clone();
                    dialog.save(Some(&parent), Some(&cancellable), move |result| {
                        let Some(b) = weak.upgrade() else {
                            stop();
                            return;
                        };
                        if !b.finish_file_dialog(&token) {
                            return;
                        }
                        if b.engine.borrow().is_none() {
                            stop();
                            return;
                        }
                        let Some(path) = result
                            .ok()
                            .and_then(|file| file.path())
                            .filter(|p| p.is_absolute())
                        else {
                            stop();
                            return;
                        };
                        if !b.file_io.can_start() {
                            failure.replace(Some(
                                "Native file workers are busy; retry the download".into(),
                            ));
                            stop();
                            return;
                        }
                        let io = b.file_io.clone();
                        let disposal = io.clone();
                        let weak = Rc::downgrade(&b);
                        let prepare_cancel = cancelled.clone();
                        io.run(
                            move || crate::download_file::Target::prepare(path, &prepare_cancel),
                            move |result| {
                                if cancelled.load(Ordering::Acquire)
                                    || complete.get()
                                    || weak.upgrade().is_none_or(|b| b.engine.borrow().is_none())
                                {
                                    if let Ok(plan) = result {
                                        disposal.discard(plan);
                                    }
                                    return;
                                }
                                match result {
                                    Ok(plan) => {
                                        let path = plan.staging_path().to_str().map(str::to_owned);
                                        if let Some(path) = path {
                                            filename.replace(plan.filename());
                                            target.replace(Some(plan));
                                            state.set(false);
                                            d.set_destination(&path);
                                        } else {
                                            disposal.discard(plan);
                                            failure.replace(Some(
                                                "Choose a download folder with a UTF-8 path".into(),
                                            ));
                                            stop();
                                        }
                                    }
                                    Err(error) => {
                                        failure.replace(Some(error));
                                        stop();
                                    }
                                }
                            },
                        );
                    });
                    return true;
                }
            }
            pending_stop();
            true
        });
        let weak = Rc::downgrade(self);
        download.connect_response_notify(move |d| {
            let allowed = weak.upgrade().is_some_and(|b| {
                d.response()
                    .and_then(|r| r.uri())
                    .is_some_and(|uri| b.allowed(&uri).is_ok())
            });
            if !allowed {
                stop();
            }
        });
        let label2 = label.clone();
        download.connect_received_data(move |d, _| {
            label2.set_text(&format!(
                "{} · {} KiB · {:.0}%",
                filename.borrow(),
                d.received_data_length() / 1024,
                d.estimated_progress() * 100.0
            ));
        });
        let label2 = label.clone();
        let done = complete.clone();
        let cancel2 = cancel.clone();
        let owner = Rc::downgrade(self);
        let failed_target = target.clone();
        let failed_cancelled = cancelled.clone();
        let cleanup = self.file_io.clone();
        download.connect_failed(move |download, error| {
            failed_cancelled.store(true, Ordering::Release);
            if let Some(b) = owner.upgrade() {
                b.cancel_download_file_dialogs(download);
            }
            if let Some(plan) = failed_target.borrow_mut().take() {
                cleanup.discard(plan);
            }
            done.set(true);
            cancel2.set_sensitive(false);
            let error = failure
                .borrow_mut()
                .take()
                .unwrap_or_else(|| error.to_string());
            label2.set_text(&format!("Download stopped: {error}"));
        });
        let io = self.file_io.clone();
        download.connect_finished(move |_| {
            network_done.set(true);
            cancel.set_sensitive(false);
            if complete.get() {
                return;
            }
            if let Some(plan) = target.borrow_mut().take() {
                label.set_text(&crate::language::tr("Saving download…"));
                let complete = complete.clone();
                let label = label.clone();
                let cancelled = cancelled.clone();
                io.run(
                    move || plan.publish(&cancelled),
                    move |result| {
                        complete.set(true);
                        match result {
                            Ok(bytes) => label.set_text(&format!("Saved {} KiB", bytes / 1024)),
                            Err(error) => label.set_text(&format!("Download stopped: {error}")),
                        }
                    },
                );
            } else {
                complete.set(true);
            }
        });
    }
    fn show_downloads(self: &Rc<Self>) {
        if self.download_list.parent().is_some() {
            return;
        }
        let dialog = adw::Dialog::builder()
            .title("Downloads")
            .content_width(560)
            .content_height(360)
            .build();
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        let scroll = gtk::ScrolledWindow::builder()
            .child(&self.download_list)
            .vexpand(true)
            .build();
        view.set_content(Some(&scroll));
        let clear = gtk::Button::with_label("Clear completed downloads");
        let weak = Rc::downgrade(self);
        clear.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                let removed = {
                    let mut transfers = b.downloads.borrow_mut();
                    let mut removed = Vec::new();
                    transfers.retain(|t| {
                        if t.complete.get() {
                            removed.push(t.row.clone());
                            false
                        } else {
                            true
                        }
                    });
                    removed
                };
                for row in removed {
                    b.download_list.remove(&row);
                }
            }
        });
        view.add_bottom_bar(&clear);
        dialog.set_child(Some(&view));
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |dialog| {
            if let Some(b) = weak.upgrade() {
                scroll.set_child(gtk::Widget::NONE);
                b.dialogs.borrow_mut().retain(|d| d != dialog);
            }
        });
        dialog.present(self.parent.upgrade().as_ref());
        self.dialogs.borrow_mut().push(dialog);
    }
}
fn button(icon: &str, title: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(title)
        .valign(gtk::Align::Center)
        .build()
}
