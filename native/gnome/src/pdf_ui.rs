//! Native read-only PDF controls and textures. No PDF engine or parsing in GTK.
#[cfg(debug_assertions)]
#[path = "pdf_language_acceptance.rs"]
mod language_acceptance;
#[cfg(debug_assertions)]
#[path = "pdf_recovery_acceptance.rs"]
mod recovery_acceptance;
#[cfg(debug_assertions)]
#[path = "pdf_unlock_acceptance.rs"]
mod unlock_acceptance;
use crate::i18n_ui::UiText;
/// Presentation only: page destinations are interface labels; URLs are opaque data.
fn link_text(
    object: &impl IsA<glib::Object>,
    property: &'static str,
    target: &crate::pdf_navigation::Target,
) {
    match target {
        crate::pdf_navigation::Target::Page { page } => crate::i18n_ui::bind_format(
            object,
            property,
            "Go to page {page}",
            &[("page", &(page + 1).to_string())],
        ),
        crate::pdf_navigation::Target::Url { url } => {
            crate::i18n_ui::literal(object, property, url)
        }
    }
}
use crate::{
    pdf::{Reply, Response},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;
#[derive(Default)]
struct State {
    generation: u64,
    serial: u64,
    view: u64,
    search: u64,
    selection_serial: u64,
    selecting: bool,
    selection: Option<crate::pdf::Selection>,
    hash: String,
    comment: Option<(crate::annotations::PdfAnchor, String)>,
    comment_verified: Option<bool>,
    id: Option<String>,
    root: String,
    path: String,
    page: usize,
    pages: usize,
    next: Option<usize>,
    query: String,
    search_partial: bool,
    initial_page: usize,
    busy: bool,
    pending_page: Option<usize>,
    links: Vec<crate::pdf_navigation::Link>,
    recovering: bool,
}
struct LinkPress {
    generation: u64,
    view: u64,
    start: [f64; 2],
    link: crate::pdf_navigation::Link,
}
pub struct Pdf {
    pub widget: adw::ToolbarView,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    title: adw::WindowTitle,
    picture: crate::scaled_picture::ScaledPicture,
    highlights: gtk::DrawingArea,
    drag: Cell<Option<[[f64; 2]; 2]>>,
    drag_origin: Cell<Option<[f64; 2]>>,
    canvas: gtk::ScrolledWindow,
    text: gtk::TextView,
    views: gtk::Stack,
    preview: gtk::ToggleButton,
    text_mode: gtk::ToggleButton,
    fit: gtk::ToggleButton,
    zoom: Cell<f64>,
    size: Cell<(i32, i32)>,
    copy: gtk::Button,
    comment: gtk::Button,
    previous: gtk::Button,
    next: gtk::Button,
    page: gtk::Entry,
    total: gtk::Label,
    status: gtk::Label,
    outline: gtk::ListBox,
    hits: gtk::ListBox,
    sidebar: gtk::Revealer,
    panels: gtk::Stack,
    links: gio::ListStore,
    link_view: gtk::ListView,
    link_button: gtk::ToggleButton,
    link_press: RefCell<Option<LinkPress>>,
    uri_request: RefCell<Option<gio::Cancellable>>,
    search_bar: gtk::SearchEntry,
    search_reveal: gtk::Revealer,
    case: gtk::ToggleButton,
    more: gtk::Button,
    state: RefCell<State>,
    password_prompt: RefCell<Option<adw::AlertDialog>>,
    updating: Cell<bool>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
    pub comments: RefCell<Option<Rc<crate::annotations_ui::CaptureTarget>>>,
}
impl Pdf {
    #[cfg(debug_assertions)]
    pub async fn smoke_comment(self: &Rc<Self>, capture: impl Fn()) -> Result<Value, String> {
        async fn wait(pdf: &Pdf, page: usize) -> Result<(), String> {
            let now = std::time::Instant::now();
            while pdf.state.borrow().pages != 3
                || pdf.state.borrow().page != page
                || pdf.state.borrow().busy
            {
                if now.elapsed() > std::time::Duration::from_secs(20) {
                    return Err(format!("PDF comment page timed out: {}", pdf.status.text()));
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        wait(self, 0).await?;
        self.load(1);
        wait(self, 1).await?;
        self.preview.set_active(true);
        capture();
        glib::timeout_future(std::time::Duration::from_millis(50)).await;
        let select = || -> Result<(), String> {
            let (x, y, w, h) = self.view_rect().ok_or("PDF preview has no image bounds")?;
            let start = self
                .point(x + w * 63.0 / 595.0, y + h * 134.0 / 842.0, false)
                .ok_or("PDF selection start missing")?;
            let end = self
                .point(x + w * 114.0 / 595.0, y + h * 134.0 / 842.0, false)
                .ok_or("PDF selection end missing")?;
            self.select_points([start, end]);
            Ok(())
        };
        select()?;
        // A page change invalidates a pending selection before its worker replies.
        self.load(2);
        wait(self, 2).await?;
        if self.state.borrow().selection.is_some() {
            return Err("A stale PDF selection reached another page".into());
        }
        self.load(1);
        wait(self, 1).await?;
        let gap = Rc::new(Cell::new(0u128));
        let ticks = Rc::new(Cell::new(0usize));
        let last = Rc::new(Cell::new(std::time::Instant::now()));
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), {
            let (gap, ticks, last) = (gap.clone(), ticks.clone(), last.clone());
            move || {
                let now = std::time::Instant::now();
                gap.set(
                    gap.get()
                        .max(now.duration_since(last.replace(now)).as_millis()),
                );
                ticks.set(ticks.get() + 1);
                glib::ControlFlow::Continue
            }
        });
        let now = std::time::Instant::now();
        let result: Result<(), String> = async {
            select()?;
            while self.state.borrow().selecting {
                if now.elapsed() > std::time::Duration::from_secs(20) {
                    return Err("PDF selection worker timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        .await;
        let elapsed = now.elapsed().as_millis();
        timer.remove();
        result?;
        if self
            .state
            .borrow()
            .selection
            .as_ref()
            .is_none_or(|s| s.excerpt != "Page" || s.rects.is_empty())
        {
            return Err(format!(
                "PDF Preview selection failed: {}",
                self.status.text()
            ));
        }
        self.copy.emit_clicked();
        if self
            .copy
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some("Page")
        {
            return Err("PDF Preview selected Copy failed".into());
        }
        if !self.comment.is_sensitive() {
            return Err("PDF Preview comment action stayed disabled".into());
        }
        self.comment.emit_clicked();
        Ok(
            json!({"selectionMs":elapsed,"selectionMainLoopTicks":ticks.get(),"selectionMainLoopMaxGapMs":gap.get(),"stalePageRejected":true,"previewCopy":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_comment_reopened(
        &self,
        expected: bool,
        changed: bool,
        legacy: bool,
    ) -> Result<(), String> {
        let now = std::time::Instant::now();
        while self.state.borrow().comment_verified.is_none()
            || self.state.borrow().busy
            || self.state.borrow().selecting
        {
            if now.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "PDF comment reopening timed out: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        let state = self.state.borrow();
        if state.page != 1
            || state.comment_verified != Some(expected)
            || state.selection.is_some() != (expected && !legacy)
        {
            return Err(format!(
                "PDF comment verification failed: {}",
                self.status.text()
            ));
        }
        if changed && !self.status.text().contains("changed") {
            return Err("Changed PDF was not identified".into());
        }
        if legacy {
            let buffer = self.text.buffer();
            let (a, _) = buffer
                .start_iter()
                .forward_search("Page", gtk::TextSearchFlags::TEXT_ONLY, None)
                .ok_or("Legacy PDF quote is missing")?;
            let tag = buffer
                .tag_table()
                .lookup("pdf-comment")
                .ok_or("Legacy PDF quote has no visible source highlight")?;
            if !self.text_mode.is_active() || !a.has_tag(&tag) {
                return Err("Legacy PDF source trusted unverified regions".into());
            }
        }
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        root: &std::path::Path,
        capture: impl Fn(&str),
    ) -> Result<Value, String> {
        async fn until(check: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !check() {
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err("Native PDF controls timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            Ok(())
        }
        let ticks = Rc::new(Cell::new(0usize));
        let count = ticks.clone();
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        });
        let started = std::time::Instant::now();
        let opened = until(|| self.state.borrow().pages == 3 && !self.state.borrow().busy).await;
        let elapsed = started.elapsed().as_millis();
        timer.remove();
        opened?;
        if self.outline.row_at_index(1).is_none() || self.picture.paintable().is_none() {
            return Err("PDF outline or first raster was missing".into());
        }
        capture("pdf-preview.png");
        // Measure a fresh page after startup separately from the cold window.
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        let page_ticks = Rc::new(Cell::new(0usize));
        let page_gap = Rc::new(Cell::new(0u128));
        let last_tick = Rc::new(Cell::new(std::time::Instant::now()));
        let (count, gap, last) = (page_ticks.clone(), page_gap.clone(), last_tick.clone());
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let now = std::time::Instant::now();
            gap.set(
                gap.get()
                    .max(now.duration_since(last.replace(now)).as_millis()),
            );
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        });
        let started = std::time::Instant::now();
        self.next.emit_clicked();
        let loaded = until(|| self.state.borrow().page == 1 && !self.state.borrow().busy).await;
        let page_elapsed = started.elapsed().as_millis();
        // Include first paint in the responsiveness sample.
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        timer.remove();
        loaded?;
        self.previous.emit_clicked();
        until(|| self.state.borrow().page == 0 && !self.state.borrow().busy).await?;
        self.load(1);
        self.load(0);
        self.load(2);
        until(|| {
            self.state.borrow().page == 2
                && !self.state.borrow().busy
                && self.state.borrow().pending_page.is_none()
        })
        .await?;
        self.outline
            .row_at_index(0)
            .unwrap()
            .child()
            .and_downcast::<gtk::Button>()
            .unwrap()
            .emit_clicked();
        until(|| self.state.borrow().page == 0 && !self.state.borrow().busy).await?;
        self.search_reveal.set_reveal_child(true);
        self.search_bar.set_text("Needle");
        until(|| self.hits.row_at_index(2).is_some() && self.state.borrow().next.is_none()).await?;
        let hit = self
            .hits
            .row_at_index(1)
            .unwrap()
            .downcast::<adw::ActionRow>()
            .unwrap();
        adw::prelude::ActionRowExt::activate(&hit);
        until(|| self.state.borrow().page == 1 && !self.state.borrow().busy).await?;
        let buffer = self.text.buffer();
        let (a, b) = buffer
            .start_iter()
            .forward_search("Needle", gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("PDF text missing")?;
        let tag = buffer
            .tag_table()
            .lookup("pdf-search")
            .ok_or("PDF highlight tag missing")?;
        if !a.has_tag(&tag) {
            return Err("PDF search did not highlight the extracted text".into());
        }
        buffer.select_range(&a, &b);
        self.copy.emit_clicked();
        let copied = self
            .copy
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?;
        if copied.as_deref() != Some("Needle") {
            return Err("PDF selected text copy was lost".into());
        }
        let previous = self.comments.borrow_mut().take();
        let captured = Rc::new(Cell::new(false));
        self.comments
            .replace(Some(Rc::new(crate::annotations_ui::CaptureTarget {
                owner: Box::new(|| Some("private-pdf-fixture".into())),
                capture: Box::new({
                    let captured = captured.clone();
                    move |_, capture| {
                        let anchor = &capture.source["anchor"];
                        if capture.excerpt != "Needle"
                            || anchor["page"] != 2
                            || anchor["documentHash"].as_str().is_none_or(|s| s.is_empty())
                            || anchor["rects"] != json!([])
                            || anchor.get("charRange").is_some()
                        {
                            return Err("PDF Text comment source was incorrect".into());
                        }
                        captured.set(true);
                        Ok(())
                    }
                }),
            })));
        self.comment.emit_clicked();
        self.comments.replace(previous);
        if !captured.get() {
            return Err("PDF Text selection did not reach native comments".into());
        }
        self.smoke_language(&capture).await?;
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        capture("pdf-text-dark.png");
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
        self.open(
            &root.to_string_lossy(),
            &root.join("locked.pdf").to_string_lossy(),
            0,
        );
        self.smoke_replace_password_prompt(root).await?;
        self.smoke_initial_unlock().await?;
        until(|| self.state.borrow().pages == 3 && !self.state.borrow().busy).await?;
        self.open(
            &root.to_string_lossy(),
            &root.join("many.pdf").to_string_lossy(),
            0,
        );
        until(|| self.state.borrow().pages == 70 && !self.state.borrow().busy).await?;
        self.search_bar.set_text("Searchable");
        until(|| self.state.borrow().next == Some(32) && self.more.is_sensitive()).await?;
        self.more.emit_clicked();
        until(|| self.state.borrow().next == Some(64) && self.more.is_sensitive()).await?;
        if self.hits.row_at_index(31).is_none() || self.hits.row_at_index(32).is_some() {
            return Err("PDF search did not keep its bounded result window".into());
        }
        self.more.emit_clicked();
        until(|| self.state.borrow().next.is_none() && self.more.is_sensitive()).await?;
        self.open(
            &root.to_string_lossy(),
            &root.join("sample.pdf").to_string_lossy(),
            1,
        );
        self.search_bar.set_text("Needle");
        self.search(false);
        until(|| {
            self.state.borrow().pages == 3
                && self.state.borrow().page == 1
                && !self.state.borrow().busy
        })
        .await?;
        until(|| self.hits.row_at_index(2).is_some()).await?;
        capture("pdf-final.png");
        Ok(
            json!({"ok":true,"openMs":elapsed,"mainLoopTicks":ticks.get(),"pageMs":page_elapsed,"pageMainLoopTicks":page_ticks.get(),"pageMainLoopMaxGapMs":page_gap.get(),"raster":true,"paging":true,"coalescedPaging":true,"outline":true,"search":true,"highlight":true,"selectedCopy":true,"textComment":true,"password":true,"searchWindows":true,"pageHint":true,"deferredSearch":true,"liveLanguage":true,"literalDataRetained":true,"passwordLiveLanguage":true}),
        )
    }
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let widget = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        let title = adw::WindowTitle::new("PDF", "");
        header.set_title_widget(Some(&title));
        let open = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Open PDF")
            .build()
            .ui_text("tooltip-text", "Open PDF");
        header.pack_start(&open);
        let close = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text("Close PDF")
            .build()
            .ui_text("tooltip-text", "Close PDF");
        header.pack_end(&close);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload PDF")
            .build()
            .ui_text("tooltip-text", "Reload PDF");
        header.pack_end(&reload);
        let contents = gtk::ToggleButton::builder()
            .icon_name("view-list-symbolic")
            .tooltip_text("Contents")
            .build()
            .ui_text("tooltip-text", "Contents");
        header.pack_end(&contents);
        let search_toggle = gtk::ToggleButton::builder()
            .icon_name("edit-find-symbolic")
            .tooltip_text("Search PDF")
            .build()
            .ui_text("tooltip-text", "Search PDF");
        header.pack_end(&search_toggle);
        let modes = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        modes.add_css_class("linked");
        let preview = gtk::ToggleButton::with_label("Preview").ui_text("label", "Preview");
        let text_mode = gtk::ToggleButton::with_label("Text").ui_text("label", "Text");
        text_mode.set_group(Some(&preview));
        preview.set_active(true);
        modes.append(&preview);
        modes.append(&text_mode);
        header.pack_end(&modes);
        widget.add_top_bar(&header);
        let controls = crate::adaptive::toolbar(6);
        controls.set_margin_start(12);
        controls.set_margin_end(12);
        controls.set_margin_bottom(8);
        controls.append(&gtk::Box::builder().hexpand(true).build());
        let minus = gtk::Button::builder()
            .icon_name("zoom-out-symbolic")
            .tooltip_text("Zoom out")
            .build()
            .ui_text("tooltip-text", "Zoom out");
        let plus = gtk::Button::builder()
            .icon_name("zoom-in-symbolic")
            .tooltip_text("Zoom in")
            .build()
            .ui_text("tooltip-text", "Zoom in");
        let fit = gtk::ToggleButton::with_label("Fit width").ui_text("label", "Fit width");
        fit.set_active(true);
        let copy = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy selected or page text")
            .build()
            .ui_text("tooltip-text", "Copy selected or page text");
        controls.append(&minus);
        controls.append(&plus);
        controls.append(&fit);
        controls.append(&copy);
        let comment = gtk::Button::builder()
            .icon_name("document-edit-symbolic")
            .tooltip_text("Comment on selected text")
            .focus_on_click(false)
            .build()
            .ui_text("tooltip-text", "Comment on selected text");
        controls.append(&comment);
        let translate = gtk::Button::builder()
            .icon_name("preferences-desktop-locale-symbolic")
            .tooltip_text("Translate selected text")
            .focus_on_click(false)
            .build()
            .ui_text("tooltip-text", "Translate selected text");
        comment
            .bind_property("sensitive", &translate, "sensitive")
            .sync_create()
            .build();
        controls.append(&translate);
        let link_button = gtk::ToggleButton::builder()
            .icon_name("insert-link-symbolic")
            .tooltip_text("Links on this page")
            .build()
            .ui_text("tooltip-text", "Links on this page");
        link_button.set_group(Some(&contents));
        controls.append(&link_button);
        widget.add_top_bar(&controls);
        let search_bar = gtk::SearchEntry::builder()
            .placeholder_text("Search PDF…")
            .hexpand(true)
            .build()
            .ui_text("placeholder-text", "Search PDF…");
        crate::i18n_ui::accessible_label(&search_bar, "Search PDF");
        let case = gtk::ToggleButton::with_label("Aa");
        crate::i18n_ui::bind(&case, "tooltip-text", "Case sensitive");
        crate::i18n_ui::accessible_label(&case, "Case sensitive");
        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        search_row.set_margin_start(12);
        search_row.set_margin_end(12);
        search_row.set_margin_bottom(8);
        search_row.append(&search_bar);
        search_row.append(&case);
        let search_reveal = gtk::Revealer::builder().child(&search_row).build();
        widget.add_top_bar(&search_reveal);
        let picture = crate::scaled_picture::ScaledPicture::new("PDF page");
        crate::i18n_ui::accessible_label(&picture, "PDF page");
        crate::i18n_ui::bind(&picture.animation_picture(), "alternative-text", "PDF page");
        picture.set_hexpand(true);
        picture.set_focusable(true);
        picture.set_cursor_from_name(Some("text"));
        let overlay = gtk::Overlay::builder().child(&picture).build();
        let highlights = gtk::DrawingArea::builder().can_target(false).build();
        overlay.add_overlay(&highlights);
        let canvas = gtk::ScrolledWindow::builder()
            .child(&overlay)
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let text = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(20)
            .right_margin(20)
            .top_margin(20)
            .bottom_margin(20)
            .build();
        crate::i18n_ui::accessible_label(&text, "PDF text");
        text.add_css_class("studio-content");
        crate::translation_ui::bind(&text);
        let views = gtk::Stack::builder().hexpand(true).vexpand(true).build();
        views.add_named(&canvas, Some("preview"));
        views.add_named(
            &gtk::ScrolledWindow::builder()
                .child(&text)
                .hexpand(true)
                .vexpand(true)
                .build(),
            Some("text"),
        );
        let outline = gtk::ListBox::new();
        outline.add_css_class("navigation-sidebar");
        let hits = gtk::ListBox::new();
        hits.add_css_class("navigation-sidebar");
        let more = gtk::Button::with_label("Next results").ui_text("label", "Next results");
        more.set_visible(false);
        let results = gtk::Box::new(gtk::Orientation::Vertical, 6);
        results.append(&hits);
        results.append(&more);
        let panels = gtk::Stack::new();
        panels.add_named(&outline, Some("outline"));
        panels.add_named(&results, Some("search"));
        let links = gio::ListStore::new::<glib::BoxedAnyObject>();
        let link_selection = gtk::SingleSelection::new(Some(links.clone()));
        link_selection.set_autoselect(false);
        let link_factory = gtk::SignalListItemFactory::new();
        link_factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .lines(4)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .margin_start(12)
                .margin_end(12)
                .margin_top(10)
                .margin_bottom(10)
                .build();
            item.set_child(Some(&label));
        });
        link_factory.connect_bind(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let data = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let link = data.borrow::<crate::pdf_navigation::Link>();
            let label = item.child().and_downcast::<gtk::Label>().unwrap();
            link_text(&label, "label", &link.target);
            link_text(&label, "tooltip-text", &link.target);
        });
        let link_view = gtk::ListView::new(Some(link_selection), Some(link_factory));
        link_view.set_single_click_activate(true);
        panels.add_named(&link_view, Some("links"));
        let sidebar = gtk::Revealer::builder()
            .child(
                &gtk::ScrolledWindow::builder()
                    .child(&panels)
                    .width_request(240)
                    .max_content_width(300)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .build(),
            )
            .transition_type(gtk::RevealerTransitionType::SlideRight)
            .build();
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&sidebar);
        body.append(&views);
        widget.set_content(Some(&body));
        let status = gtk::Label::builder()
            .label("Open a PDF")
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .hexpand(true)
            .build();
        crate::i18n_ui::bind(&status, "label", "Open a PDF");
        status.add_css_class("caption");
        let previous = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Previous page")
            .build()
            .ui_text("tooltip-text", "Previous page");
        let next = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Next page")
            .build()
            .ui_text("tooltip-text", "Next page");
        let page = gtk::Entry::builder()
            .text("1")
            .width_chars(5)
            .max_width_chars(5)
            .max_length(10)
            .input_purpose(gtk::InputPurpose::Digits)
            .build();
        crate::i18n_ui::accessible_label(&page, "PDF page number");
        let total = gtk::Label::new(Some("/ 0"));
        let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bottom.set_margin_start(12);
        bottom.set_margin_end(12);
        bottom.set_margin_top(6);
        bottom.set_margin_bottom(6);
        bottom.append(&status);
        bottom.append(&previous);
        bottom.append(&page);
        bottom.append(&total);
        bottom.append(&next);
        widget.add_bottom_bar(&bottom);
        let pdf = Rc::new(Self {
            widget,
            parent: parent.downgrade(),
            sender,
            title,
            picture,
            highlights,
            drag: Cell::new(None),
            drag_origin: Cell::new(None),
            canvas,
            text,
            views,
            preview,
            text_mode,
            fit,
            zoom: Cell::new(1.0),
            size: Cell::new((0, 0)),
            copy,
            comment,
            previous,
            next,
            page,
            total,
            status,
            outline,
            hits,
            sidebar,
            panels,
            links,
            link_view,
            link_button,
            link_press: RefCell::new(None),
            uri_request: RefCell::new(None),
            search_bar,
            search_reveal,
            case,
            more,
            state: RefCell::default(),
            password_prompt: RefCell::new(None),
            updating: Cell::new(false),
            on_close: RefCell::default(),
            on_open: RefCell::default(),
            comments: RefCell::default(),
        });
        let weak = Rc::downgrade(&pdf);
        pdf.highlights.set_draw_func(move |_, context, _, _| {
            let Some(pdf) = weak.upgrade() else { return };
            let Some((x, y, w, h)) = pdf.view_rect() else {
                return;
            };
            context.set_source_rgba(0.18, 0.55, 0.85, 0.3);
            if let Some([a, b]) = pdf.drag.get() {
                context.rectangle(
                    x + a[0].min(b[0]) * w,
                    y + a[1].min(b[1]) * h,
                    (a[0] - b[0]).abs() * w,
                    (a[1] - b[1]).abs() * h,
                );
                let _ = context.fill();
            } else if let Some(selection) = &pdf.state.borrow().selection {
                for r in &selection.rects {
                    context.rectangle(x + r.x * w, y + r.y * h, r.width * w, r.height * h);
                }
                let _ = context.fill();
            }
        });
        let gesture = gtk::GestureDrag::new();
        gesture.set_button(1);
        let weak = Rc::downgrade(&pdf);
        gesture.connect_drag_begin(move |_, x, y| {
            if let Some(pdf) = weak.upgrade() {
                if pdf.state.borrow().busy {
                    return;
                }
                pdf.picture.grab_focus();
                pdf.clear_selection();
                pdf.state.borrow_mut().comment.take();
                if let Some(point) = pdf.point(x, y, false) {
                    pdf.drag.set(Some([point, point]));
                    pdf.drag_origin.set(Some([x, y]));
                }
            }
        });
        let weak = Rc::downgrade(&pdf);
        gesture.connect_drag_update(move |_, dx, dy| {
            if let Some(pdf) = weak.upgrade() {
                if let (Some([start, _]), Some([x, y])) = (pdf.drag.get(), pdf.drag_origin.get()) {
                    if let Some(end) = pdf.point(x + dx, y + dy, true) {
                        pdf.drag.set(Some([start, end]));
                        pdf.highlights.queue_draw();
                    }
                }
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.link_button.connect_toggled(move |button| {
            if let Some(pdf) = weak.upgrade() {
                if button.is_active() {
                    pdf.panels.set_visible_child_name("links");
                }
                pdf.sidebar.set_reveal_child(button.is_active());
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.link_view.connect_activate(move |_, index| {
            if let Some(pdf) = weak.upgrade() {
                if pdf.state.borrow().busy {
                    return;
                }
                let link = pdf
                    .links
                    .item(index)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .map(|data| data.borrow::<crate::pdf_navigation::Link>().clone());
                if let Some(link) = link {
                    pdf.activate_link(&link);
                }
            }
        });
        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(&pdf);
        motion.connect_motion(move |_, x, y| {
            if let Some(pdf) = weak.upgrade() {
                let link = pdf.link_at(x, y);
                pdf.picture.set_cursor_from_name(Some(if link.is_some() {
                    "pointer"
                } else {
                    "text"
                }));
                pdf.hover_link(link.as_ref());
            }
        });
        let weak = Rc::downgrade(&pdf);
        motion.connect_leave(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.picture.set_cursor_from_name(Some("text"));
                pdf.hover_link(None);
            }
        });
        pdf.picture.add_controller(motion);
        let click = gtk::GestureClick::new();
        click.set_button(1);
        let weak = Rc::downgrade(&pdf);
        click.connect_pressed(move |_, count, x, y| {
            if let Some(pdf) = weak.upgrade() {
                if count != 1 {
                    pdf.link_press.borrow_mut().take();
                    return;
                }
                let link = pdf.link_at(x, y);
                let state = pdf.state.borrow();
                pdf.link_press.replace(link.map(|link| LinkPress {
                    generation: state.generation,
                    view: state.view,
                    start: [x, y],
                    link,
                }));
            }
        });
        let weak = Rc::downgrade(&pdf);
        click.connect_released(move |_, _, x, y| {
            if let Some(pdf) = weak.upgrade() {
                pdf.release_link(x, y);
            }
        });
        let weak = Rc::downgrade(&pdf);
        click.connect_stopped(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.link_press.borrow_mut().take();
            }
        });
        pdf.picture.add_controller(click.clone());
        let weak = Rc::downgrade(&pdf);
        gesture.connect_drag_end(move |_, dx, dy| {
            if let Some(pdf) = weak.upgrade() {
                if let (Some([start, _]), Some([x, y])) = (pdf.drag.take(), pdf.drag_origin.take())
                {
                    if let Some(end) = pdf
                        .point(x + dx, y + dy, true)
                        .filter(|_| dx.hypot(dy) > 4.0)
                    {
                        pdf.select_points([start, end]);
                    }
                }
                pdf.highlights.queue_draw();
            }
        });
        let weak = Rc::downgrade(&pdf);
        gesture.connect_cancel(move |_, _| {
            if let Some(pdf) = weak.upgrade() {
                pdf.drag.set(None);
                pdf.drag_origin.set(None);
                pdf.highlights.queue_draw();
            }
        });
        pdf.picture.add_controller(gesture.clone());
        click.group_with(&gesture);
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&pdf);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gtk::gdk::Key::c && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                if let Some(pdf) = weak.upgrade() {
                    if pdf.copy.is_sensitive() {
                        pdf.copy.emit_clicked();
                    }
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        pdf.picture.add_controller(keys);
        let weak = Rc::downgrade(&pdf);
        pdf.comment.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.comment_selection();
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.text.buffer().connect_has_selection_notify(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.controls();
            }
        });
        let weak = Rc::downgrade(&pdf);
        open.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.choose();
            }
        });
        let weak = Rc::downgrade(&pdf);
        close.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.close();
                if let Some(callback) = pdf.on_close.borrow().as_ref() {
                    callback();
                }
            }
        });
        let weak = Rc::downgrade(&pdf);
        reload.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                let state = pdf.state.borrow();
                let (root, path, page) = (state.root.clone(), state.path.clone(), state.page);
                drop(state);
                pdf.open(&root, &path, page);
            }
        });
        let weak = Rc::downgrade(&pdf);
        contents.connect_toggled(move |button| {
            if let Some(pdf) = weak.upgrade() {
                pdf.panels.set_visible_child_name("outline");
                pdf.sidebar.set_reveal_child(button.is_active());
            }
        });
        let weak = Rc::downgrade(&pdf);
        search_toggle.connect_toggled(move |button| {
            if let Some(pdf) = weak.upgrade() {
                pdf.search_reveal.set_reveal_child(button.is_active());
                if button.is_active() {
                    pdf.search_bar.grab_focus();
                }
            }
        });
        for (button, name) in [(&pdf.preview, "preview"), (&pdf.text_mode, "text")] {
            let weak = Rc::downgrade(&pdf);
            button.connect_toggled(move |button| {
                if button.is_active() {
                    if let Some(pdf) = weak.upgrade() {
                        pdf.views.set_visible_child_name(name);
                        pdf.link_press.borrow_mut().take();
                        pdf.controls();
                    }
                }
            });
        }
        let weak = Rc::downgrade(&pdf);
        pdf.fit.connect_toggled(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.layout();
            }
        });
        for (button, multiplier) in [(&plus, 1.25), (&minus, 0.8)] {
            let weak = Rc::downgrade(&pdf);
            button.connect_clicked(move |_| {
                if let Some(pdf) = weak.upgrade() {
                    pdf.zoom.set((pdf.zoom.get() * multiplier).clamp(0.25, 4.0));
                    pdf.fit.set_active(false);
                    pdf.layout();
                }
            });
        }
        let weak = Rc::downgrade(&pdf);
        pdf.previous.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                let page = pdf.state.borrow().page.saturating_sub(1);
                pdf.load(page);
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.next.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                let page = pdf.state.borrow().page + 1;
                pdf.load(page);
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.page.connect_activate(move |entry| {
            if let Some(pdf) = weak.upgrade() {
                let page = entry
                    .text()
                    .parse::<usize>()
                    .ok()
                    .filter(|page| *page > 0 && *page <= pdf.state.borrow().pages);
                if let Some(page) = page {
                    pdf.load(page - 1);
                } else {
                    pdf.controls();
                }
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.copy.connect_clicked(move |button| {
            if let Some(pdf) = weak.upgrade() {
                if pdf.preview.is_active() {
                    if let Some(selection) = &pdf.state.borrow().selection {
                        button.clipboard().set_text(&selection.excerpt);
                        return;
                    }
                }
                let buffer = pdf.text.buffer();
                let (a, b) = buffer
                    .selection_bounds()
                    .unwrap_or_else(|| (buffer.start_iter(), buffer.end_iter()));
                button.clipboard().set_text(&buffer.text(&a, &b, true));
            }
        });
        let weak = Rc::downgrade(&pdf);
        translate.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.translate_selection();
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.search_bar.connect_search_changed(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.search(false);
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.case.connect_toggled(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.search(false);
            }
        });
        let weak = Rc::downgrade(&pdf);
        pdf.more.connect_clicked(move |_| {
            if let Some(pdf) = weak.upgrade() {
                pdf.search(true);
            }
        });
        pdf.controls();
        pdf
    }
    fn view_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (iw, ih) = self.size.get();
        let (width, height) = (self.picture.width() as f64, self.picture.height() as f64);
        if iw <= 0 || ih <= 0 || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let scale = (width / iw as f64).min(height / ih as f64);
        let (w, h) = (iw as f64 * scale, ih as f64 * scale);
        Some(((width - w) * 0.5, (height - h) * 0.5, w, h))
    }
    fn link_at(&self, x: f64, y: f64) -> Option<crate::pdf_navigation::Link> {
        if !self.preview.is_active() || !self.picture.is_mapped() {
            return None;
        }
        let point = self.point(x, y, false)?;
        let state = self.state.borrow();
        if state.busy || state.id.is_none() {
            return None;
        }
        state
            .links
            .iter()
            .rev()
            .find(|link| link.rects.iter().any(|rect| rect.contains(point)))
            .cloned()
    }
    fn release_link(self: &Rc<Self>, x: f64, y: f64) {
        let Some(press) = self.link_press.borrow_mut().take() else {
            return;
        };
        let valid = self.preview.is_active() && self.picture.is_mapped() && {
            let state = self.state.borrow();
            !state.busy
                && state.id.is_some()
                && state.generation == press.generation
                && state.view == press.view
        };
        if !valid || (x - press.start[0]).hypot(y - press.start[1]) > 4.0 {
            return;
        }
        if self
            .point(x, y, false)
            .is_some_and(|point| press.link.rects.iter().any(|r| r.contains(point)))
        {
            self.activate_link(&press.link);
        }
    }
    fn activate_link(self: &Rc<Self>, link: &crate::pdf_navigation::Link) {
        {
            let state = self.state.borrow();
            if state.busy || state.id.is_none() || !link.valid(state.pages) {
                return;
            }
        }
        match &link.target {
            crate::pdf_navigation::Target::Page { page } => {
                self.load(*page);
            }
            crate::pdf_navigation::Target::Url { url } => {
                let previous = self.uri_request.borrow_mut().take();
                if let Some(previous) = previous {
                    previous.cancel();
                }
                let cancel = gio::Cancellable::new();
                self.uri_request.replace(Some(cancel.clone()));
                let generation = self.state.borrow().generation;
                let view = self.state.borrow().view;
                let weak = Rc::downgrade(self);
                let expected = cancel.clone();
                gtk::UriLauncher::new(url).launch(
                    self.parent.upgrade().as_ref(),
                    Some(&cancel),
                    move |result| {
                        let Some(pdf) = weak.upgrade() else {
                            return;
                        };
                        let current = {
                            let state = pdf.state.borrow();
                            state.generation == generation && state.view == view
                        };
                        let matches = pdf
                            .uri_request
                            .borrow()
                            .as_ref()
                            .is_some_and(|pending| pending == &expected);
                        if !current || !matches {
                            return;
                        }
                        pdf.uri_request.borrow_mut().take();
                        if let Err(error) = result {
                            for property in ["label", "tooltip-text"] {
                                crate::i18n_ui::bind_format(
                                    &pdf.status,
                                    property,
                                    "Could not open PDF link: {error}",
                                    &[("error", &error.to_string())],
                                );
                            }
                        }
                    },
                );
            }
        }
    }
    fn notice(&self, message: &str) {
        for property in ["label", "tooltip-text"] {
            crate::i18n_ui::bind(&self.status, property, message);
        }
    }
    fn diagnostic(&self, error: &str) {
        for property in ["label", "tooltip-text"] {
            crate::i18n_ui::literal(&self.status, property, error);
        }
    }
    fn hover_link(&self, link: Option<&crate::pdf_navigation::Link>) {
        if let Some(link) = link {
            link_text(&self.picture, "tooltip-text", &link.target);
        } else {
            crate::i18n_ui::literal(&self.picture, "tooltip-text", "");
            self.picture.set_tooltip_text(None);
        }
    }
    fn clear_links(&self) {
        self.link_press.borrow_mut().take();
        let pending = self.uri_request.borrow_mut().take();
        if let Some(pending) = pending {
            pending.cancel();
        }
        self.state.borrow_mut().links.clear();
        self.links.remove_all();
        self.picture.set_cursor_from_name(Some("text"));
        self.hover_link(None);
    }
    fn point(&self, x: f64, y: f64, clamp: bool) -> Option<[f64; 2]> {
        let (left, top, w, h) = self.view_rect()?;
        let (x, y) = ((x - left) / w, (y - top) / h);
        if !x.is_finite()
            || !y.is_finite()
            || !clamp && (!(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y))
        {
            return None;
        }
        Some([x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)])
    }
    fn translate_selection(&self) {
        let excerpt = if self.preview.is_active() {
            let state = self.state.borrow();
            if state.busy || state.selecting {
                return;
            }
            state.selection.as_ref().map(|s| {
                s.excerpt
                    .chars()
                    .take(crate::translation_text::MAX_SOURCE)
                    .collect()
            })
        } else {
            crate::translation_ui::selection(self.text.upcast_ref())
        };
        if let Some(excerpt) = excerpt {
            crate::translation_ui::start_selected(excerpt);
        }
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_translation(&self, preview: bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !self.smoke_ready() {
            if start.elapsed() > std::time::Duration::from_secs(25) {
                return Err(format!(
                    "Translation PDF did not open: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        self.preview.set_active(preview);
        if preview {
            let body = {
                let state = self.state.borrow();
                json!({"operation":"select","id":state.id,"page":state.page,"range":[0,4]})
            };
            self.request("select", body);
            while self.state.borrow().selecting {
                if start.elapsed() > std::time::Duration::from_secs(25) {
                    return Err("Translation PDF selection timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            if self
                .state
                .borrow()
                .selection
                .as_ref()
                .is_none_or(|s| s.excerpt != "Page")
            {
                return Err("Translation PDF Preview selected another passage".into());
            }
        } else {
            let buffer = self.text.buffer();
            buffer.select_range(&buffer.start_iter(), &buffer.iter_at_offset(4));
        }
        self.translate_selection();
        Ok(())
    }
    fn clear_selection(&self) {
        let mut state = self.state.borrow_mut();
        state.serial += 1;
        state.selection_serial = state.serial;
        state.selecting = false;
        state.selection = None;
        drop(state);
        self.drag.set(None);
        self.drag_origin.set(None);
        self.highlights.queue_draw();
        self.controls();
    }
    fn select_points(&self, points: [[f64; 2]; 2]) {
        let state = self.state.borrow();
        if state.busy || state.id.is_none() {
            return;
        }
        let body = json!({"operation":"select","id":state.id,"page":state.page,"points":points});
        drop(state);
        self.notice("Selecting text…");
        self.request("select", body);
    }
    fn comment_selection(&self) {
        enum CommentError {
            Ui(&'static str),
            Raw(String),
        }
        impl From<String> for CommentError {
            fn from(error: String) -> Self {
                Self::Raw(error)
            }
        }
        let result = (|| -> Result<(), CommentError> {
            let target = self
                .comments
                .borrow()
                .clone()
                .ok_or(CommentError::Ui("Comments are unavailable"))?;
            let owner =
                (target.owner)().ok_or(CommentError::Ui("Select a session before commenting"))?;
            let state = self.state.borrow();
            if state.busy || state.selecting || state.id.is_none() {
                return Err(CommentError::Ui("Wait for the PDF page and selection"));
            }
            let (excerpt, rects, range) = if self.preview.is_active() {
                let selected = state
                    .selection
                    .as_ref()
                    .ok_or(CommentError::Ui("Drag over text in the page first"))?;
                (
                    selected.excerpt.clone(),
                    selected.rects.clone(),
                    Some(selected.range),
                )
            } else {
                let buffer = self.text.buffer();
                let (a, b) = buffer
                    .selection_bounds()
                    .ok_or(CommentError::Ui("Select text in the Text view first"))?;
                (buffer.text(&a, &b, true).to_string(), Vec::new(), None)
            };
            let mut anchor = json!({"root":state.root,"filePath":state.path,"documentHash":state.hash,"page":state.page+1,"rects":rects});
            if let Some(range) = range {
                anchor["charRange"] = json!(range);
            }
            let capture = crate::annotations::Capture {
                excerpt,
                source: json!({"provider":"pdf","version":1,"anchor":anchor}),
            }
            .validate()?;
            drop(state);
            (target.capture)(owner, capture).map_err(CommentError::Raw)
        })();
        match result {
            Ok(()) => self.notice("Comment added"),
            Err(CommentError::Ui(message)) => self.notice(message),
            Err(CommentError::Raw(error)) => self.diagnostic(&error),
        }
    }
    pub fn open_comment(self: &Rc<Self>, anchor: crate::annotations::PdfAnchor, quote: &str) {
        self.open(&anchor.root, &anchor.file_path, anchor.page - 1);
        self.state.borrow_mut().comment = Some((anchor, quote.into()));
    }
    fn restore_comment(&self) {
        let (anchor, quote, hash, page) = {
            let state = self.state.borrow();
            let Some((anchor, quote)) = &state.comment else {
                return;
            };
            (
                anchor.clone(),
                quote.clone(),
                state.hash.clone(),
                state.page,
            )
        };
        if anchor.page - 1 != page {
            self.state.borrow_mut().comment.take();
            self.state.borrow_mut().comment_verified = Some(false);
            self.notice("Comment page is unavailable; showing the first page");
        } else if anchor.document_hash.is_empty() || anchor.document_hash != hash {
            self.state.borrow_mut().comment.take();
            self.state.borrow_mut().comment_verified = Some(false);
            self.notice(
                "PDF changed or its edition is unverified; showing the page without a highlight",
            );
            self.preview.set_active(true);
        } else if let Some(range) = anchor.char_range {
            let state = self.state.borrow();
            let body = json!({"operation":"select","id":state.id,"page":page,"range":range});
            drop(state);
            self.request("select", body);
        } else {
            self.state.borrow_mut().comment.take();
            self.text_mode.set_active(true);
            let buffer = self.text.buffer();
            let matched = buffer
                .start_iter()
                .forward_search(&quote, gtk::TextSearchFlags::TEXT_ONLY, None)
                .filter(|(_, b)| {
                    b.forward_search(&quote, gtk::TextSearchFlags::TEXT_ONLY, None)
                        .is_none()
                });
            self.state.borrow_mut().comment_verified = Some(matched.is_some());
            if let Some((mut a, b)) = matched {
                let tag = buffer.tag_table().lookup("pdf-comment").unwrap_or_else(|| {
                    let tag = gtk::TextTag::builder()
                        .name("pdf-comment")
                        .background("#9bcaf2")
                        .foreground("#112236")
                        .build();
                    buffer.tag_table().add(&tag);
                    tag
                });
                buffer.apply_tag(&tag, &a, &b);
                buffer.select_range(&a, &b);
                self.text.scroll_to_iter(&mut a, 0.1, false, 0.0, 0.0);
                self.notice("Comment passage · Text view");
            } else {
                self.notice("Comment passage is absent or ambiguous; showing its page");
            }
        }
    }
    fn request(&self, kind: &'static str, body: Value) {
        let (generation, serial) = {
            let mut state = self.state.borrow_mut();
            state.serial += 1;
            let serial = state.serial;
            if kind == "select" {
                state.selection_serial = serial;
                state.selecting = true;
            } else if kind == "search" {
                state.search = serial;
            } else if kind != "close" {
                state.view = serial;
                state.busy = true;
            }
            (state.generation, serial)
        };
        if self
            .sender
            .try_send(Command::Pdf {
                generation,
                serial,
                kind,
                body,
            })
            .is_err()
        {
            self.notice("PDF request queue is busy; try again");
            if kind == "select" {
                self.state.borrow_mut().selecting = false;
            } else if kind == "search" {
                self.more.set_sensitive(true);
            } else if kind != "close" {
                self.state.borrow_mut().busy = false;
            }
        }
        self.controls();
    }
    pub fn open(self: &Rc<Self>, root: &str, path: &str, initial_page: usize) {
        self.retire_password();
        self.clear_links();
        let old = {
            let mut state = self.state.borrow_mut();
            let old = state.id.take();
            let generation = state.generation + 1;
            *state = State {
                generation,
                id: Some(glib::uuid_string_random().to_string()),
                root: root.into(),
                path: path.into(),
                initial_page,
                ..Default::default()
            };
            old
        };
        if let Some(id) = old {
            self.request("close", json!({"operation":"close","id":id}));
        }
        self.picture.clear_texture();
        self.drag.set(None);
        self.highlights.queue_draw();
        self.text.buffer().set_text("");
        self.title.set_title(
            std::path::Path::new(path)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("PDF"),
        );
        self.title.set_subtitle(path);
        self.notice("Opening PDF…");
        self.clear_lists();
        self.updating.set(true);
        self.search_bar.set_text("");
        self.updating.set(false);
        self.zoom.set(1.0);
        self.fit.set_active(true);
        self.sidebar.set_reveal_child(false);
        let id = self.state.borrow().id.clone().unwrap();
        self.request(
            "open",
            json!({"operation":"open","id":id,"root":root,"path":path}),
        );
        self.controls();
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
    }
    pub fn has_document(&self) -> bool {
        let state = self.state.borrow();
        state.id.is_some() || state.recovering
    }
    pub fn gateway_failed(&self, error: &str) {
        self.retire_password();
        let sidebar = self.sidebar.reveals_child();
        self.clear_links();
        self.sidebar.set_reveal_child(sidebar);
        self.clear_selection();
        let mut state = self.state.borrow_mut();
        let open = state.id.take().is_some();
        state.recovering |= open;
        state.generation += 1;
        state.busy = false;
        state.pending_page = None;
        state.comment_verified = Some(false);
        drop(state);
        self.controls();
        if open {
            self.diagnostic(error);
        }
    }
    pub fn gateway_restored(&self) {
        if self.password_prompt.borrow().as_ref().cloned().is_some() {
            return;
        }
        let body = {
            let mut state = self.state.borrow_mut();
            if !state.recovering || state.busy {
                return;
            }
            let id = glib::uuid_string_random().to_string();
            state.id = Some(id.clone());
            state.generation += 1;
            state.initial_page = 0;
            json!({"id":id,"root":state.root,"path":state.path,"hash":state.hash,"page":state.page})
        };
        self.notice("Revalidating PDF…");
        self.request("resume", body);
    }
    #[cfg(debug_assertions)]
    pub fn smoke_ready(&self) -> bool {
        !self.state.borrow().busy && self.picture.paintable().is_some()
    }
    pub fn close(&self) {
        self.retire_password();
        self.clear_links();
        self.clear_selection();
        let id = {
            let mut state = self.state.borrow_mut();
            state.generation += 1;
            state.recovering = false;
            state.id.take()
        };
        if let Some(id) = id {
            self.request("close", json!({"operation":"close","id":id}));
        }
        self.picture.clear_texture();
        self.text.buffer().set_text("");
        self.clear_lists();
        self.controls();
    }
    fn clear_lists(&self) {
        self.link_button.set_active(false);
        self.links.remove_all();
        while let Some(child) = self.outline.first_child() {
            self.outline.remove(&child);
        }
        while let Some(child) = self.hits.first_child() {
            self.hits.remove(&child);
        }
        self.more.set_visible(false);
    }
    fn load(&self, page: usize) {
        let body = {
            let mut state = self.state.borrow_mut();
            let Some(id) = &state.id else { return };
            if page >= state.pages {
                return;
            }
            if state.busy {
                state.pending_page = Some(page);
                return;
            }
            json!({"operation":"page","id":id,"page":page})
        };
        self.clear_selection();
        self.clear_links();
        {
            let mut state = self.state.borrow_mut();
            if state
                .comment
                .as_ref()
                .is_some_and(|(a, _)| a.page - 1 != page)
            {
                state.comment.take();
            }
        }
        self.notice("Loading page…");
        self.request("page", body);
    }
    fn search(&self, more: bool) {
        if self.updating.get() {
            return;
        }
        let query = self.search_bar.text().to_string();
        let body = {
            let mut state = self.state.borrow_mut();
            if !more {
                state.next = Some(0);
                state.query = query.clone();
                state.search_partial = false;
            }
            let Some(id) = state.id.clone() else { return };
            // An opening snapshot is not published until its first page is ready.
            // Retain the query and start it after that response, including password retries.
            if state.pages == 0 {
                return;
            }
            if query.is_empty() {
                state.serial += 1;
                state.search = state.serial;
                drop(state);
                while let Some(child) = self.hits.first_child() {
                    self.hits.remove(&child);
                }
                self.more.set_visible(false);
                self.notice("Read only");
                self.highlight();
                return;
            }
            let Some(from) = state.next else { return };
            json!({"operation":"search","id":id,"query":query,"from":from,"case_sensitive":self.case.is_active()})
        };
        {
            while let Some(child) = self.hits.first_child() {
                self.hits.remove(&child);
            }
        }
        self.more.set_sensitive(false);
        self.panels.set_visible_child_name("search");
        self.sidebar.set_reveal_child(true);
        self.request("search", body);
    }
    pub fn event(self: &Rc<Self>, response: Response) {
        if response.kind == "close" {
            return;
        }
        let current = {
            let state = self.state.borrow();
            response.generation == state.generation
                && response.serial
                    == if response.kind == "select" {
                        state.selection_serial
                    } else if response.kind == "search" {
                        state.search
                    } else {
                        state.view
                    }
        };
        if !current {
            if matches!(response.kind, "open" | "resume") {
                if let Ok(Reply::Page { meta, .. }) = response.result {
                    self.request("close", json!({"operation":"close","id":meta.document_id}));
                }
            }
            return;
        }
        if response.kind == "select" {
            self.state.borrow_mut().selecting = false;
        } else if response.kind != "search" {
            self.state.borrow_mut().busy = false;
        }
        match response.result {
            Err(error) => {
                if response.kind == "resume" {
                    self.state.borrow_mut().id = None;
                }
                if response.kind == "select" && self.state.borrow_mut().comment.take().is_some() {
                    self.state.borrow_mut().comment_verified = Some(false);
                }
                self.diagnostic(&error);
                if error.starts_with("PDF password required") {
                    self.password();
                }
                self.more.set_sensitive(true);
            }
            Ok(Reply::Closed) => {}
            Ok(Reply::Selection(selection)) => {
                let mut state = self.state.borrow_mut();
                if state.id.as_deref() != Some(selection.document_id.as_str())
                    || state.page != selection.page
                    || state.hash != selection.hash
                {
                    return;
                }
                let expected = state.comment.take();
                let matched = expected
                    .as_ref()
                    .is_none_or(|(_, q)| q.trim() == selection.excerpt.trim());
                if expected.is_some() {
                    state.comment_verified = Some(matched);
                }
                if matched {
                    state.selection = Some(selection);
                }
                drop(state);
                self.highlights.queue_draw();
                if matched {
                    self.notice("Text selected · Copy or comment");
                    if expected.is_some() {
                        self.preview.set_active(true);
                    }
                } else {
                    self.notice("Comment range did not match its quote; showing the page without a highlight");
                }
            }
            Ok(Reply::Search(found)) => {
                let pages = self.state.borrow().pages;
                if found.hits.iter().any(|h| h.page >= pages) {
                    self.notice("PDF search returned an invalid page");
                    return;
                }
                let (next, partial) = (found.next, found.partial);
                {
                    let mut state = self.state.borrow_mut();
                    state.next = next;
                    state.search_partial |= partial;
                }
                for hit in found.hits {
                    let button = adw::ActionRow::builder()
                        .title(format!("Page {}", hit.page + 1))
                        .subtitle(&hit.snippet)
                        .title_lines(1)
                        .subtitle_lines(5)
                        .use_markup(false)
                        .activatable(true)
                        .build();
                    crate::i18n_ui::bind_format(
                        &button,
                        "title",
                        "Page {value}",
                        &[("value", &(hit.page + 1).to_string())],
                    );
                    let weak = Rc::downgrade(self);
                    let generation = response.generation;
                    button.connect_activated(move |_| {
                        if let Some(pdf) = weak.upgrade() {
                            if pdf.state.borrow().generation == generation {
                                pdf.text_mode.set_active(true);
                                pdf.load(hit.page);
                            }
                        }
                    });
                    self.hits.append(&button);
                }
                self.more.set_visible(next.is_some());
                self.more.set_sensitive(true);
                self.notice(if self.state.borrow().search_partial {
                    "Partial search results"
                } else if next.is_some() {
                    "Search continues · Next results"
                } else {
                    "Search complete"
                });
            }
            Ok(Reply::Page { meta, image }) => {
                let sidebar = if response.kind == "resume" {
                    Some((
                        self.sidebar.reveals_child(),
                        self.panels.visible_child_name(),
                        self.link_button.is_active(),
                    ))
                } else {
                    None
                };
                if response.kind == "page"
                    && self.state.borrow().id.as_deref() != Some(&meta.document_id)
                {
                    return;
                }
                let opening = matches!(response.kind, "open" | "resume");
                if response.kind == "resume" {
                    self.state.borrow_mut().recovering = false;
                    self.clear_lists();
                }
                {
                    let mut state = self.state.borrow_mut();
                    state.id = Some(meta.document_id.clone());
                    state.page = meta.page;
                    state.pages = meta.pages;
                    state.links = meta.links.clone();
                    if opening {
                        state.hash = meta.hash.clone();
                        state.path = meta.path.clone();
                    }
                }
                self.size.set((image.width as i32, image.height as i32));
                self.picture.set_texture(&crate::images_ui::texture(image));
                self.picture.set_logical_width(self.size.get().0 as u32);
                let text = self.text.buffer();
                if response.kind != "resume"
                    || text.text(&text.start_iter(), &text.end_iter(), true) != meta.text
                {
                    text.set_text(&meta.text);
                }
                let objects = meta
                    .links
                    .iter()
                    .cloned()
                    .map(glib::BoxedAnyObject::new)
                    .collect::<Vec<_>>();
                self.links.splice(0, self.links.n_items(), &objects);
                let mut notices = vec!["Read only"];
                if meta.text_limited {
                    notices.push("page text exceeds the extraction limit");
                }
                if meta.links_limited {
                    notices.push("page link list is limited");
                }
                if meta.links_unavailable {
                    notices.push("some page links are unavailable");
                }
                for property in ["label", "tooltip-text"] {
                    crate::i18n_ui::bind_summary(
                        &self.status,
                        property,
                        notices[0],
                        &[],
                        &notices[1..],
                    );
                }
                let mut link_details = Vec::new();
                if meta.links_limited {
                    link_details.push("list limited");
                }
                if meta.links_unavailable {
                    link_details.push("some links unavailable");
                }
                crate::i18n_ui::bind_summary(
                    &self.link_button,
                    "tooltip-text",
                    "Links on this page",
                    &[],
                    &link_details,
                );
                self.layout();
                if opening {
                    for heading in meta.outline {
                        let button = gtk::Button::with_label(&heading.title);
                        button.add_css_class("flat");
                        button.set_halign(gtk::Align::Fill);
                        button.set_margin_start((heading.depth * 12) as i32);
                        let weak = Rc::downgrade(self);
                        let generation = response.generation;
                        button.connect_clicked(move |_| {
                            if let Some(pdf) = weak.upgrade() {
                                if pdf.state.borrow().generation == generation {
                                    pdf.load(heading.page);
                                }
                            }
                        });
                        self.outline.append(&button);
                    }
                    if meta.outline_limited {
                        self.notice("Contents limited to 500 entries");
                    }
                    let initial = self.state.borrow().initial_page;
                    if initial > 0 && initial < meta.pages {
                        self.state.borrow_mut().initial_page = 0;
                        self.load(initial);
                    } else {
                        self.restore_comment();
                    }
                    let search_pending = !self.state.borrow().query.is_empty();
                    if search_pending {
                        self.search(false);
                    }
                    if let Some((visible, panel, links)) = sidebar {
                        self.link_button.set_active(links);
                        if let Some(panel) = panel {
                            self.panels.set_visible_child_name(&panel);
                        }
                        self.sidebar.set_reveal_child(visible);
                    }
                }
                if !opening {
                    self.restore_comment();
                }
                self.controls();
            }
        }
        self.highlight();
        self.controls();
        if !matches!(response.kind, "search" | "select") {
            let pending = self.state.borrow_mut().pending_page.take();
            if let Some(page) = pending {
                let current = self.state.borrow().page;
                if page != current {
                    self.load(page);
                }
            }
        }
    }
    fn highlight(&self) {
        let buffer = self.text.buffer();
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        let tag = buffer.tag_table().lookup("pdf-search").unwrap_or_else(|| {
            let tag = gtk::TextTag::builder()
                .name("pdf-search")
                .background("#ffd966")
                .foreground("#202020")
                .build();
            buffer.tag_table().add(&tag);
            tag
        });
        buffer.remove_tag(&tag, &start, &end);
        let query = self.state.borrow().query.clone();
        if query.is_empty() {
            return;
        }
        let flags = gtk::TextSearchFlags::TEXT_ONLY
            | if self.case.is_active() {
                gtk::TextSearchFlags::empty()
            } else {
                gtk::TextSearchFlags::CASE_INSENSITIVE
            };
        let mut iter = start;
        for index in 0..1000 {
            let Some((mut a, b)) = iter.forward_search(&query, flags, None) else {
                break;
            };
            buffer.apply_tag(&tag, &a, &b);
            if index == 0 {
                self.text.scroll_to_iter(&mut a, 0.1, false, 0.0, 0.0);
            }
            iter = b;
        }
    }
    fn retire_password(&self) {
        let prompt = self.password_prompt.borrow_mut().take();
        if let Some(dialog) = prompt {
            if let Some(entry) = dialog.extra_child().and_downcast::<gtk::PasswordEntry>() {
                entry.set_text("");
            }
            dialog.force_close();
        }
    }
    fn password(self: &Rc<Self>) {
        if self.password_prompt.borrow().as_ref().cloned().is_some() {
            return;
        }
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let entry = gtk::PasswordEntry::builder()
            .show_peek_icon(true)
            .activates_default(true)
            .build();
        crate::i18n_ui::accessible_label(&entry, "PDF password");
        let dialog = adw::AlertDialog::builder()
            .heading("Unlock PDF")
            .body("Enter the document password.")
            .extra_child(&entry)
            .build()
            .ui_text("heading", "Unlock PDF")
            .ui_text("body", "Enter the document password.");
        dialog.add_responses(&[("cancel", "Cancel"), ("open", "Open")]);
        crate::i18n_ui::response(&dialog, "cancel", "Cancel");
        crate::i18n_ui::response(&dialog, "open", "Open");
        dialog.set_response_appearance("open", adw::ResponseAppearance::Suggested);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("open"));
        let weak = Rc::downgrade(self);
        let (generation, view) = {
            let state = self.state.borrow();
            (state.generation, state.view)
        };
        self.password_prompt.replace(Some(dialog.clone()));
        dialog.connect_response(None, move |dialog, answer| {
            if let Some(pdf) = weak.upgrade() {
                let current =
                    pdf.password_prompt.borrow().as_ref().cloned().as_ref() == Some(dialog);
                if !current {
                    return;
                }
                pdf.password_prompt.borrow_mut().take();
                if answer != "open" {
                    entry.set_text("");
                    return;
                }
                let password = entry.text().to_string();
                entry.set_text("");
                let mut state = pdf.state.borrow_mut();
                if state.generation != generation || state.view != view || state.busy {
                    return;
                }
                let kind = if state.recovering { "resume" } else { "open" };
                if state.id.is_none() {
                    state.id = Some(glib::uuid_string_random().to_string());
                }
                let mut body = json!({"operation":"open", "id":state.id,
                    "root":state.root, "path":state.path, "password":password});
                if state.recovering {
                    body["hash"] = state.hash.clone().into();
                    body["page"] = state.page.into();
                }
                drop(state);
                pdf.request(kind, body);
            }
        });
        dialog.present(Some(&parent));
    }
    fn layout(&self) {
        let fit = self.fit.is_active();
        let (w, h) = self.size.get();
        self.picture.set_halign(if fit {
            gtk::Align::Fill
        } else {
            gtk::Align::Center
        });
        self.picture.set_valign(if fit {
            gtk::Align::Fill
        } else {
            gtk::Align::Center
        });
        self.picture.set_size_request(
            if fit {
                -1
            } else {
                (w as f64 * self.zoom.get()) as i32
            },
            if fit {
                -1
            } else {
                (h as f64 * self.zoom.get()) as i32
            },
        );
        self.canvas.set_policy(
            if fit {
                gtk::PolicyType::Never
            } else {
                gtk::PolicyType::Automatic
            },
            gtk::PolicyType::Automatic,
        );
        self.highlights.queue_draw();
    }
    fn controls(&self) {
        let state = self.state.borrow();
        self.link_button
            .set_sensitive(!state.busy && !state.links.is_empty());
        self.updating.set(true);
        self.page.set_text(&(state.page + 1).to_string());
        self.updating.set(false);
        self.total.set_text(&format!("/ {}", state.pages));
        self.previous
            .set_sensitive(state.id.is_some() && state.page > 0);
        self.next
            .set_sensitive(state.id.is_some() && state.page + 1 < state.pages);
        self.copy.set_sensitive(
            state.id.is_some()
                && !state.busy
                && !state.selecting
                && self.text.buffer().char_count() > 0,
        );
        let selected = if self.preview.is_active() {
            state.selection.is_some()
        } else {
            self.text.buffer().has_selection()
        };
        self.comment
            .set_sensitive(state.id.is_some() && !state.busy && !state.selecting && selected);
    }
    fn choose(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open PDF")
            .accept_label("Open")
            .build()
            .ui_text("title", "Open PDF")
            .ui_text("accept-label", "Open");
        let filter = gtk::FileFilter::new();
        crate::i18n_ui::bind(&filter, "name", "PDF documents");
        filter.add_suffix("pdf");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let weak = Rc::downgrade(self);
        dialog.open(
            self.parent.upgrade().as_ref(),
            gio::Cancellable::NONE,
            move |result| {
                if let (Some(pdf), Ok(file)) = (weak.upgrade(), result) {
                    if let Some(path) = file.path() {
                        if let Some(root) = path.parent() {
                            pdf.open(&root.to_string_lossy(), &path.to_string_lossy(), 0);
                        }
                    }
                }
            },
        );
    }
}
#[cfg(debug_assertions)]
impl Pdf {
    async fn smoke_pointer(
        &self,
        root: &std::path::Path,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    ) -> Result<(), String> {
        let parent = self
            .parent
            .upgrade()
            .ok_or("PDF fixture window disappeared")?;
        let start = self
            .picture
            .compute_point(&parent, &gtk::graphene::Point::new(x as f32, y as f32))
            .ok_or("PDF pointer origin is not mapped")?;
        let value = serde_json::to_vec(&json!({"x":start.x(),"y":start.y(),"dx":dx,"dy":dy}))
            .map_err(|e| e.to_string())?;
        let root = root.to_owned();
        gio::spawn_blocking(move || {
            let temporary = root.join("pdf-links-pointer.tmp");
            std::fs::write(&temporary, value)
                .and_then(|_| std::fs::rename(temporary, root.join("pdf-links-pointer.json")))
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Pointer fixture worker stopped")?
    }
    pub async fn smoke_links(
        self: &Rc<Self>,
        root: &std::path::Path,
        capture: impl Fn(),
    ) -> Result<Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(25) {
                    return Err(format!("PDF link test timed out: {step}"));
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            Ok(())
        }
        async fn portal_count(root: &std::path::Path, count: usize) -> Result<(), String> {
            let start = std::time::Instant::now();
            loop {
                let path = root.join("portal-uris.json");
                let value = gio::spawn_blocking(move || {
                    std::fs::read(path)
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
                        .map_or(0, |items| items.len())
                })
                .await
                .map_err(|_| "URI fixture file worker stopped")?;
                if value >= count {
                    return Ok(());
                }
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err("GTK URI launcher did not reach the private portal".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
        }
        until("initial page", || {
            !self.state.borrow().busy && self.state.borrow().pages == 3
        })
        .await?;
        if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some() {
            until("focus", || {
                self.parent
                    .upgrade()
                    .is_some_and(|window| window.is_active())
            })
            .await?;
            let parent = self
                .parent
                .upgrade()
                .ok_or("PDF fixture window disappeared")?;
            parent.fullscreen();
            until("owned fullscreen monitor", || {
                parent.is_fullscreen() && parent.width() == 1600 && parent.height() == 1000
            })
            .await?;
            if parent.scale_factor() != 1 {
                return Err("PDF pointer fixture requires its scale-one monitor".into());
            }
        }
        let controllers = self.picture.observe_controllers();
        let click = (0..controllers.n_items())
            .find_map(|i| controllers.item(i).and_downcast::<gtk::GestureClick>())
            .ok_or("PDF preview has no click controller")?;
        let drag = (0..controllers.n_items())
            .find_map(|i| controllers.item(i).and_downcast::<gtk::GestureDrag>())
            .ok_or("PDF preview has no drag controller")?;
        if !click.is_grouped_with(&drag) {
            return Err("PDF click/selection gestures are not grouped".into());
        }
        if self.state.borrow().links.len() != 6 || self.links.n_items() != 6 {
            return Err("Native PDF did not receive bounded safe links".into());
        }
        self.link_button.set_active(true);
        glib::timeout_future(std::time::Duration::from_millis(200)).await;
        capture();
        self.text_mode.set_active(true);
        if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some() {
            self.link_view
                .model()
                .and_downcast::<gtk::SingleSelection>()
                .ok_or("PDF links have no native selection")?
                .set_selected(0);
            if !self.link_view.grab_focus() {
                return Err("PDF link list could not receive keyboard focus".into());
            }
            let key = root.join("pdf-links-key");
            gio::spawn_blocking(move || std::fs::write(key, b"Return"))
                .await
                .map_err(|_| "Keyboard fixture worker stopped")?
                .map_err(|e| e.to_string())?;
        } else {
            self.link_view.emit_by_name::<()>("activate", &[&0u32]);
        }
        until("sidebar destination", || {
            !self.state.borrow().busy && self.state.borrow().page == 2
        })
        .await?;
        if !self.text_mode.is_active() {
            return Err("PDF link changed the user's view mode".into());
        }
        self.load(0);
        until("return to link page", || {
            !self.state.borrow().busy && self.state.borrow().page == 0
        })
        .await?;
        self.preview.set_active(true);
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        let point = |index: usize| -> Result<[f64; 2], String> {
            let rect = self
                .state
                .borrow()
                .links
                .get(index)
                .and_then(|l| l.rects.first())
                .cloned()
                .ok_or("PDF link region is absent")?;
            let (x, y, w, h) = self.view_rect().ok_or("PDF preview has no allocation")?;
            Ok([
                x + (rect.x + rect.width / 2.0) * w,
                y + (rect.y + rect.height / 2.0) * h,
            ])
        };
        let [x, y] = point(0)?;
        click.emit_by_name::<()>("pressed", &[&1i32, &x, &y]);
        click.emit_by_name::<()>("released", &[&1i32, &(x + 6.0), &y]);
        if self.state.borrow().busy || self.state.borrow().page != 0 {
            return Err("Dragging a PDF link activated it".into());
        }
        click.emit_by_name::<()>("pressed", &[&1i32, &x, &y]);
        self.text_mode.set_active(true);
        click.emit_by_name::<()>("released", &[&1i32, &x, &y]);
        if self.state.borrow().busy {
            return Err("A PDF click survived a view-mode change".into());
        }
        self.preview.set_active(true);
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        click.emit_by_name::<()>("pressed", &[&1i32, &x, &y]);
        self.load(1);
        until("stale click page", || {
            !self.state.borrow().busy && self.state.borrow().page == 1
        })
        .await?;
        click.emit_by_name::<()>("released", &[&1i32, &x, &y]);
        if self.state.borrow().busy || self.state.borrow().page != 1 {
            return Err("A stale PDF click navigated another page".into());
        }
        self.load(0);
        until("click page", || {
            !self.state.borrow().busy && self.state.borrow().page == 0
        })
        .await?;
        glib::timeout_future(std::time::Duration::from_millis(80)).await;
        let [x, y] = point(0)?;
        let real_pointer = std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some();
        if real_pointer {
            self.smoke_pointer(root, x, y, 0.0, 0.0).await?;
        } else {
            click.emit_by_name::<()>("pressed", &[&1i32, &x, &y]);
            click.emit_by_name::<()>("released", &[&1i32, &x, &y]);
        }
        until("preview destination", || {
            !self.state.borrow().busy && self.state.borrow().page == 2
        })
        .await?;
        self.load(0);
        until("URI link page", || {
            !self.state.borrow().busy && self.state.borrow().page == 0
        })
        .await?;
        let (px, py, pw, ph) = self.view_rect().ok_or("PDF drag has no image bounds")?;
        let (sx, sy) = (px + pw * 65.0 / 595.0, py + ph * (842.0 - 662.0) / 842.0);
        let dx = pw * 70.0 / 595.0;
        if real_pointer {
            self.smoke_pointer(root, sx, sy, dx, 0.0).await?;
            glib::timeout_future(std::time::Duration::from_millis(400)).await;
        } else {
            click.emit_by_name::<()>("pressed", &[&1i32, &sx, &sy]);
            drag.emit_by_name::<()>("drag-begin", &[&sx, &sy]);
            drag.emit_by_name::<()>("drag-update", &[&dx, &0.0f64]);
            click.emit_by_name::<()>("stopped", &[]);
            drag.emit_by_name::<()>("drag-end", &[&dx, &0.0f64]);
        }
        until("drag text selection", || !self.state.borrow().selecting).await?;
        if self.state.borrow().busy
            || self.state.borrow().page != 0
            || !self
                .state
                .borrow()
                .selection
                .as_ref()
                .is_some_and(|selection| selection.excerpt.trim_start().starts_with("Go to"))
        {
            return Err(format!("Text selection over a PDF link was lost or navigated: status={} page={} busy={} selecting={} start=({sx},{sy}) delta={dx}",self.status.text(),self.state.borrow().page,self.state.borrow().busy,self.state.borrow().selecting));
        }
        let rects = self.state.borrow().links[5].rects.clone();
        let (x, y, w, h) = self.view_rect().unwrap();
        let px = x + (rects[0].x + 0.01) * w;
        let py = y + (rects[0].y + rects[0].height + rects[1].y) / 2.0 * h;
        if self.link_at(px, py).is_some() {
            return Err("The gap between PDF link quads is clickable".into());
        }
        self.link_view.emit_by_name::<()>("activate", &[&3u32]);
        portal_count(root, 1).await?;
        until("URI response", || self.uri_request.borrow().is_none()).await?;
        self.link_view.emit_by_name::<()>("activate", &[&4u32]);
        portal_count(root, 2).await?;
        until("localhost URI response", || {
            self.uri_request.borrow().is_none()
        })
        .await?;
        let path = root.join("hold-uri");
        gio::spawn_blocking(move || std::fs::write(path, b"hold"))
            .await
            .map_err(|_| "URI fixture file worker stopped")?
            .map_err(|e| e.to_string())?;
        self.link_view.emit_by_name::<()>("activate", &[&3u32]);
        portal_count(root, 3).await?;
        self.load(1);
        until("URI cancellation", || {
            !self.state.borrow().busy && self.state.borrow().page == 1
        })
        .await?;
        if self.uri_request.borrow().is_some()
            || self.status.text().starts_with("Could not open PDF link")
        {
            return Err("Cancelled PDF URI request affected another page".into());
        }
        self.load(0);
        until("replacement source", || {
            !self.state.borrow().busy && self.state.borrow().page == 0
        })
        .await?;
        let [x, y] = point(0)?;
        click.emit_by_name::<()>("pressed", &[&1i32, &x, &y]);
        self.open(&root.join("project").to_string_lossy(), "plain.pdf", 0);
        until("replacement PDF", || {
            !self.state.borrow().busy
                && self.state.borrow().pages == 3
                && self.state.borrow().links.is_empty()
        })
        .await?;
        click.emit_by_name::<()>("released", &[&1i32, &x, &y]);
        if self.state.borrow().busy || self.state.borrow().page != 0 || self.links.n_items() != 0 {
            return Err("Stale PDF link survived file replacement".into());
        }
        Ok(
            json!({"ok":true,"previewLinks":true,"ownedPointerEvents":real_pointer,"keyboardAccessibleRows":true,"viewModeRetained":true,"dragDoesNotActivate":true,"textSelectionRetained":true,"quadGap":true,"staleClickRejected":true,"modeSwitchGuard":true,"replacementIsolation":true,"asyncUriPortal":true,"pendingUriCancelled":true}),
        )
    }
}
