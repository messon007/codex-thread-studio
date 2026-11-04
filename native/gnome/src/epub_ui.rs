//! Native EPUB controls. WebKit handles only the book's XHTML/CSS content.
#[cfg(debug_assertions)]
#[path = "epub_recovery_acceptance.rs"]
mod recovery_acceptance;
use crate::i18n_ui::{self, UiText};
use crate::{
    epub::{self, Reply, Response},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;
use webkit6::prelude::*;
const WORLD: &str = "studio-epub-reader";
const MAX_COPY_BYTES: usize = 2 * 1024 * 1024;
#[cfg(debug_assertions)]
#[path = "epub_copy_smoke.rs"]
mod copy_smoke;
const CSP:&str="default-src 'none'; script-src 'none'; style-src 'unsafe-inline' studio-epub:; img-src studio-epub: data:; font-src studio-epub: data:; connect-src 'none'; frame-src 'none'; object-src 'none'; media-src 'none'; worker-src 'none'; form-action 'none'; base-uri 'none'";
struct OpenTarget {
    root: String,
    path: String,
    comment: Option<(crate::annotations::EpubAnchor, String)>,
}
#[derive(Default)]
struct State {
    generation: u64,
    serial: u64,
    document_epoch: u64,
    id: Option<String>,
    root: String,
    path: String,
    book: Option<Arc<epub::Book>>,
    reading: Option<epub::Reading>,
    version: u64,
    current: usize,
    resource: String,
    cfi: String,
    comment: Option<(crate::annotations::EpubAnchor, String)>,
    quote: String,
    notice: Option<String>,
    anchor_match: Option<bool>,
    fragment: String,
    end: bool,
    ready: bool,
    supplement: bool,
    dirty: bool,
    saving: bool,
    recovering: bool,
    resume_pending: bool,
}
pub struct Epub {
    pub widget: adw::ToolbarView,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    title: adw::WindowTitle,
    content: gtk::Stack,
    placeholder: adw::StatusPage,
    sidebar: gtk::Revealer,
    toc: gtk::StringList,
    toc_view: gtk::ListView,
    position: gtk::Entry,
    position_display: RefCell<String>,
    total: gtk::Label,
    status: gtk::Label,
    font: gtk::SpinButton,
    theme: gtk::DropDown,
    flow: gtk::DropDown,
    search: gtk::SearchEntry,
    search_toggle: gtk::ToggleButton,
    book_search: RefCell<Option<Rc<crate::epub_search_ui::Panel>>>,
    book_search_button: gtk::Button,
    case: gtk::ToggleButton,
    previous: gtk::Button,
    next: gtk::Button,
    copy: gtk::Button,
    copying: Cell<bool>,
    commenting: Cell<bool>,
    comment: gtk::Button,
    pub comments: RefCell<Option<Rc<crate::annotations_ui::CaptureTarget>>>,
    surface: RefCell<Option<webkit6::WebView>>,
    state: RefCell<State>,
    updating: Cell<bool>,
    queued: RefCell<VecDeque<(u64, webkit6::URISchemeRequest, String)>>,
    active: RefCell<HashMap<u64, webkit6::URISchemeRequest>>,
    save_timer: RefCell<Option<glib::SourceId>>,
    close_pending: RefCell<HashSet<(u64, u64)>>,
    closing_window: Cell<bool>,
    pending_open: RefCell<Option<OpenTarget>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
}
impl Epub {
    #[cfg(debug_assertions)]
    pub async fn smoke_reference(self: &Rc<Self>, capture: impl Fn(&str)) -> Result<Value, String> {
        async fn ready(reader: &Rc<Epub>) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !reader.state.borrow().ready {
                if start.elapsed() > std::time::Duration::from_secs(20) {
                    return Err(format!(
                        "Reference book timed out: {}",
                        reader.status.text()
                    ));
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        let ticks = Rc::new(Cell::new(0usize));
        let gap = Rc::new(Cell::new(0u128));
        let last = Rc::new(Cell::new(std::time::Instant::now()));
        let (count, maximum, previous) = (ticks.clone(), gap.clone(), last.clone());
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let now = std::time::Instant::now();
            maximum.set(
                maximum
                    .get()
                    .max(now.duration_since(previous.replace(now)).as_millis()),
            );
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        });
        let started = std::time::Instant::now();
        let opened = ready(self).await;
        let open_ms = started.elapsed().as_millis();
        timer.remove();
        opened?;
        let book = self.state.borrow().book.clone().unwrap();
        let mut targets = vec![0, book.spine.len() / 2, book.spine.len() - 1];
        targets.sort_unstable();
        targets.dedup();
        let mut pages = Vec::new();
        for index in targets {
            let started = std::time::Instant::now();
            self.chapter(index, false);
            ready(self).await?;
            let web = self.surface.borrow().clone().unwrap();
            let _ = web
                .snapshot_future(
                    webkit6::SnapshotRegion::Visible,
                    webkit6::SnapshotOptions::NONE,
                )
                .await;
            let sample=web.evaluate_javascript_future("JSON.stringify({styles:document.styleSheets.length,textCharacters:document.body?.textContent.length||0,images:Array.from(document.images||[]).map(i=>({complete:i.complete,width:i.naturalWidth})),fonts:Array.from(document.fonts||[]).map(f=>({status:f.status}))})",Some(WORLD),None).await.map_err(|e|e.to_string())?;
            let sample: Value =
                serde_json::from_str(&sample.to_str()).map_err(|e| e.to_string())?;
            if sample["fonts"]
                .as_array()
                .is_some_and(|fonts| fonts.iter().any(|f| f["status"] == "error"))
            {
                return Err("Reference book had an embedded font loading error".into());
            }
            if sample["images"].as_array().is_some_and(|images| {
                images
                    .iter()
                    .any(|i| i["complete"] == true && i["width"] == 0)
            }) {
                return Err("Reference book had an image decoding error".into());
            }
            capture(&format!("epub-reference-{index}.png"));
            pages
                .push(json!({"chapter":index,"ms":started.elapsed().as_millis(),"content":sample}));
        }
        Ok(
            json!({"ok":true,"chapters":book.spine.len(),"toc":book.toc.len(),"openMs":open_ms,"mainLoopTicks":ticks.get(),"maxMainLoopGapMs":gap.get(),"pages":pages}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        restore: bool,
        capture: impl Fn(&str),
    ) -> Result<Value, String> {
        async fn until(check: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !check() {
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err("EPUB controls timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        async fn inspect(reader: &Rc<Epub>, script: &str) -> Result<Value, String> {
            let web = reader
                .surface
                .borrow()
                .clone()
                .ok_or("EPUB content missing")?;
            let value = web
                .evaluate_javascript_future(&format!("JSON.stringify({script})"), Some(WORLD), None)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::from_str(&value.to_str()).map_err(|e| e.to_string())
        }
        let started = std::time::Instant::now();
        until(|| {
            let state = self.state.borrow();
            state.ready && state.reading.as_ref().is_some_and(|r| !r.cfi.is_empty())
        })
        .await?;
        let open_ms = started.elapsed().as_millis();
        if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some() {
            until(|| {
                self.parent
                    .upgrade()
                    .is_some_and(|window| window.is_active())
            })
            .await?;
        }
        if restore {
            let state = self.state.borrow();
            let reading = state.reading.as_ref().unwrap();
            if state.current != 1
                || reading.theme != "dark"
                || (reading.font_scale - 1.2).abs() > 0.001
            {
                return Err("EPUB position/appearance were not restored".into());
            }
            return Ok(
                json!({"ok":true,"restored":true,"chapter":state.current,"cfi":reading.cfi,"openMs":open_ms}),
            );
        }
        let checks=inspect(self,"({unicode:document.body.textContent.includes('你好'),idPreserved:document.getElementById('studio-native-reader-style')?.textContent==='Source ID remains untouched.',script:document.body.hasAttribute('data-script-ran'),event:document.body.hasAttribute('data-event-ran'),color:getComputedStyle(document.querySelector('.book-color')).color,bridge:typeof window.webkit.messageHandlers.reader==='object'})").await?;
        if checks["unicode"] != true
            || checks["idPreserved"] != true
            || checks["script"] != false
            || checks["event"] != false
            || checks["color"] != "rgb(14, 100, 76)"
        {
            let diagnostics=inspect(self,"({links:Array.from(document.querySelectorAll('link')).map(l=>({href:l.href,sheet:!!l.sheet})),sheets:Array.from(document.styleSheets).map(s=>{try{return {href:s.href,disabled:s.disabled,rules:Array.from(s.cssRules).map(r=>r.cssText)}}catch(e){return {error:String(e)}}}),resources:performance.getEntriesByType('resource').map(r=>({name:r.name,bytes:r.transferSize})),violations:window.StudioReader?.violations})").await?;
            return Err(format!(
                "EPUB content/isolation failed: {checks}; {diagnostics}"
            ));
        }
        let web = self.surface.borrow().clone().unwrap();
        let untrusted = web
            .evaluate_javascript_future("typeof window.webkit?.messageHandlers?.reader", None, None)
            .await
            .map_err(|e| e.to_string())?;
        if untrusted.to_str() != "undefined" {
            return Err("Book content could access native message handler".into());
        }
        let _ = web
            .snapshot_future(
                webkit6::SnapshotRegion::Visible,
                webkit6::SnapshotOptions::NONE,
            )
            .await;
        capture("epub-reader.png");
        self.search.set_text("Needle");
        let found = Rc::new(Cell::new(false));
        let check = found.clone();
        if let Some(find) = web.find_controller() {
            find.connect_found_text(move |_, count| {
                check.set(count > 0);
            });
        }
        self.find();
        until(|| found.get()).await?;
        self.search.set_text("");
        self.find();
        glib::timeout_future(std::time::Duration::from_millis(300)).await;
        self.copy.clipboard().set_text("native-epub-copy-sentinel");
        let sentinel = self
            .copy
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| format!("GTK clipboard before EPUB Copy: {e}"))?;
        if sentinel.as_deref() != Some("native-epub-copy-sentinel") {
            return Err("GTK clipboard did not retain its private sentinel".into());
        }
        // Programmatic focus cannot create a Wayland input serial.
        glib::timeout_future(std::time::Duration::from_millis(250)).await;
        let selected=web.evaluate_javascript_future("(()=>{const t=document.querySelector('#copy').firstChild;const r=document.createRange();r.setStart(t,0);r.setEnd(t,6);getSelection().removeAllRanges();getSelection().addRange(r);return JSON.stringify({text:getSelection().toString(),ranges:getSelection().rangeCount,focus:document.hasFocus()})})()",Some(WORLD),None).await.map_err(|e|e.to_string())?;
        let selected = selected.to_str();
        let selected: Value = serde_json::from_str(&selected).map_err(|e| e.to_string())?;
        if selected["text"] != "Needle" {
            return Err(format!(
                "EPUB DOM selection was not established: {selected}"
            ));
        }
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        let clipboard_changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let changes = clipboard_changes.clone();
        let handler = self.copy.clipboard().connect_changed(move |clipboard| {
            let mut changes = changes.borrow_mut();
            if changes.len() < 16 {
                changes.push(format!(
                    "local={} provider={} formats={}",
                    clipboard.is_local(),
                    clipboard.content().is_some(),
                    clipboard.formats()
                ));
            }
        });
        self.copy.emit_clicked();
        // The DOM selection is read asynchronously; GTK owns the resulting text.
        let mut copied = false;
        let mut clipboard_observed = String::new();
        for _ in 0..40 {
            let result = self.copy.clipboard().read_text_future().await;
            copied = result.as_ref().ok().and_then(|v| v.as_deref()) == Some("Needle");
            clipboard_observed = match result {
                Ok(Some(text)) => format!("<text: {} characters>", text.chars().count()),
                Ok(None) => "<empty>".into(),
                Err(error) => format!("<error: {error}>"),
            };
            if copied {
                break;
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        let copy_error = if !copied {
            let selection = inspect(self, "({selection:getSelection()?.toString()})").await?;
            Some(format!(
                "EPUB native Copy lost selected text: {}; {selection}; clipboard={clipboard_observed:?}; changes={:?}",
                self.status.text(),clipboard_changes.borrow()
            ))
        } else {
            None
        };
        self.copy.clipboard().disconnect(handler);
        self.search.set_text("");
        self.position.set_text("2");
        self.position.emit_activate();
        until(|| {
            let state = self.state.borrow();
            state.current == 1
                && state.ready
                && state
                    .reading
                    .as_ref()
                    .is_some_and(|r| epub::cfi_chapter(&r.cfi) == Some(1))
        })
        .await?;
        self.flow.set_selected(1);
        self.font.set_value(120.0);
        self.theme.set_selected(2);
        glib::timeout_future(std::time::Duration::from_millis(400)).await;
        self.next.emit_clicked();
        until(|| {
            self.state
                .borrow()
                .reading
                .as_ref()
                .is_some_and(|r| r.progress > 1.0 / 3.0 + 0.001)
        })
        .await?;
        let colors=inspect(self,"({background:getComputedStyle(document.body).backgroundColor,font:getComputedStyle(document.body).fontSize})").await?;
        let font = colors["font"]
            .as_str()
            .and_then(|v| v.strip_suffix("px"))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        if colors["background"] != "rgb(30, 30, 30)" || (font - 21.6).abs() > 0.1 {
            return Err(format!("EPUB paper theme was not applied: {colors}"));
        }
        self.toc_view.emit_by_name::<()>("activate", &[&2u32]);
        until(|| {
            let state = self.state.borrow();
            state.current == 2
                && state.ready
                && state
                    .reading
                    .as_ref()
                    .is_some_and(|r| epub::cfi_chapter(&r.cfi) == Some(2))
        })
        .await?;
        self.chapter(1, false);
        until(|| {
            let state = self.state.borrow();
            state.current == 1
                && state.ready
                && state
                    .reading
                    .as_ref()
                    .is_some_and(|r| epub::cfi_chapter(&r.cfi) == Some(1))
        })
        .await?;
        // Leave the last position update for the real window-close handshake.
        web.evaluate_javascript_future("scrollTo(innerWidth*2,0)", Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        let _ = web
            .snapshot_future(
                webkit6::SnapshotRegion::Visible,
                webkit6::SnapshotOptions::NONE,
            )
            .await;
        capture("epub-paged-dark.png");
        glib::timeout_future(std::time::Duration::from_millis(300)).await;
        let before_hide = self.state.borrow().reading.as_ref().unwrap().cfi.clone();
        let stack = self
            .widget
            .parent()
            .and_downcast::<gtk::Stack>()
            .ok_or("EPUB workspace stack missing")?;
        stack.set_visible_child_name("document");
        glib::timeout_future(std::time::Duration::from_millis(250)).await;
        if self.state.borrow().reading.as_ref().unwrap().cfi != before_hide {
            return Err("Hidden EPUB view changed the saved reading anchor".into());
        }
        stack.set_visible_child(&self.widget);
        glib::timeout_future(std::time::Duration::from_millis(300)).await;
        let (root, path) = {
            let state = self.state.borrow();
            (state.root.clone(), state.path.clone())
        };
        drop(web);
        self.open(&root, &path);
        self.open(&root, &path);
        self.open(&root, &path);
        until(|| {
            let state = self.state.borrow();
            state.ready
                && state.current == 1
                && state
                    .reading
                    .as_ref()
                    .is_some_and(|r| r.theme == "dark" && (r.font_scale - 1.2).abs() < 0.001)
        })
        .await?;
        Ok(
            json!({"ok":copy_error.is_none(),"clipboardError":copy_error,"openMs":open_ms,"styles":true,"unicode":true,"scriptIsolation":true,"copy":copied,"chapterEntry":true,"contents":true,"hiddenAnchor":true,"rapidReplacement":true,"paged":true,"theme":true,"font":true,"search":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_comment(self: &Rc<Self>) -> Result<(), String> {
        async fn ready(reader: &Epub) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !reader.state.borrow().ready {
                if start.elapsed() > std::time::Duration::from_secs(20) {
                    return Err(format!(
                        "EPUB comment did not load: {}",
                        reader.status.text()
                    ));
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        ready(self).await?;
        self.chapter(1, false);
        ready(self).await?;
        let web = self
            .surface
            .borrow()
            .clone()
            .ok_or("Missing EPUB surface")?;
        let _ = web
            .snapshot_future(
                webkit6::SnapshotRegion::Visible,
                webkit6::SnapshotOptions::NONE,
            )
            .await;
        web.evaluate_javascript_future("(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('p15'));const s=getSelection();s.removeAllRanges();s.addRange(r)})()",Some(WORLD),None).await.map_err(|e|e.to_string())?;
        glib::timeout_future(std::time::Duration::from_millis(40)).await;
        web.evaluate_javascript_future("getSelection().removeAllRanges()", Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        self.comment_selection();
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_comment_reopened(
        &self,
        quote: &str,
        expected: bool,
        changed: bool,
    ) -> Result<(), String> {
        let started = std::time::Instant::now();
        while !self.state.borrow().ready || self.state.borrow().anchor_match.is_none() {
            if started.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "EPUB comment reopening timed out: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.state.borrow().current != 1 || self.state.borrow().anchor_match != Some(expected) {
            return Err(format!(
                "Wrong EPUB section or unverified range highlight: {}",
                self.status.text()
            ));
        }
        if changed && !self.state.borrow().cfi.is_empty() {
            return Err("Changed EPUB edition reused a stale CFI".into());
        }
        let web = self
            .surface
            .borrow()
            .clone()
            .ok_or("EPUB surface missing")?;
        let selected = web
            .evaluate_javascript_future("String(getSelection()||'')", Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?
            .to_str()
            .to_string();
        if expected && selected.trim() != quote.trim() {
            return Err("EPUB CFI did not highlight the original quoted passage".into());
        }
        if !expected && !selected.is_empty() {
            return Err("Unverified EPUB comment highlighted another passage".into());
        }
        Ok(())
    }
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let widget = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        let title = adw::WindowTitle::new("EPUB", "");
        header.set_title_widget(Some(&title));
        let open = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Open EPUB")
            .build()
            .ui_text("tooltip-text", "Open EPUB");
        header.pack_start(&open);
        let close = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text("Close EPUB")
            .build()
            .ui_text("tooltip-text", "Close EPUB");
        header.pack_end(&close);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload EPUB")
            .build()
            .ui_text("tooltip-text", "Reload EPUB");
        header.pack_end(&reload);
        let comment = gtk::Button::builder()
            .icon_name("insert-text-symbolic")
            .tooltip_text("Comment on selected passage")
            .focus_on_click(false)
            .build()
            .ui_text("tooltip-text", "Comment on selected passage");
        header.pack_end(&comment);
        let translate = gtk::Button::builder()
            .icon_name("preferences-desktop-locale-symbolic")
            .tooltip_text("Translate selected text")
            .focus_on_click(false)
            .build();
        crate::i18n_ui::bind(&translate, "tooltip-text", "Translate selected text");
        header.pack_end(&translate);
        let settings = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Reading settings")
            .build()
            .ui_text("tooltip-text", "Reading settings");
        // A menu button otherwise derives its accessible name from the open
        // popover's contents, changing its identity as options are selected.
        i18n_ui::accessible_label(&settings, "Reading settings");
        header.pack_end(&settings);
        let contents = gtk::ToggleButton::builder()
            .icon_name("view-list-symbolic")
            .tooltip_text("Contents")
            .build()
            .ui_text("tooltip-text", "Contents");
        header.pack_end(&contents);
        let find = gtk::ToggleButton::builder()
            .icon_name("edit-find-symbolic")
            .tooltip_text("Search this section (Ctrl+F)")
            .build()
            .ui_text("tooltip-text", "Search this section (Ctrl+F)");
        header.pack_end(&find);
        let copy = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy selected text")
            .build()
            .ui_text("tooltip-text", "Copy selected text");
        copy.set_focus_on_click(false);
        header.pack_end(&copy);
        widget.add_top_bar(&header);
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search this section")
            .hexpand(true)
            .width_chars(12)
            .max_width_chars(32)
            .build()
            .ui_text("placeholder-text", "Search this section");
        i18n_ui::accessible_label(&search, "Search this section");
        let case = gtk::ToggleButton::with_label("Aa");
        i18n_ui::bind(&case, "tooltip-text", "Match case");
        let search_prev = gtk::Button::builder()
            .icon_name("go-up-symbolic")
            .tooltip_text("Previous match")
            .build()
            .ui_text("tooltip-text", "Previous match");
        let search_next = gtk::Button::builder()
            .icon_name("go-down-symbolic")
            .tooltip_text("Next match")
            .build()
            .ui_text("tooltip-text", "Next match");
        let search_book = gtk::Button::with_label("Book…").ui_text("label", "Book…");
        i18n_ui::bind(
            &search_book,
            "tooltip-text",
            "Search every section in this book",
        );
        let search_box = crate::adaptive::toolbar(6);
        for child in [
            search.upcast_ref::<gtk::Widget>(),
            case.upcast_ref(),
            search_prev.upcast_ref(),
            search_next.upcast_ref(),
            search_book.upcast_ref(),
        ] {
            search_box.append(child);
        }
        search_box.set_margin_start(12);
        search_box.set_margin_end(12);
        search_box.set_margin_bottom(6);
        let search_reveal = gtk::Revealer::new();
        search_reveal.set_child(Some(&search_box));
        widget.add_top_bar(&search_reveal);
        let content = gtk::Stack::new();
        let placeholder = adw::StatusPage::builder()
            .title("Open an EPUB")
            .icon_name("accessories-dictionary-symbolic")
            .build()
            .ui_text("title", "Open an EPUB");
        content.add_named(&placeholder, Some("placeholder"));
        content.set_visible_child_name("placeholder");
        let toc = gtk::StringList::new(&[]);
        let selection = gtk::SingleSelection::new(Some(toc.clone()));
        selection.set_autoselect(false);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .lines(3)
                .margin_start(12)
                .margin_end(12)
                .margin_top(8)
                .margin_bottom(8)
                .build();
            item.set_child(Some(&label));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let label = item.child().and_downcast::<gtk::Label>().unwrap();
            label.set_text(
                &item
                    .item()
                    .and_downcast::<gtk::StringObject>()
                    .unwrap()
                    .string(),
            );
        });
        let toc_view = gtk::ListView::new(Some(selection), Some(factory));
        toc_view.set_single_click_activate(true);
        let sidebar = gtk::Revealer::new();
        contents
            .bind_property("active", &sidebar, "reveal-child")
            .bidirectional()
            .sync_create()
            .build();
        sidebar.set_transition_type(gtk::RevealerTransitionType::SlideRight);
        sidebar.set_child(Some(
            &gtk::ScrolledWindow::builder()
                .width_request(240)
                .max_content_width(320)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&toc_view)
                .build(),
        ));
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&sidebar);
        content.set_hexpand(true);
        content.set_vexpand(true);
        body.append(&content);
        widget.set_content(Some(&body));
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        footer.set_margin_start(12);
        footer.set_margin_end(12);
        footer.set_margin_top(6);
        footer.set_margin_bottom(6);
        let status = gtk::Label::builder()
            .label("Read only")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build()
            .ui_text("label", "Read only");
        status.add_css_class("dim-label");
        footer.append(&status);
        let previous = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Previous page or chapter")
            .build()
            .ui_text("tooltip-text", "Previous page or chapter");
        footer.append(&previous);
        let position = gtk::Entry::builder()
            .width_chars(4)
            .max_width_chars(6)
            .max_length(5)
            .xalign(0.5)
            .tooltip_text("Reading section number")
            .build()
            .ui_text("tooltip-text", "Reading section number");
        footer.append(&position);
        let total = gtk::Label::new(Some("/ 0"));
        total.add_css_class("dim-label");
        footer.append(&total);
        let next = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Next page or chapter")
            .build()
            .ui_text("tooltip-text", "Next page or chapter");
        footer.append(&next);
        widget.add_bottom_bar(&footer);
        let popover = gtk::Popover::new();
        let options = gtk::Box::new(gtk::Orientation::Vertical, 12);
        options.set_margin_start(16);
        options.set_margin_end(16);
        options.set_margin_top(16);
        options.set_margin_bottom(16);
        let font = gtk::SpinButton::with_range(75.0, 200.0, 5.0);
        font.set_value(100.0);
        let theme = gtk::DropDown::from_strings(&["Light", "Sepia", "Dark"]);
        let flow = gtk::DropDown::from_strings(&["Scroll", "Pages"]);
        crate::adaptive::ui_string_selector(&theme, 12, None);
        crate::adaptive::ui_string_selector(&flow, 12, Some("EPUB flow"));
        for (caption, child) in [
            ("Text size (%)", font.upcast_ref::<gtk::Widget>()),
            ("Paper", theme.upcast_ref()),
            ("Layout", flow.upcast_ref()),
        ] {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            let label = gtk::Label::builder()
                .label(caption)
                .xalign(0.0)
                .hexpand(true)
                .build();
            i18n_ui::bind(&label, "label", caption);
            i18n_ui::accessible_label(child, caption);
            row.append(&label);
            row.append(child);
            options.append(&row);
        }
        popover.set_child(Some(&options));
        settings.set_popover(Some(&popover));
        let reader = Rc::new(Self {
            widget,
            parent: parent.downgrade(),
            sender,
            title,
            content,
            placeholder,
            sidebar,
            toc,
            toc_view,
            position,
            position_display: RefCell::new(String::new()),
            total,
            status,
            font,
            theme,
            flow,
            search,
            search_toggle: find.clone(),
            book_search: RefCell::new(None),
            book_search_button: search_book.clone(),
            case,
            previous,
            next,
            copy,
            copying: Cell::new(false),
            commenting: Cell::new(false),
            comment,
            comments: RefCell::new(None),
            surface: RefCell::new(None),
            state: RefCell::new(State::default()),
            updating: Cell::new(false),
            queued: RefCell::new(VecDeque::new()),
            active: RefCell::new(HashMap::new()),
            save_timer: RefCell::new(None),
            on_close: RefCell::new(None),
            on_open: RefCell::new(None),
            close_pending: RefCell::new(HashSet::new()),
            closing_window: Cell::new(false),
            pending_open: RefCell::new(None),
        });
        let weak = Rc::downgrade(&reader);
        translate.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.translate_selection();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.comment.connect_clicked({
            let weak = Rc::downgrade(&reader);
            move |_| {
                if let Some(reader) = weak.upgrade() {
                    reader.comment_selection();
                }
            }
        });
        open.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.choose();
            }
        });
        let weak = Rc::downgrade(&reader);
        close.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.close();
                if let Some(callback) = reader.on_close.borrow().as_ref() {
                    callback();
                }
            }
        });
        let weak = Rc::downgrade(&reader);
        reload.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                let (root, path) = {
                    let state = reader.state.borrow();
                    (state.root.clone(), state.path.clone())
                };
                if !path.is_empty() {
                    reader.open(&root, &path);
                }
            }
        });
        let weak = Rc::downgrade(&reader);
        contents.connect_toggled(move |button| {
            if let Some(reader) = weak.upgrade() {
                reader.sidebar.set_reveal_child(button.is_active());
                if !reader.updating.get() {
                    reader.preference_changed();
                }
            }
        });
        let weak = Rc::downgrade(&reader);
        find.connect_toggled(move |button| {
            if let Some(reader) = weak.upgrade() {
                search_reveal.set_reveal_child(button.is_active());
                if button.is_active() {
                    reader.search.grab_focus();
                } else if let Some(web) = reader.surface.borrow().as_ref() {
                    if let Some(find) = web.find_controller() {
                        find.search_finish();
                    }
                }
            }
        });
        for (button, direction) in [(&reader.previous, -1), (&reader.next, 1)] {
            let weak = Rc::downgrade(&reader);
            button.connect_clicked(move |_| {
                if let Some(reader) = weak.upgrade() {
                    reader.javascript(format!("StudioReader.step({direction})"));
                }
            });
        }
        let weak = Rc::downgrade(&reader);
        reader.copy.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.copy_selection();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.position.connect_activate(move |entry| {
            if let Some(reader) = weak.upgrade() {
                let target = entry
                    .text()
                    .parse::<usize>()
                    .ok()
                    .and_then(|v| v.checked_sub(1));
                if let Some(target) = target {
                    reader.chapter(target, false);
                }
                reader.reset_position();
                reader.controls();
            }
        });
        let position_focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(&reader);
        position_focus.connect_leave(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.reset_position();
            }
        });
        reader.position.add_controller(position_focus);
        let weak = Rc::downgrade(&reader);
        reader.toc_view.connect_activate(move |_, index| {
            if let Some(reader) = weak.upgrade() {
                let href = reader
                    .state
                    .borrow()
                    .book
                    .as_ref()
                    .and_then(|book| book.toc.get(index as usize))
                    .map(|h| h.href.clone());
                if let Some(href) = href {
                    reader.navigate(&href, "", false);
                }
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.font.connect_value_changed(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.preference_changed();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.theme.connect_selected_notify(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.preference_changed();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.flow.connect_selected_notify(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.preference_changed();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.book_search_button.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.search_book();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.search.connect_search_changed(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.find();
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.case.connect_toggled(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.find();
            }
        });
        for (button, forward) in [(search_next, true), (search_prev, false)] {
            let weak = Rc::downgrade(&reader);
            button.connect_clicked(move |_| {
                if let Some(reader) = weak.upgrade() {
                    if let Some(web) = reader.surface.borrow().as_ref() {
                        let Some(find) = web.find_controller() else {
                            return;
                        };
                        if forward {
                            find.search_next();
                        } else {
                            find.search_previous();
                        }
                    }
                }
            });
        }
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&reader);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gtk::gdk::Key::f
                && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !modifiers.intersects(
                    gtk::gdk::ModifierType::SHIFT_MASK
                        | gtk::gdk::ModifierType::ALT_MASK
                        | gtk::gdk::ModifierType::SUPER_MASK,
                )
            {
                if let Some(reader) = weak.upgrade() {
                    find.set_active(true);
                    reader.search.grab_focus();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        reader.widget.add_controller(keys);
        reader.controls();
        reader
    }
    fn request(&self, kind: &'static str, body: Value) -> Option<u64> {
        let (generation, serial) = {
            let mut state = self.state.borrow_mut();
            state.serial += 1;
            (state.generation, state.serial)
        };
        if self
            .sender
            .try_send(Command::Epub {
                generation,
                serial,
                kind,
                body,
            })
            .is_err()
        {
            if kind == "resume" {
                let mut state = self.state.borrow_mut();
                state.resume_pending = false;
                state.id = None;
                drop(state);
                if let Some(panel) = self.book_search.borrow().as_ref() {
                    panel.gateway_failed("EPUB request queue is busy; reconnect to try again");
                }
            }
            self.status_message("EPUB request queue is busy; try again");
            return None;
        }
        Some(serial)
    }
    fn update(&self) -> Option<Value> {
        let state = self.state.borrow();
        state
            .reading
            .as_ref()
            .filter(|_| state.version > 0)
            .map(|reading| json!({"revision":state.version,"reading":reading}))
    }
    pub fn open(self: &Rc<Self>, root: &str, path: &str) {
        self.open_target(OpenTarget {
            root: root.into(),
            path: path.into(),
            comment: None,
        });
    }
    pub fn open_comment(self: &Rc<Self>, anchor: crate::annotations::EpubAnchor, excerpt: &str) {
        self.open_target(OpenTarget {
            root: anchor.root.clone(),
            path: anchor.file_path.clone(),
            comment: Some((anchor, excerpt.into())),
        });
    }
    fn open_target(self: &Rc<Self>, target: OpenTarget) {
        // Capture the user's navigation now; a deferred reading-position save
        // must not steal focus when its acknowledgement later starts the open.
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
        self.closing_window.set(false);
        self.close();
        if !self.close_pending.borrow().is_empty() {
            self.pending_open.replace(Some(target));
            i18n_ui::bind(&self.placeholder, "title", "Opening EPUB…");
            self.status_message("Saving previous reading position…");
            return;
        }
        self.begin_open(target);
    }
    fn begin_open(self: &Rc<Self>, target: OpenTarget) {
        let id = glib::uuid_string_random().to_string();
        {
            let mut state = self.state.borrow_mut();
            let generation = state.generation + 1;
            *state = State {
                generation,
                id: Some(id.clone()),
                root: target.root.clone(),
                path: target.path.clone(),
                comment: target.comment,
                ..State::default()
            };
        }
        self.reset_position();
        i18n_ui::literal(
            &self.title,
            "title",
            std::path::Path::new(&target.path)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("EPUB"),
        );
        i18n_ui::literal(&self.title, "subtitle", &target.path);
        i18n_ui::bind(&self.placeholder, "title", "Opening EPUB…");
        self.placeholder.set_description(None);
        self.content.set_visible_child_name("placeholder");
        self.status_message("Reading archive…");
        self.toc.splice(0, self.toc.n_items(), &[]);
        self.request(
            "open",
            json!({"operation":"open","id":id,"root":target.root,"path":target.path}),
        );
        self.controls();
    }
    pub fn close(&self) {
        let panel = self.book_search.borrow_mut().take();
        if let Some(panel) = panel {
            panel.dialog.force_close();
        }
        self.pending_open.borrow_mut().take();
        self.copying.set(false);
        self.commenting.set(false);
        if let Some(timer) = self.save_timer.borrow_mut().take() {
            timer.remove();
        }
        let update = self.update();
        let id = {
            let mut state = self.state.borrow_mut();
            state.generation += 1;
            state.ready = false;
            state.recovering = false;
            state.resume_pending = false;
            state.id.take()
        };
        if let Some(id) = id {
            if let Some(serial) = self.request(
                "close",
                json!({"operation":"close","id":id,"update":update}),
            ) {
                self.close_pending
                    .borrow_mut()
                    .insert((self.state.borrow().generation, serial));
            }
        }
        if let Some(web) = self.surface.borrow_mut().take() {
            web.stop_loading();
            web.terminate_web_process();
            self.content.remove(&web);
        }
        self.cancel_assets();
        self.state.borrow_mut().book = None;
        self.state.borrow_mut().reading = None;
        self.content.set_visible_child_name("placeholder");
        self.controls();
    }
    #[cfg(debug_assertions)]
    pub fn smoke_ready(&self) -> bool {
        self.state.borrow().ready
    }
    pub fn has_document(&self) -> bool {
        let state = self.state.borrow();
        state.id.is_some() || state.recovering || self.pending_open.borrow().is_some()
    }
    pub fn gateway_failed(&self, error: &str) {
        if let Some(timer) = self.save_timer.borrow_mut().take() {
            timer.remove();
        }
        let mut state = self.state.borrow_mut();
        let open = state.id.take().is_some();
        state.recovering |= open;
        state.resume_pending = false;
        state.generation += 1;
        state.ready = false;
        state.saving = false;
        state.anchor_match = Some(false);
        drop(state);
        self.pending_open.borrow_mut().take();
        self.copying.set(false);
        self.commenting.set(false);
        if let Some(panel) = self.book_search.borrow().as_ref() {
            panel.gateway_failed(error);
        }
        self.cancel_assets();
        if let Some(web) = self.surface.borrow().as_ref() {
            web.stop_loading();
        }
        self.controls();
        if open {
            self.literal_message(error);
        }
    }
    pub fn gateway_restored(&self) {
        let body = {
            let mut state = self.state.borrow_mut();
            if !state.recovering || state.resume_pending {
                return;
            }
            let (Some(book), Some(reading)) = (&state.book, &state.reading) else {
                return;
            };
            let id = glib::uuid_string_random().to_string();
            let body = json!({"id":id,"root":book.root,"path":book.path,"hash":book.hash,"reading":reading});
            state.id = Some(id);
            state.generation += 1;
            state.resume_pending = true;
            body
        };
        self.status_message("Revalidating EPUB…");
        if let Some(panel) = self.book_search.borrow().as_ref() {
            panel.revalidating();
        }
        self.request("resume", body);
    }
    pub fn allow_window_close(self: &Rc<Self>) -> bool {
        self.pending_open.borrow_mut().take();
        if self.state.borrow().id.is_none() && self.close_pending.borrow().is_empty() {
            return true;
        }
        if self.closing_window.replace(true) {
            return false;
        }
        if self.state.borrow().id.is_none() {
            return false;
        }
        let weak = Rc::downgrade(self);
        let web = self.surface.borrow().clone().filter(|web| web.is_mapped());
        let generation = self.state.borrow().generation;
        self.status_message("Saving reading position…");
        glib::spawn_future_local(async move {
            let raw = if let Some(web) = web {
                web.evaluate_javascript_future("StudioReader.snapshot()", Some(WORLD), None)
                    .await
                    .ok()
                    .filter(|v| v.is_string())
                    .map(|v| v.to_str().to_string())
                    .filter(|v| v.len() <= 16 * 1024)
            } else {
                None
            };
            if let Some(reader) = weak.upgrade() {
                let current = reader.state.borrow().generation == generation;
                if current {
                    if let Some(raw) = raw {
                        if let Ok(Ok(message)) =
                            gio::spawn_blocking(move || epub::message(raw)).await
                        {
                            reader.message(message);
                        }
                    }
                    reader.close();
                }
                if reader.closing_window.get() && reader.close_pending.borrow().is_empty() {
                    if let Some(parent) = reader.parent.upgrade() {
                        parent.close();
                    }
                }
            }
        });
        false
    }
    fn fail_request(request: &webkit6::URISchemeRequest, message: &str) {
        request.finish_error(&mut glib::Error::new(gio::IOErrorEnum::Cancelled, message));
    }
    fn cancel_assets(&self) {
        let queued = self.queued.borrow_mut().drain(..).collect::<Vec<_>>();
        let active = self.active.borrow_mut().drain().collect::<Vec<_>>();
        for (_, request, _) in queued {
            Self::fail_request(&request, "EPUB navigation cancelled");
        }
        for (_, request) in active {
            Self::fail_request(&request, "EPUB navigation cancelled");
        }
    }
    fn pump(&self) {
        while self.active.borrow().len() < 1 {
            let Some((serial, request, path)) = self.queued.borrow_mut().pop_front() else {
                break;
            };
            let state = self.state.borrow();
            let Some(id) = &state.id else {
                Self::fail_request(&request, "EPUB closed");
                continue;
            };
            let body = json!({"operation":"asset","id":id,"path":path});
            let generation = state.generation;
            drop(state);
            if self
                .sender
                .try_send(Command::Epub {
                    generation,
                    serial,
                    kind: "asset",
                    body,
                })
                .is_err()
            {
                Self::fail_request(&request, "EPUB resource queue is busy");
                continue;
            }
            self.active.borrow_mut().insert(serial, request);
        }
    }
    fn surface(self: &Rc<Self>) {
        let mut pressure = webkit6::MemoryPressureSettings::new();
        pressure.set_memory_limit(512);
        pressure.set_kill_threshold(1.0);
        pressure.set_poll_interval(1.0);
        let context = webkit6::WebContext::builder()
            .memory_pressure_settings(&pressure)
            .build();
        if let Some(security) = context.security_manager() {
            security.register_uri_scheme_as_secure("studio-epub");
            security.register_uri_scheme_as_cors_enabled("studio-epub");
        }
        context.set_cache_model(webkit6::CacheModel::DocumentViewer);
        let weak = Rc::downgrade(self);
        let generation = self.state.borrow().generation;
        context.register_uri_scheme("studio-epub", move |request| {
            let Some(reader) = weak.upgrade() else {
                Self::fail_request(request, "EPUB reader closed");
                return;
            };
            let uri = request.uri().and_then(|v| url::Url::parse(&v).ok());
            let valid = {
                let state = reader.state.borrow();
                state.generation == generation
                    && uri.as_ref().is_some_and(|u| {
                        u.host_str() == state.id.as_deref()
                            && u.query().is_none()
                            && u.username().is_empty()
                    })
            };
            if !valid || reader.queued.borrow().len() + reader.active.borrow().len() >= 64 {
                Self::fail_request(request, "EPUB resource request was rejected");
                return;
            }
            let serial = {
                let mut state = reader.state.borrow_mut();
                state.serial += 1;
                state.serial
            };
            let path = uri.unwrap().path().trim_start_matches('/').to_owned();
            reader
                .queued
                .borrow_mut()
                .push_back((serial, request.clone(), path));
            reader.pump();
        });
        let manager = webkit6::UserContentManager::new();
        manager.register_script_message_handler("reader", Some(WORLD));
        let script = format!(
            "{}\n{}",
            include_str!("../vendor/epub/cfi.js"),
            include_str!("epub_content.js")
        );
        manager.add_script(&webkit6::UserScript::for_world(
            &script,
            webkit6::UserContentInjectedFrames::TopFrame,
            webkit6::UserScriptInjectionTime::End,
            WORLD,
            &[],
            &[],
        ));
        let weak = Rc::downgrade(self);
        manager.connect_script_message_received(Some("reader"), move |_, value| {
            if !value.is_string() {
                return;
            }
            let raw = value.to_str();
            if raw.len() > 16 * 1024 {
                return;
            }
            let raw = raw.to_string();
            let weak = weak.clone();
            glib::spawn_future_local(async move {
                let parsed = gio::spawn_blocking(move || epub::message(raw)).await;
                if let (Some(reader), Ok(Ok(message))) = (weak.upgrade(), parsed) {
                    if reader.state.borrow().generation == generation {
                        reader.message(message);
                    }
                }
            });
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
        let web = webkit6::WebView::builder()
            .web_context(&context)
            .network_session(&webkit6::NetworkSession::new_ephemeral())
            .user_content_manager(&manager)
            .settings(&settings)
            .default_content_security_policy(CSP)
            .vexpand(true)
            .hexpand(true)
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
        web.connect_decide_policy(move |_, decision, kind| {
            if !matches!(
                kind,
                webkit6::PolicyDecisionType::NavigationAction
                    | webkit6::PolicyDecisionType::NewWindowAction
            ) {
                return false;
            }
            let Some(reader) = weak.upgrade() else {
                decision.ignore();
                return true;
            };
            let Some(action) = decision
                .downcast_ref::<webkit6::NavigationPolicyDecision>()
                .and_then(|d| d.navigation_action())
            else {
                decision.ignore();
                return true;
            };
            let uri = action
                .request()
                .and_then(|r| r.uri())
                .and_then(|v| url::Url::parse(&v).ok());
            let current = reader.state.borrow().generation == generation;
            if let Some(uri) = uri {
                let same = {
                    let state = reader.state.borrow();
                    uri.scheme() == "studio-epub" && uri.host_str() == state.id.as_deref()
                };
                if current && same {
                    if action.navigation_type() == webkit6::NavigationType::LinkClicked {
                        decision.ignore();
                        let href = format!(
                            "{}{}",
                            uri.path().trim_start_matches('/'),
                            uri.fragment().map(|f| format!("#{f}")).unwrap_or_default()
                        );
                        reader.navigate(&href, "", false);
                    } else {
                        decision.use_();
                    }
                    return true;
                }
                if current
                    && action.is_user_gesture()
                    && action.navigation_type() == webkit6::NavigationType::LinkClicked
                    && matches!(uri.scheme(), "http" | "https")
                {
                    let _ = gio::AppInfo::launch_default_for_uri(
                        uri.as_str(),
                        gio::AppLaunchContext::NONE,
                    );
                }
            }
            decision.ignore();
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_load_failed(move |_, _, _, error| {
            if let Some(reader) = weak.upgrade() {
                if reader.state.borrow().generation == generation {
                    reader.message_format(
                        "Unable to load EPUB chapter: {error}",
                        &[("error", &error.to_string())],
                    );
                }
            }
            true
        });
        let weak = Rc::downgrade(self);
        web.connect_web_process_terminated(move |_, _| {
            if let Some(reader) = weak.upgrade() {
                if reader.state.borrow().generation == generation {
                    reader.state.borrow_mut().ready = false;
                    reader
                        .status
                        .set_text("EPUB renderer stopped. Reload to reopen the book.");
                    reader.cancel_assets();
                    reader.controls();
                }
            }
        });
        self.content.add_named(&web, Some("book"));
        self.content.set_visible_child_name("book");
        *self.surface.borrow_mut() = Some(web);
    }
    fn chapter(self: &Rc<Self>, index: usize, end: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.quote.clear();
            state.notice = None;
        }
        let href = self
            .state
            .borrow()
            .book
            .as_ref()
            .and_then(|book| book.spine.get(index))
            .map(|s| s.href.clone());
        if let Some(href) = href {
            self.navigate(&href, "", end);
        }
    }
    fn navigate(self: &Rc<Self>, href: &str, cfi: &str, end: bool) {
        if href.len() > 8192 {
            return;
        }
        let body = {
            let mut state = self.state.borrow_mut();
            let Some(book) = state.book.clone() else {
                return;
            };
            let Some(id) = &state.id else { return };
            let uri = format!("studio-epub://{id}/{href}");
            self.copying.set(false);
            self.commenting.set(false);
            state.document_epoch += 1;
            if state.ready && cfi.is_empty() {
                state.quote.clear();
                state.notice = None;
            }
            let (resource, fragment) = href.split_once('#').unwrap_or((href, ""));
            if state.resource == resource && state.ready && !fragment.is_empty() {
                // Recovery must follow the latest same-section link rather
                // than the fragment that originally loaded this surface.
                state.fragment = fragment.into();
                state.cfi.clear();
                state.end = false;
                drop(state);
                self.controls();
                self.javascript(format!("StudioReader.anchor({})", json!(fragment)));
                return;
            }
            let chapter = book.spine.iter().position(|s| s.href == resource);
            state.supplement = chapter.is_none();
            if let Some(chapter) = chapter {
                state.current = chapter;
            }
            state.resource = resource.into();
            state.fragment = fragment.into();
            state.cfi = cfi.into();
            state.end = end;
            state.ready = false;
            uri
        };
        self.cancel_assets();
        self.status_message("Loading section…");
        let web = self.surface.borrow().clone();
        if let Some(web) = web {
            web.load_uri(&body);
        }
        self.controls();
    }
    fn message(self: &Rc<Self>, message: epub::Message) {
        let href = match &message {
            epub::Message::Anchor { href, .. }
            | epub::Message::Ready { href }
            | epub::Message::LaidOut { href }
            | epub::Message::Position { href, .. }
            | epub::Message::Edge { href, .. } => href,
        };
        let matches = {
            let state = self.state.borrow();
            url::Url::parse(href).is_ok_and(|u| {
                u.scheme() == "studio-epub"
                    && u.host_str() == state.id.as_deref()
                    && u.path().trim_start_matches('/') == state.resource
            })
        };
        if !matches {
            return;
        }
        match message {
            epub::Message::Anchor { matched, .. } => {
                self.state.borrow_mut().anchor_match = Some(matched);
                if !matched {
                    let mut state = self.state.borrow_mut();
                    if state.notice.is_none() {
                        state.notice = Some(
                            "The comment passage could not be verified; showing its section".into(),
                        );
                    }
                }
            }
            epub::Message::Ready { .. } => {
                let config = {
                    let state = self.state.borrow();
                    let (Some(book), Some(reading)) = (&state.book, &state.reading) else {
                        return;
                    };
                    json!({"base":if state.supplement{String::new()}else{format!("/{}/{}",(book.spine_node_index+1)*2,(state.current+1)*2)},"cfi":state.cfi,"quote":state.quote,"fragment":state.fragment,"end":state.end,"theme":reading.theme,"flow":reading.flow,"fontScale":reading.font_scale,"fixed":book.layout=="pre-paginated"})
                };
                self.javascript(format!("StudioReader.init({config})"));
            }
            epub::Message::LaidOut { .. } => {
                self.state.borrow_mut().ready = true;
                self.section_label();
                self.find();
                self.controls();
            }
            epub::Message::Position { cfi, fraction, .. } => {
                let mut state = self.state.borrow_mut();
                let Some(book) = state.book.clone() else {
                    return;
                };
                if state.supplement {
                    return;
                }
                let current = state.current;
                let Some(reading) = &mut state.reading else {
                    return;
                };
                reading.cfi = cfi;
                reading.chapter_label = book.spine[current].label.clone();
                reading.progress =
                    ((current as f64 + fraction) / book.spine.len() as f64).clamp(0.0, 1.0);
                state.version += 1;
                state.dirty = true;
                drop(state);
                self.section_label();
                self.defer_save();
            }
            epub::Message::Edge { direction, .. } => {
                let target = {
                    let state = self.state.borrow();
                    let Some(book) = &state.book else { return };
                    if state.supplement {
                        Some(state.current)
                    } else if direction > 0 {
                        ((state.current + 1)..book.spine.len()).find(|i| book.spine[*i].linear)
                    } else {
                        (0..state.current).rev().find(|i| book.spine[*i].linear)
                    }
                };
                if let Some(target) = target {
                    self.chapter(target, direction < 0);
                } else {
                    self.status_message(if direction < 0 {
                        "Start of book"
                    } else {
                        "End of book"
                    });
                }
            }
        }
    }
    fn section_label(&self) {
        let state = self.state.borrow();
        if let Some(notice) = state.notice.as_deref() {
            self.status_message(notice);
        } else if state.supplement {
            self.status_message("Supplement");
        } else if let Some(section) = state.book.as_ref().and_then(|b| b.spine.get(state.current)) {
            for property in ["label", "tooltip-text"] {
                i18n_ui::literal(&self.status, property, &section.label);
            }
        } else {
            self.status_message("Read only");
        }
    }
    fn literal_message(&self, text: &str) {
        for property in ["label", "tooltip-text"] {
            i18n_ui::literal(&self.status, property, text);
        }
    }
    fn status_message(&self, source: &str) {
        self.message_format(source, &[]);
    }
    fn message_format(&self, source: &str, variables: &[(&str, &str)]) {
        for property in ["label", "tooltip-text"] {
            i18n_ui::bind_format(&self.status, property, source, variables);
        }
    }
    fn javascript(self: &Rc<Self>, script: String) {
        let web = self.surface.borrow().clone();
        let generation = self.state.borrow().generation;
        let weak = Rc::downgrade(self);
        if let Some(web) = web {
            glib::spawn_future_local(async move {
                if let Err(error) = web
                    .evaluate_javascript_future(&script, Some(WORLD), None)
                    .await
                {
                    if let Some(reader) = weak.upgrade() {
                        if reader.state.borrow().generation == generation {
                            reader.message_format(
                                "EPUB content: {error}",
                                &[("error", &error.to_string())],
                            );
                        }
                    }
                }
            });
        }
    }
    fn find(&self) {
        if let Some(web) = self.surface.borrow().as_ref() {
            let Some(find) = web.find_controller() else {
                return;
            };
            let query = self.search.text();
            if query.is_empty() {
                find.search_finish();
            } else {
                let mut options = webkit6::FindOptions::WRAP_AROUND;
                if !self.case.is_active() {
                    options |= webkit6::FindOptions::CASE_INSENSITIVE;
                }
                find.search(&query, options.bits(), 1000);
            }
        }
    }
    fn search_document(&self, owner: &(String, String, String)) -> Option<String> {
        let state = self.state.borrow();
        let book = state.book.as_ref()?;
        if state.recovering
            || (&book.root, &book.path, &book.hash) != (&owner.0, &owner.1, &owner.2)
        {
            return None;
        }
        state.id.clone()
    }
    fn search_book(self: &Rc<Self>) {
        let existing = self.book_search.borrow().clone();
        if let Some(panel) = existing {
            if panel.is_open() {
                panel.dialog.present(self.parent.upgrade().as_ref());
                return;
            }
            self.book_search.borrow_mut().take();
            panel.dialog.force_close();
        }
        let (hash, total, title, owner) = {
            let state = self.state.borrow();
            let (Some(_), Some(book)) = (&state.id, &state.book) else {
                return;
            };
            if state.recovering {
                return;
            }
            (
                book.hash.clone(),
                book.spine.len(),
                book.title.clone(),
                (book.root.clone(), book.path.clone(), book.hash.clone()),
            )
        };
        let weak = Rc::downgrade(self);
        let selected = Rc::downgrade(self);
        let request_owner = owner.clone();
        let panel = crate::epub_search_ui::Panel::new(
            hash,
            total,
            &title,
            &self.search.text(),
            self.case.is_active(),
            move |start, query, case_sensitive| {
                let reader = weak.upgrade()?;
                let id = reader.search_document(&request_owner)?;
                reader.request("search", json!({"operation":"search","id":id,"query":query,"caseSensitive":case_sensitive,"start":start}))
            },
            move |section, query, case_sensitive| {
                if let Some(reader) = selected.upgrade() {
                    let valid = {
                        let state = reader.state.borrow();
                        reader.search_document(&owner).is_some()
                            && state
                                .book
                                .as_ref()
                                .and_then(|b| b.spine.get(section.chapter))
                                .is_some_and(|c| c.href == section.href)
                    };
                    if valid {
                        reader.search.set_text(query);
                        reader.case.set_active(case_sensitive);
                        reader.search_toggle.set_active(true);
                        reader.chapter(section.chapter, false);
                    }
                }
            },
        );
        let weak = Rc::downgrade(self);
        let closing = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(reader) = weak.upgrade() {
                let current = closing.upgrade().is_some_and(|closing| {
                    reader
                        .book_search
                        .borrow()
                        .as_ref()
                        .is_some_and(|panel| Rc::ptr_eq(panel, &closing))
                });
                if current {
                    reader.book_search.borrow_mut().take();
                }
            }
        });
        panel.dialog.present(self.parent.upgrade().as_ref());
        self.book_search.replace(Some(panel));
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_translation(self: &Rc<Self>) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !self.smoke_ready() {
            if start.elapsed() > std::time::Duration::from_secs(25) {
                return Err(format!(
                    "Translation EPUB did not open: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        let web = self
            .surface
            .borrow()
            .clone()
            .ok_or("Missing translation EPUB content")?;
        web.evaluate_javascript_future("(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById('translation'));const s=getSelection();s.removeAllRanges();s.addRange(r)})()",Some(WORLD),None).await.map_err(|e|e.to_string())?;
        glib::timeout_future(std::time::Duration::from_millis(50)).await;
        web.evaluate_javascript_future("getSelection().removeAllRanges()", Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        self.translate_selection();
        Ok(())
    }
    fn translate_selection(self: &Rc<Self>) {
        let Some(action) = crate::translation_ui::capture() else {
            self.status_message("Select a session before translating");
            return;
        };
        let Some(web) = self.surface.borrow().clone() else {
            return;
        };
        let (generation, epoch, resource) = {
            let state = self.state.borrow();
            if !state.ready {
                return;
            }
            (
                state.generation,
                state.document_epoch,
                state.resource.clone(),
            )
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let selected = web
                .evaluate_javascript_future(
                    "StudioReader.translationSelection()",
                    Some(WORLD),
                    None,
                )
                .await;
            let Some(reader) = weak.upgrade() else {
                return;
            };
            let current = {
                let state = reader.state.borrow();
                state.generation == generation
                    && state.document_epoch == epoch
                    && state.resource == resource
                    && state.ready
            };
            if !current {
                return;
            }
            match selected {
                Ok(value) if value.is_string() => action(value.to_str().to_string()),
                Ok(_) => reader.status_message("Select a smaller passage to translate"),
                Err(error) => reader.literal_message(&error.to_string()),
            }
        });
    }
    fn comment_selection(self: &Rc<Self>) {
        let Some(target) = self.comments.borrow().clone() else {
            return;
        };
        let Some(owner) = (target.owner)() else {
            self.status_message("Select a session before commenting");
            return;
        };
        if self.commenting.replace(true) {
            return;
        }
        let Some(web) = self.surface.borrow().clone() else {
            self.commenting.set(false);
            return;
        };
        let (generation, resource, epoch) = {
            let state = self.state.borrow();
            (
                state.generation,
                state.resource.clone(),
                state.document_epoch,
            )
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result = web
                .evaluate_javascript_future("StudioReader.commentSelection()", Some(WORLD), None)
                .await;
            let Some(reader) = weak.upgrade() else {
                return;
            };
            if reader.state.borrow().generation != generation
                || reader.state.borrow().resource != resource
                || reader.state.borrow().document_epoch != epoch
            {
                return;
            }
            reader.commenting.set(false);
            reader.controls();
            let result: Result<crate::annotations::Capture, String> = (|| {
                let raw = result.map_err(|e| e.to_string())?.to_str();
                let selection = epub::selection(&raw)?;
                let state = reader.state.borrow();
                let book = state.book.as_ref().ok_or("Book is closed")?;
                let uri =
                    url::Url::parse(&selection.href).map_err(|_| "Invalid EPUB selection URL")?;
                if uri.scheme() != "studio-epub"
                    || uri.host_str() != state.id.as_deref()
                    || uri.path().trim_start_matches('/') != state.resource
                {
                    return Err("EPUB section changed; select the passage again".into());
                }
                let label = book
                    .spine
                    .get(state.current)
                    .map_or("Supplement", |s| s.label.as_str());
                crate::annotations::Capture{excerpt:selection.excerpt,source:json!({"provider":"epub","version":1,"anchor":{"root":book.root,"filePath":book.path,"bookHash":book.hash,"cfiRange":selection.cfi_range,"href":state.resource,"chapterLabel":crate::annotations::bounded(label,1024)}})}.validate()
            })();
            match result.and_then(|capture| (target.capture)(owner, capture)) {
                Ok(()) => reader.status_message("Comment added"),
                Err(error) => reader.literal_message(&error),
            }
        });
    }
    fn copy_selection(self: &Rc<Self>) {
        if self.copying.get() || !self.state.borrow().ready {
            return;
        }
        let Some(web) = self.surface.borrow().clone() else {
            return;
        };
        let (generation, epoch) = {
            let state = self.state.borrow();
            (state.generation, state.document_epoch)
        };
        self.copying.set(true);
        self.controls();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            // Selection needs the DOM; GTK owns the clipboard and the explicit action.
            let selected = web
                .evaluate_javascript_future("StudioReader.copySelection()", Some(WORLD), None)
                .await;
            if let Some(reader) = weak.upgrade() {
                let current = {
                    let state = reader.state.borrow();
                    state.generation == generation && state.document_epoch == epoch
                };
                if !current {
                    return;
                }
                reader.copying.set(false);
                reader.controls();
                match selected {
                    Ok(value) if value.is_string() => {
                        let text = value.to_str();
                        if text.is_empty() {
                            reader.status_message("Select text to copy");
                        } else if text.len() > MAX_COPY_BYTES {
                            reader.status_message("Select a smaller passage to copy");
                        } else {
                            let bytes = glib::Bytes::from_owned(text.as_bytes().to_vec());
                            let provider = gtk::gdk::ContentProvider::new_union(&[
                                gtk::gdk::ContentProvider::for_value(&text.to_string().to_value()),
                                gtk::gdk::ContentProvider::for_bytes(
                                    "text/plain;charset=utf-8",
                                    &bytes,
                                ),
                                gtk::gdk::ContentProvider::for_bytes("text/plain", &bytes),
                            ]);
                            match reader.copy.clipboard().set_content(Some(&provider)) {
                                Ok(()) => reader.status_message("Copied selected text"),
                                Err(error) => reader.message_format(
                                    "Unable to copy: {error}",
                                    &[("error", &error.to_string())],
                                ),
                            }
                        }
                    }
                    Ok(_) => reader.status_message("Select a smaller passage to copy"),
                    Err(error) => reader.message_format(
                        "Unable to copy: {error}",
                        &[("error", &error.to_string())],
                    ),
                }
            }
        });
    }
    fn defer_save(self: &Rc<Self>) {
        if let Some(timer) = self.save_timer.borrow_mut().take() {
            timer.remove();
        }
        let weak = Rc::downgrade(self);
        *self.save_timer.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(600),
            move || {
                if let Some(reader) = weak.upgrade() {
                    reader.save_timer.borrow_mut().take();
                    reader.save();
                }
            },
        ));
    }
    fn save(&self) {
        let update = self.update();
        let body = {
            let mut state = self.state.borrow_mut();
            if state.saving || !state.dirty {
                return;
            }
            let Some(id) = &state.id else { return };
            let body = json!({"operation":"save","id":id,"update":update});
            state.saving = true;
            state.dirty = false;
            body
        };
        self.request("save", body);
    }
    fn preference_changed(self: &Rc<Self>) {
        if self.updating.get() {
            return;
        }
        let config = {
            let mut state = self.state.borrow_mut();
            let Some(reading) = &mut state.reading else {
                return;
            };
            reading.font_scale = self.font.value() / 100.0;
            reading.theme = ["light", "sepia", "dark"]
                .get(self.theme.selected() as usize)
                .unwrap_or(&"light")
                .to_string();
            reading.flow = if self.flow.selected() == 1 {
                "paginated"
            } else {
                "scrolled"
            }
            .into();
            reading.toc_open = self.sidebar.reveals_child();
            let config =
                json!({"fontScale":reading.font_scale,"theme":reading.theme,"flow":reading.flow});
            state.version += 1;
            state.dirty = true;
            config
        };
        self.javascript(format!("StudioReader.configure({config})"));
        self.defer_save();
    }
    pub fn event(self: &Rc<Self>, response: Response) {
        if response.kind == "close" {
            let owned = self
                .close_pending
                .borrow_mut()
                .remove(&(response.generation, response.serial));
            if !owned {
                return;
            }
            let closing = self.closing_window.get() && self.close_pending.borrow().is_empty();
            if let Err(error) = response.result {
                if let Some(parent) = self.parent.upgrade() {
                    let dialog = adw::MessageDialog::builder()
                        .transient_for(&parent)
                        .heading("Reading position wasn’t saved")
                        .body(&error)
                        .build();
                    i18n_ui::bind(&dialog, "heading", "Reading position wasn’t saved");
                    dialog.add_response("close", "Close");
                    i18n_ui::response(&dialog, "close", "Close");
                    if closing {
                        let parent = parent.clone();
                        dialog.connect_response(None, move |_, _| {
                            parent.close();
                        });
                    }
                    dialog.present();
                }
            } else if closing {
                if let Some(parent) = self.parent.upgrade() {
                    parent.close();
                }
            }
            if !self.closing_window.get() && self.close_pending.borrow().is_empty() {
                let next = self.pending_open.borrow_mut().take();
                if let Some(target) = next {
                    self.begin_open(target);
                }
            }
            return;
        }
        if response.generation != self.state.borrow().generation {
            if let Ok(Reply::Open(book)) = response.result {
                self.request("close", json!({"operation":"close","id":book.document_id}));
            }
            return;
        }
        if response.kind == "search" {
            let panel = self.book_search.borrow().clone();
            if let Some(panel) = panel {
                let result = response.result.and_then(|reply| match reply {
                    Reply::Search(page) => Ok(page),
                    _ => Err("Invalid EPUB search reply".into()),
                });
                panel.event(response.serial, result);
            }
            return;
        }
        if response.kind == "asset" {
            if let Some(request) = self.active.borrow_mut().remove(&response.serial) {
                match response.result {
                    Ok(Reply::Asset(asset)) => {
                        let bytes = glib::Bytes::from_owned(asset.bytes);
                        let stream = gio::MemoryInputStream::from_bytes(&bytes);
                        let result = webkit6::URISchemeResponse::new(&stream, bytes.len() as i64);
                        result.set_content_type(&asset.mime);
                        let headers = webkit6::soup::MessageHeaders::new(
                            webkit6::soup::MessageHeadersType::Response,
                        );
                        headers.append("Content-Security-Policy", CSP);
                        headers.append("Content-Type", &asset.mime);
                        headers.append("X-Content-Type-Options", "nosniff");
                        headers.append("Cache-Control", "no-store");
                        result.set_http_headers(headers);
                        request.finish_with_response(&result);
                    }
                    Err(error) => {
                        Self::fail_request(&request, &error);
                        self.literal_message(&error);
                    }
                    _ => Self::fail_request(&request, "Invalid EPUB resource reply"),
                }
            }
            self.pump();
            return;
        }
        match response.result {
            Err(error) => {
                if response.kind == "resume" {
                    let mut state = self.state.borrow_mut();
                    state.resume_pending = false;
                    state.id = None;
                    drop(state);
                    if let Some(panel) = self.book_search.borrow().as_ref() {
                        panel.revalidation_failed(&error);
                    }
                }
                if response.kind == "save" {
                    self.state.borrow_mut().saving = false;
                }
                self.literal_message(&error);
                if response.kind == "open" {
                    i18n_ui::bind(&self.placeholder, "title", "Unable to open EPUB");
                    self.placeholder.set_description(Some(&error));
                }
            }
            Ok(Reply::Open(book)) => {
                if self.state.borrow().id.as_deref() != Some(&book.document_id) {
                    return;
                }
                let reading = book.reading(adw::StyleManager::default().is_dark());
                let index = epub::cfi_chapter(&reading.cfi)
                    .filter(|i| *i < book.spine.len())
                    .unwrap_or(0);
                let mut href = book.spine[index].href.clone();
                let mut cfi = reading.cfi.clone();
                if response.kind == "resume" {
                    let mut state = self.state.borrow_mut();
                    state.resume_pending = false;
                    state.recovering = false;
                    if state.supplement {
                        href = state.resource.clone();
                        if !state.fragment.is_empty() {
                            href.push('#');
                            href.push_str(&state.fragment);
                        }
                        cfi = state.cfi.clone();
                    }
                    drop(state);
                    if let Some(web) = self.surface.borrow_mut().take() {
                        web.stop_loading();
                        web.terminate_web_process();
                        self.content.remove(&web);
                    }
                }
                let comment = self.state.borrow_mut().comment.take();
                if let Some((anchor, quote)) = comment {
                    let same = !anchor.book_hash.is_empty() && anchor.book_hash == book.hash;
                    let target =
                        epub::cfi_chapter(&anchor.cfi_range).filter(|i| *i < book.spine.len());
                    if same && target.is_some() {
                        href = book.spine[target.unwrap()].href.clone();
                        cfi = anchor.cfi_range;
                        self.state.borrow_mut().quote = quote;
                    } else {
                        cfi.clear();
                        self.state.borrow_mut().anchor_match = Some(false);
                        if !anchor.href.is_empty() {
                            href = book
                                .spine
                                .iter()
                                .map(|s| s.href.as_str())
                                .chain(book.toc.iter().map(|s| s.href.as_str()))
                                .find(|s| {
                                    *s == anchor.href || s.ends_with(&format!("/{}", anchor.href))
                                })
                                .map(str::to_owned)
                                .unwrap_or(anchor.href);
                        }
                        self.state.borrow_mut().notice=Some(if same{"The comment has no verified text range; showing its section"}else{"Book changed or its edition is unverified; showing the section without a range highlight"}.into());
                    }
                }
                self.updating.set(true);
                self.font.set_value(reading.font_scale * 100.0);
                self.theme.set_selected(match reading.theme.as_str() {
                    "sepia" => 1,
                    "dark" => 2,
                    _ => 0,
                });
                self.flow
                    .set_selected(u32::from(reading.flow == "paginated"));
                self.flow.set_sensitive(book.layout != "pre-paginated");
                self.sidebar.set_reveal_child(reading.toc_open);
                self.updating.set(false);
                i18n_ui::literal(&self.title, "title", &book.title);
                i18n_ui::literal(&self.title, "subtitle", &book.author);
                let labels = book
                    .toc
                    .iter()
                    .map(|h| format!("{}{}", "  ".repeat(h.depth), h.label))
                    .collect::<Vec<_>>();
                let refs = labels.iter().map(String::as_str).collect::<Vec<_>>();
                self.toc.splice(0, self.toc.n_items(), &refs);
                if book.toc_limited {
                    self.status_message("Contents limited to 2,000 entries");
                }
                if response.kind == "resume" {
                    if let Some(panel) = self.book_search.borrow().as_ref() {
                        panel.gateway_restored(&book.hash, book.spine.len());
                    }
                }
                let mut state = self.state.borrow_mut();
                state.book = Some(Arc::new(book));
                state.reading = Some(reading);
                drop(state);
                self.surface();
                self.navigate(&href, &cfi, false);
            }
            Ok(Reply::Saved) => {
                let mut state = self.state.borrow_mut();
                state.saving = false;
                let dirty = state.dirty;
                drop(state);
                if dirty {
                    self.defer_save();
                }
            }
            _ => {}
        }
        self.controls();
    }
    fn controls(&self) {
        let state = self.state.borrow();
        // Layout and save replies arrive while a user may be typing a new
        // section. Refresh only the value we last displayed; Enter commits
        // navigation and leaving the field cancels an unsubmitted value.
        let unchanged = self.position.text().as_str() == self.position_display.borrow().as_str();
        let value = (state.current + 1).to_string();
        self.position_display.replace(value.clone());
        if unchanged {
            self.position.set_text(&value);
        }
        self.total.set_text(&format!(
            "/ {}",
            state.book.as_ref().map_or(0, |b| b.spine.len())
        ));
        self.previous.set_sensitive(state.ready);
        self.next.set_sensitive(state.ready);
        self.copy.set_sensitive(state.ready && !self.copying.get());
        self.comment
            .set_sensitive(state.ready && !self.commenting.get());
    }
    fn reset_position(&self) {
        let value = (self.state.borrow().current + 1).to_string();
        self.position_display.replace(value.clone());
        self.position.set_text(&value);
    }
    fn choose(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open EPUB")
            .accept_label("Open")
            .build()
            .ui_text("accept-label", "Open")
            .ui_text("title", "Open EPUB");
        let filter = gtk::FileFilter::new();
        i18n_ui::bind(&filter, "name", "EPUB books");
        filter.add_suffix("epub");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let weak = Rc::downgrade(self);
        dialog.open(
            self.parent.upgrade().as_ref(),
            gio::Cancellable::NONE,
            move |result| {
                if let (Some(reader), Ok(file)) = (weak.upgrade(), result) {
                    if let Some(path) = file.path() {
                        if let Some(root) = path.parent() {
                            reader.open(&root.to_string_lossy(), &path.to_string_lossy());
                        }
                    }
                }
            },
        );
    }
}
#[cfg(debug_assertions)]
impl Epub {
    pub async fn smoke_book_search(
        self: &Rc<Self>,
        project: &std::path::Path,
        capture: impl Fn(&adw::Dialog, &str) -> Result<(), String>,
    ) -> Result<Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(30) {
                    return Err("EPUB book search navigation timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            Ok(())
        }
        until(|| self.state.borrow().ready).await?;
        if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some() {
            until(|| {
                self.parent
                    .upgrade()
                    .is_some_and(|parent| parent.is_active())
            })
            .await?;
        }
        if self.state.borrow().current != 0 {
            return Err("Book fixture did not begin in its first section".into());
        }
        let ticks = Rc::new(Cell::new(0u64));
        let gap = Rc::new(Cell::new(0u128));
        let last = Rc::new(Cell::new(std::time::Instant::now()));
        let (t, g, l) = (ticks.clone(), gap.clone(), last.clone());
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            t.set(t.get() + 1);
            g.set(
                g.get()
                    .max(l.replace(std::time::Instant::now()).elapsed().as_millis()),
            );
            glib::ControlFlow::Continue
        });
        struct Beat(Option<glib::SourceId>);
        impl Drop for Beat {
            fn drop(&mut self) {
                if let Some(timer) = self.0.take() {
                    timer.remove();
                }
            }
        }
        let _timer = Beat(Some(timer));
        self.search_toggle.set_active(true);
        self.book_search_button.emit_clicked();
        let panel = self
            .book_search
            .borrow()
            .clone()
            .ok_or("Book search button did not open its dialog")?;
        let found = Rc::new(Cell::new(0u32));
        let observed = found.clone();
        let web = self.surface.borrow().clone().unwrap();
        let signal = web
            .find_controller()
            .unwrap()
            .connect_found_text(move |_, count| observed.set(count));
        let (chapter, first_page_ms) = panel
            .smoke_test(|name| capture(&panel.dialog, name))
            .await?;
        until(|| {
            self.state.borrow().ready && self.state.borrow().current == chapter && found.get() > 0
        })
        .await?;
        web.find_controller().unwrap().disconnect(signal);
        if chapter != 17 || self.search.text().as_str() != "needle" {
            return Err("Book search did not open the selected unopened section".into());
        }
        self.book_search_button.emit_clicked();
        let old = self
            .book_search
            .borrow()
            .clone()
            .ok_or("Book search did not reopen")?;
        let generation = self.state.borrow().generation;
        let serial = old.smoke_pending().ok_or("Book search did not start")?;
        self.open(
            &project.to_string_lossy(),
            &project.join("replacement.epub").to_string_lossy(),
        );
        until(|| {
            self.state.borrow().ready
                && self
                    .state
                    .borrow()
                    .book
                    .as_ref()
                    .is_some_and(|book| book.spine.len() == 3)
        })
        .await?;
        if self.book_search.borrow().is_some() || old.dialog.is_mapped() {
            return Err("Replacing a book retained its old search dialog".into());
        }
        self.book_search_button.emit_clicked();
        let new = self.book_search.borrow().clone().unwrap();
        self.event(Response {
            generation,
            serial,
            kind: "search",
            result: Err("Old book search failure".into()),
        });
        new.smoke_empty().await?;
        new.dialog.close();
        Ok(
            json!({"ok":true,"wholeBook":true,"adaptiveSearchLayout":true,"unopenedSections":true,"boundedResults":true,"previousContinue":true,"caseSensitive":true,"skippedSectionNotice":true,"stopRejectsLateReply":true,"renderedMatchNavigation":true,"bookReplacementIsolation":true,"firstResultPageMs":first_page_ms,"mainLoopTicks":ticks.get(),"maximumMainLoopGapIncludingSnapshotMs":gap.get()}),
        )
    }
}
