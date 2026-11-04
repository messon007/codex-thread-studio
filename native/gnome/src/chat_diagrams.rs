//! Visible conversation diagrams share the document render lane and raster cache.
use crate::{
    content_widgets::Styles,
    document::{Message, Reply, Request, Response},
    scaled_picture::ScaledPicture,
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
#[cfg(debug_assertions)]
#[path = "chat_diagram_budget_acceptance.rs"]
mod budget_acceptance;
#[cfg(debug_assertions)]
#[path = "diagram_viewport_acceptance.rs"]
mod viewport_acceptance;
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub const KEY: u64 = 4;
pub struct Renderer {
    key: u64,
    owner: Weak<Renderer>,
    visible_only: bool,
    visibility: RefCell<Option<Rc<crate::viewport::FrameUpdate>>>,
    #[cfg(debug_assertions)]
    visibility_passes: Cell<u64>,
    viewport: RefCell<Option<crate::viewport::Tracker>>,
    sender: mpsc::Sender<Message>,
    generation: Cell<u64>,
    sequence: Cell<u64>,
    next_id: Cell<usize>,
    widgets: RefCell<HashMap<usize, Weak<Diagram>>>,
    pending: RefCell<VecDeque<usize>>,
    active: RefCell<HashSet<usize>>,
    style: RefCell<Option<glib::SignalHandlerId>>,
    preferences: RefCell<Option<Rc<dyn Fn()>>>,
    preview_budget: Option<usize>,
}
struct Diagram {
    owner: Weak<Renderer>,
    root: glib::WeakRef<gtk::Box>,
    id: usize,
    generation: u64,
    kind: String,
    source: String,
    serial: Cell<u64>,
    label: gtk::Label,
    picture: ScaledPicture,
    loading: gtk::Spinner,
    retry: gtk::Button,
    bytes: Cell<usize>,
    failed: Cell<bool>,
    viewports: RefCell<crate::viewport::Scopes>,
}
impl Renderer {
    #[cfg(debug_assertions)]
    pub fn smoke_work_counts(&self) -> (usize, usize) {
        (self.active.borrow().len(), self.pending.borrow().len())
    }
    #[cfg(debug_assertions)]
    pub fn streaming_probe(&self) -> (usize, bool) {
        let visible = self
            .widgets
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        (
            self.next_id.get(),
            ["mermaid", "math"].iter().all(|kind| {
                visible
                    .iter()
                    .any(|d| d.kind == *kind && d.picture.smoke_signature().is_some())
            }),
        )
    }
    pub fn new(sender: mpsc::Sender<Message>) -> Rc<Self> {
        Self::new_with_key(sender, KEY)
    }
    pub fn new_with_key(sender: mpsc::Sender<Message>, key: u64) -> Rc<Self> {
        Self::with_budget(sender, key, None, false)
    }
    pub fn new_bounded(sender: mpsc::Sender<Message>, key: u64, bytes: usize) -> Rc<Self> {
        Self::with_budget(sender, key, Some(bytes), false)
    }
    pub fn new_viewport_bounded(sender: mpsc::Sender<Message>, key: u64, bytes: usize) -> Rc<Self> {
        Self::with_budget(sender, key, Some(bytes), true)
    }
    fn with_budget(
        sender: mpsc::Sender<Message>,
        key: u64,
        preview_budget: Option<usize>,
        visible_only: bool,
    ) -> Rc<Self> {
        let renderer = Rc::new_cyclic(|owner| Self {
            owner: owner.clone(),
            visible_only,
            visibility: RefCell::new(None),
            #[cfg(debug_assertions)]
            visibility_passes: Cell::new(0),
            viewport: RefCell::new(None),
            preview_budget,
            key,
            sender,
            generation: Cell::new(next_generation()),
            sequence: Cell::new(0),
            next_id: Cell::new(0),
            widgets: RefCell::new(HashMap::new()),
            pending: RefCell::new(VecDeque::new()),
            active: RefCell::new(HashSet::new()),
            style: RefCell::new(None),
            preferences: RefCell::new(None),
        });
        if visible_only {
            let weak = Rc::downgrade(&renderer);
            renderer
                .visibility
                .replace(Some(crate::viewport::FrameUpdate::new(Rc::new(
                    move |clock| {
                        if let Some(renderer) = weak.upgrade() {
                            renderer.refresh_visible(clock);
                        }
                    },
                ))));
            let weak = Rc::downgrade(&renderer);
            renderer
                .viewport
                .replace(Some(crate::viewport::Tracker::new(Rc::new(move || {
                    if let Some(renderer) = weak.upgrade() {
                        renderer.queue_visible();
                    }
                }))));
        }
        let weak = Rc::downgrade(&renderer);
        let handler = adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(renderer) = weak.upgrade() {
                let widgets = renderer
                    .widgets
                    .borrow()
                    .values()
                    .filter_map(Weak::upgrade)
                    .collect::<Vec<_>>();
                for widget in widgets {
                    renderer.cancel(widget.id);
                    widget.failed.set(false);
                    if renderer.visible_only {
                        widget.clear();
                    } else if widget.root.upgrade().is_some_and(|root| root.is_mapped()) {
                        renderer.enqueue(widget.id);
                    }
                }
                if renderer.visible_only {
                    renderer.queue_visible();
                }
            }
        });
        renderer.style.replace(Some(handler));
        let weak = Rc::downgrade(&renderer);
        let callback: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(renderer) = weak.upgrade() {
                renderer.preferences_changed();
            }
        });
        crate::appearance_ui::watch_diagrams(&callback);
        renderer.preferences.replace(Some(callback));
        renderer
    }
    fn preferences_changed(&self) {
        let widgets = self
            .widgets
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .filter(|w| w.kind == "mermaid")
            .collect::<Vec<_>>();
        for widget in widgets {
            self.cancel(widget.id);
            widget.clear();
            widget.failed.set(false);
            if !self.visible_only && widget.root.upgrade().is_some_and(|root| root.is_mapped()) {
                self.enqueue(widget.id);
            }
        }
        if self.visible_only {
            self.queue_visible();
        }
    }
    fn queue_visible(&self) {
        if let Some(update) = self.visibility.borrow().as_ref() {
            for root in self
                .widgets
                .borrow()
                .values()
                .filter_map(Weak::upgrade)
                .filter_map(|w| w.root.upgrade())
            {
                update.queue(&root);
            }
        }
    }
    fn refresh_visible(&self, clock: &gtk::gdk::FrameClock) {
        #[cfg(debug_assertions)]
        self.visibility_passes.set(self.visibility_passes.get() + 1);
        let widgets = self
            .widgets
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .filter(|w| {
                w.root
                    .upgrade()
                    .and_then(|root| root.frame_clock())
                    .as_ref()
                    == Some(clock)
            })
            .collect::<Vec<_>>();
        // Release offscreen data before admitting replacements against budget.
        for widget in &widgets {
            if !widget.in_view() {
                self.cancel(widget.id);
                widget.clear();
                if !widget.picture.is_visible() {
                    widget.deferred();
                }
            }
        }
        for widget in widgets {
            if widget.generation == self.generation.get()
                && widget.in_view()
                && !widget.failed.get()
                && widget.picture.paintable().is_none()
            {
                self.enqueue(widget.id);
            }
        }
        self.pump();
    }
    pub fn reset(&self) {
        let previous = self.generation.replace(next_generation());
        if let Some(viewport) = self.viewport.borrow().as_ref() {
            viewport.clear();
        }
        if let Some(update) = self.visibility.borrow().as_ref() {
            update.clear();
        }
        self.pending.borrow_mut().clear();
        self.active.borrow_mut().clear();
        let widgets = self
            .widgets
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        self.widgets.borrow_mut().clear();
        for widget in widgets {
            widget.clear();
        }
        let _ = self.send_for_generation(previous, Request::CancelDiagrams);
    }
    fn send(&self, request: Request) -> Option<u64> {
        self.send_for_generation(self.generation.get(), request)
    }
    fn send_for_generation(&self, generation: u64, request: Request) -> Option<u64> {
        let serial = self.sequence.get().wrapping_add(1);
        self.sequence.set(serial);
        self.sender
            .try_send(Message {
                key: self.key,
                generation,
                serial,
                request,
            })
            .ok()
            .map(|_| serial)
    }
    fn cancel(&self, id: usize) {
        if let Some(widget) = self.widgets.borrow().get(&id).and_then(Weak::upgrade) {
            widget.serial.set(0);
        }
        self.pending.borrow_mut().retain(|queued| *queued != id);
        if self.active.borrow_mut().remove(&id) {
            let _ = self.send(Request::CancelDiagram { id });
        }
    }
    fn enqueue(&self, id: usize) {
        if self.active.borrow().contains(&id) || self.pending.borrow().contains(&id) {
            return;
        }
        if self.pending.borrow().len() >= 128 {
            if let Some(widget) = self.widgets.borrow().get(&id).and_then(Weak::upgrade) {
                widget.failed("Too many visible diagrams; retry after other diagrams finish");
            }
            return;
        }
        self.pending.borrow_mut().push_back(id);
        self.pump();
    }
    fn pump(&self) {
        // Chat admits two jobs so reader navigation and document diagrams keep
        // access to the shared lane. Queueing itself never blocks GTK.
        while self.active.borrow().len() < 2 {
            let id = self.pending.borrow_mut().pop_front();
            let Some(id) = id else {
                break;
            };
            let widget = self.widgets.borrow().get(&id).and_then(Weak::upgrade);
            let Some(widget) = widget else {
                continue;
            };
            if widget.generation != self.generation.get() || !widget.in_view() {
                continue;
            }
            crate::i18n_ui::bind_format(
                &widget.label,
                "label",
                "Rendering {kind}…",
                &[("kind", &widget.kind)],
            );
            widget
                .label
                .set_visible(!self.visible_only || !widget.picture.is_visible());
            widget.retry.set_visible(false);
            if let Some(serial) = self.send(Request::Diagram {
                id,
                kind: widget.kind.clone(),
                source: widget.source.clone(),
                dark: adw::StyleManager::default().is_dark(),
                style: crate::appearance_ui::diagram_style(),
            }) {
                widget.serial.set(serial);
                self.active.borrow_mut().insert(id);
            } else {
                widget.failed("Diagram request queue is unavailable; retry shortly");
            }
        }
    }
    pub fn event(&self, response: Response) {
        if response.key != self.key
            || response.generation != self.generation.get()
            || response.kind != "diagram"
        {
            return;
        }
        let widget = self
            .widgets
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .find(|widget| widget.serial.get() == response.serial);
        if let Some(widget) = widget {
            self.active.borrow_mut().remove(&widget.id);
            if self.visible_only && !widget.in_view() {
                widget.serial.set(0);
                widget.clear();
                self.queue_visible();
                return;
            }
            match response.result {
                Ok(Reply::Diagram { image, .. }) => {
                    let bytes = image
                        .pixels
                        .len()
                        .saturating_add(image.vector.as_ref().map_or(0, |v| v.heap_bytes()));
                    let used = self
                        .widgets
                        .borrow()
                        .values()
                        .filter_map(Weak::upgrade)
                        .filter(|w| w.id != widget.id)
                        .map(|w| w.bytes.get())
                        .sum::<usize>();
                    if self
                        .preview_budget
                        .is_some_and(|limit| used.saturating_add(bytes) > limit)
                    {
                        widget.clear();
                        let key =
                            "Diagram previews are too large; use Source or open a smaller section";
                        widget.failed(key);
                        crate::i18n_ui::bind(&widget.label, "label", key);
                    } else {
                        widget.loading.stop();
                        widget.loading.set_visible(false);
                        widget.picture.set_diagram(image);
                        widget.bytes.set(bytes);
                        widget.picture.set_visible(true);
                        widget.label.set_visible(false);
                        widget.retry.set_visible(false);
                    }
                }
                Err(error) => widget.failed(&error),
                _ => {}
            }
        }
        self.widgets
            .borrow_mut()
            .retain(|_, widget| widget.strong_count() > 0);
        if self.visible_only {
            self.queue_visible();
        } else {
            self.pump();
        }
    }
    pub fn widget(self: &Rc<Self>, kind: &str, source: &str, styles: Rc<Styles>) -> gtk::Box {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("card");
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.set_margin_start(12);
        header.set_margin_end(6);
        header.set_margin_top(6);
        let title = gtk::Label::builder()
            .label(kind)
            .xalign(0.0)
            .hexpand(true)
            .build();
        title.add_css_class("caption");
        header.append(&title);
        root.append(&header);
        let label = gtk::Label::builder()
            .label(format!("Rendering {kind}…"))
            .wrap(true)
            .xalign(0.0)
            .build();
        crate::i18n_ui::bind_format(&label, "label", "Rendering {kind}…", &[("kind", kind)]);
        label.set_margin_start(12);
        label.set_margin_end(12);
        root.append(&label);
        let alternative = if kind == "math" {
            format!("Math formula: {source}")
        } else {
            format!("{kind} conversation diagram")
        };
        let picture = ScaledPicture::new(&alternative);
        if self.preview_budget.is_some() {
            picture.set_max_height(720);
        }
        picture.set_hexpand(true);
        picture.set_visible(false);
        header.append(&picture.inspect_button(kind));
        let source_button = gtk::ToggleButton::with_label("Source");
        crate::i18n_ui::bind(&source_button, "label", "markdown-mode\u{4}Source");
        crate::i18n_ui::bind(&source_button, "tooltip-text", "Show diagram source");
        crate::i18n_ui::accessible_label(&source_button, "Show diagram source");
        header.append(&source_button);
        let preview = gtk::Overlay::builder().child(&picture).build();
        let loading = gtk::Spinner::builder()
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .can_target(false)
            .visible(false)
            .build();
        crate::i18n_ui::accessible_label(&loading, "Rendering diagram");
        preview.add_overlay(&loading);
        root.append(&preview);
        let retry = gtk::Button::with_label("Retry diagram");
        crate::i18n_ui::bind(&retry, "label", "Retry diagram");
        retry.set_halign(gtk::Align::Start);
        retry.set_visible(false);
        root.append(&retry);
        let code = gtk::Box::new(gtk::Orientation::Vertical, 0);
        code.set_visible(false);
        root.append(&code);
        let kind_text = kind.to_owned();
        let source_text = source.to_owned();
        let weak_code = code.downgrade();
        source_button.connect_toggled(move |button| {
            if let Some(code) = weak_code.upgrade() {
                if button.is_active() && code.first_child().is_none() {
                    code.append(&styles.code(&kind_text, &source_text));
                }
                code.set_visible(button.is_active());
            }
        });
        let id = self.next_id.get().wrapping_add(1);
        self.next_id.set(id);
        let diagram = Rc::new(Diagram {
            owner: Rc::downgrade(self),
            root: root.downgrade(),
            id,
            generation: self.generation.get(),
            kind: kind.into(),
            source: source.into(),
            serial: Cell::new(0),
            label,
            picture,
            loading,
            retry,
            bytes: Cell::new(0),
            failed: Cell::new(false),
            viewports: RefCell::default(),
        });
        if self.visible_only {
            diagram.deferred();
        }
        let weak_owner = self.owner.clone();
        diagram.picture.on_allocate(Rc::new(move || {
            if let Some(owner) = weak_owner.upgrade().filter(|o| o.visible_only) {
                owner.queue_visible();
            }
        }));
        self.widgets
            .borrow_mut()
            .retain(|_, widget| widget.strong_count() > 0);
        self.widgets
            .borrow_mut()
            .insert(id, Rc::downgrade(&diagram));
        let widget = diagram.clone();
        root.connect_map(move |_| {
            if let Some(owner) = widget.owner.upgrade() {
                if widget.generation != owner.generation.get() {
                    return;
                }
                if owner.visible_only {
                    *widget.viewports.borrow_mut() = owner
                        .viewport
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .watch(widget.root.upgrade().as_ref().unwrap());
                    owner.queue_visible();
                } else {
                    owner.enqueue(widget.id);
                }
            }
        });
        let widget = diagram.clone();
        root.connect_unmap(move |_| {
            if let Some(owner) = widget.owner.upgrade() {
                owner.cancel(widget.id);
                owner.pump();
            }
            widget.clear();
            widget.failed.set(false);
            widget.viewports.borrow_mut().clear();
        });
        let weak = Rc::downgrade(&diagram);
        diagram.retry.connect_clicked(move |_| {
            if let Some(widget) = weak.upgrade() {
                if let Some(owner) = widget.owner.upgrade() {
                    widget.failed.set(false);
                    if owner.visible_only {
                        owner.queue_visible();
                    } else {
                        owner.enqueue(widget.id);
                    }
                }
            }
        });
        root
    }
}
#[cfg(debug_assertions)]
impl Renderer {
    pub fn smoke_style_state(&self, root: &gtk::Box) -> Option<serde_json::Value> {
        let widget = self
            .widgets
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .find(|d| d.root.upgrade().as_ref() == Some(root))?;
        let mut value = widget.picture.smoke_signature()?;
        value["serial"] = serde_json::json!(widget.serial.get());
        value["mathLabels"] =
            serde_json::json!(widget.picture.vector().map_or(0, |v| v.smoke_math_labels()));
        Some(value)
    }
    pub fn smoke_stale_style_reply(&self, serial: u64) {
        self.event(Response {
            key: self.key,
            generation: self.generation.get(),
            serial,
            kind: "diagram",
            result: Err("stale diagram profile failure".into()),
        });
    }
    fn smoke_recreated_owner(&self) -> Result<(), String> {
        let old = Self::new_with_key(self.sender.clone(), 999);
        old.reset();
        let stale = old.generation.get();
        let current = Self::new_with_key(self.sender.clone(), 999);
        current.reset();
        let root = current.widget("mermaid", "flowchart LR\nA --> B", Styles::new());
        let diagram = current
            .widgets
            .borrow()
            .values()
            .find_map(Weak::upgrade)
            .ok_or("Replacement diagram was not registered")?;
        diagram.serial.set(42);
        current.active.borrow_mut().insert(diagram.id);
        let reply = |generation| Response {
            key: 999,
            generation,
            serial: 42,
            kind: "diagram",
            result: Ok(Reply::Diagram {
                id: diagram.id,
                image: crate::diagram::Image {
                    width: 1,
                    height: 1,
                    pixels: std::sync::Arc::from([0u8, 0, 0, 255]),
                    math: None,
                    vector: None,
                },
            }),
        };
        current.event(reply(stale));
        if diagram.picture.is_visible() || !current.active.borrow().contains(&diagram.id) {
            return Err("A late diagram result changed a recreated context window".into());
        }
        old.reset();
        current.event(reply(current.generation.get()));
        if !diagram.picture.is_visible() || current.active.borrow().contains(&diagram.id) {
            return Err("The replacement diagram could not accept its own result".into());
        }
        drop(root);
        Ok(())
    }
    pub async fn smoke_test(
        &self,
        view: &gtk::ListView,
        store: &gtk::gio::ListStore,
        scroll: &gtk::ScrolledWindow,
        capture: impl Fn(&str),
    ) -> Result<serde_json::Value, String> {
        self.smoke_recreated_owner()?;
        let mut checked = Vec::new();
        let mut inspectors = serde_json::Map::new();
        for kind in ["mermaid", "plantuml", "d2", "math"] {
            let position = (0..store.n_items()).find(|index| {
                let object = store.item(*index).and_downcast::<glib::BoxedAnyObject>().unwrap(); let row = object.borrow::<crate::chat_ui::Row>();
                if let crate::chat_ui::Part::Block(block) = row.part { matches!(&row.item.rich.as_ref().unwrap().rows[block].block.kind, studio_markdown_native::BlockKind::Diagram { kind: value, .. } if value == kind) } else { false }
            }).ok_or_else(|| format!("The {kind} conversation block is missing"))?;
            view.scroll_to(position, gtk::ListScrollFlags::NONE, None);
            crate::window_capture::next_frame(view).await?;
            capture(&format!("chat-{kind}.png"));
            let started = std::time::Instant::now();
            let widget = loop {
                let widget = self
                    .widgets
                    .borrow()
                    .values()
                    .filter_map(Weak::upgrade)
                    .find(|widget| widget.kind == kind && widget.root.upgrade().is_some());
                if let Some(widget) = widget {
                    if widget.picture.paintable().is_some()
                        && !widget.label.is_visible()
                        && !self.active.borrow().contains(&widget.id)
                    {
                        break widget;
                    }
                    if widget.retry.is_visible() {
                        return Err(format!("Chat {kind}: {}", widget.label.text()));
                    }
                }
                if started.elapsed() > std::time::Duration::from_secs(12) {
                    return Err(format!("Chat {kind} diagram did not render"));
                }
                glib::timeout_future(std::time::Duration::from_millis(30)).await;
            };
            let root = widget
                .root
                .upgrade()
                .ok_or("Diagram row was recycled before inspection")?;
            if let Some(bounds) = root.compute_bounds(view) {
                let adjustment = scroll.vadjustment();
                adjustment.set_value((adjustment.value() + f64::from(bounds.y()) - 20.0).max(0.0));
            }
            crate::window_capture::next_frame(view).await?;
            capture(&format!("chat-{kind}.png"));
            if widget.picture.width() < 32 || widget.picture.height() < 20 {
                return Err(format!("Chat {kind} image has no native layout size"));
            }
            if kind == "mermaid"
                && widget.picture.vector().map_or(0, |v| v.smoke_math_labels()) != 2
            {
                return Err("Conversation Mermaid formulas did not become native outlines".into());
            }
            let source = root
                .first_child()
                .and_downcast::<gtk::Box>()
                .and_then(|header| header.last_child())
                .and_downcast::<gtk::ToggleButton>()
                .ok_or("Diagram Source control missing")?;
            source.set_active(true);
            let code = root
                .last_child()
                .and_downcast::<gtk::Box>()
                .ok_or("Diagram source container missing")?;
            if code.first_child().is_none() {
                return Err("Diagram source was not created on demand".into());
            }
            source.set_active(false);
            let inspect = root
                .first_child()
                .and_then(|header| {
                    let mut child = header.first_child();
                    while let Some(current) = child {
                        child = current.next_sibling();
                        if current.tooltip_text().as_deref() == Some("Inspect diagram") {
                            return current.downcast::<gtk::Button>().ok();
                        }
                    }
                    None
                })
                .ok_or("Diagram inspection control missing")?;
            let inspection =
                crate::texture_viewer::smoke(&widget.picture, &inspect, kind, &capture).await?;
            inspectors.insert(kind.into(), inspection);
            checked.push(kind);
        }
        Ok(
            serde_json::json!({"ok":true,"formats":checked,"mermaidMathLabels":2,"lazySource":true,"nativeImages":true,"recreatedOwnerIsolation":true,"nativeInspector":true,"zoomFitCopy":true,"inspectorSurvivesSourceUnload":true,"closedInspectorReleasesPixels":true,"vectorInspectors":inspectors}),
        )
    }
}

impl Diagram {
    fn in_view(&self) -> bool {
        self.root.upgrade().is_some_and(|root| {
            root.is_mapped()
                && self.owner.upgrade().is_some_and(|owner| {
                    !owner.visible_only
                        || crate::viewport::intersects(&root, &self.viewports.borrow())
                })
        })
    }
    fn deferred(&self) {
        if !self.failed.get() {
            crate::i18n_ui::bind(&self.label, "label", "Diagram will render when visible");
        }
    }
    fn clear(&self) {
        self.loading.stop();
        self.loading.set_visible(false);
        self.picture.clear_texture();
        self.bytes.set(0);
    }
    fn failed(&self, error: &str) {
        self.failed.set(true);
        self.loading.stop();
        self.loading.set_visible(false);
        crate::i18n_ui::literal(&self.label, "label", error);
        self.label.set_visible(true);
        self.retry.set_visible(true);
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        self.reset();
        if let Some(handler) = self.style.get_mut().take() {
            adw::StyleManager::default().disconnect(handler);
        }
    }
}
