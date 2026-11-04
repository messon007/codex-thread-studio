//! Visible native labels share the bounded Rust diagram lane and raster cache.
use crate::{
    document::{Message, Reply, Request, Response},
    inline_math_shape::{Prepared, Surface},
};
use adw::prelude::*;
use gtk::{gdk, gio, glib, pango};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};
use tokio::sync::mpsc;
pub const KEY: u64 = 9901;
const GROUP_BYTES: usize = 4 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(1);
static BUDGET: OnceLock<Arc<AtomicUsize>> = OnceLock::new();
thread_local! {static ACTIVE:RefCell<Weak<Renderer>>=RefCell::new(Weak::new());}
struct Formula {
    range: std::ops::Range<usize>,
    source: String,
    display: bool,
}
struct Label {
    id: usize,
    owner: Weak<Renderer>,
    widget: glib::WeakRef<gtk::Label>,
    text: String,
    parts: Vec<Formula>,
    epoch: Cell<u64>,
    failed: Cell<bool>,
    ready: RefCell<Vec<Rc<Surface>>>,
    bytes: Cell<usize>,
    font: Cell<f64>,
    base: RefCell<Option<pango::AttrList>>,
    applied: RefCell<Option<pango::AttrList>>,
    setting: Cell<bool>,
    tooltip: RefCell<Option<glib::GString>>,
    paintable: gtk::WidgetPaintable,
    draws: RefCell<Option<Rc<Cell<usize>>>>,
}
struct Ticket {
    id: usize,
    epoch: u64,
    index: usize,
    converting: bool,
    cancelled: bool,
}
pub struct Renderer {
    sender: mpsc::Sender<Message>,
    generation: u64,
    sequence: Cell<u64>,
    next: Cell<usize>,
    labels: RefCell<HashMap<usize, Weak<Label>>>,
    pending: RefCell<VecDeque<usize>>,
    active: RefCell<HashMap<u64, Ticket>>,
    dark: RefCell<Option<glib::SignalHandlerId>>,
    budget: Arc<AtomicUsize>,
    viewport: crate::viewport::Tracker,
    visibility: Rc<crate::viewport::FrameUpdate>,
}
#[cfg(debug_assertions)]
impl Renderer {
    pub fn smoke_info(&self, widget: &gtk::Label) -> Option<serde_json::Value> {
        let label = self
            .labels
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .find(|label| label.widget.upgrade().as_ref() == Some(widget))?;
        Some(
            serde_json::json!({"id":label.id,"epoch":label.epoch.get(),"parts":label.parts.len(),"ready":label.ready.borrow().len(),
            "failed":label.failed.get(),"draws":label.draws.borrow().as_ref().map_or(0,|n|n.get()),"font":label.font.get(),"surfaceBytes":self.budget.load(Ordering::Acquire)}),
        )
    }
    pub async fn smoke_recycling(
        self: &Rc<Self>,
        widget: &gtk::Label,
        markup: &str,
    ) -> Result<serde_json::Value, String> {
        let label = self
            .labels
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .find(|label| label.widget.upgrade().as_ref() == Some(widget))
            .ok_or("No mapped math registration")?;
        if !label.valid() {
            return Err("Math lifecycle widget is not mapped".into());
        }
        self.cancel(&label);
        let epoch = label.epoch.get();
        let serial = self
            .send(Request::CancelDiagram { id: label.id })
            .ok_or("No math command channel")?;
        self.active.borrow_mut().insert(
            serial,
            Ticket {
                id: label.id,
                epoch,
                index: 0,
                converting: false,
                cancelled: false,
            },
        );
        self.event(Response {
            key: KEY,
            generation: self.generation,
            serial,
            kind: "diagram",
            result: Ok(Reply::Diagram {
                id: label.id,
                image: crate::diagram::Image {
                    width: 1,
                    height: 1,
                    pixels: Arc::from([255, 0, 255, 255]),
                    vector: None,
                    math: Some(crate::math::Metrics {
                        width: 1.0,
                        height: 1.0,
                        baseline: 0.8,
                        em: 24.0,
                    }),
                },
            }),
        });
        // Reuse the same mapped label before its old asynchronous conversion
        // starts, then restore equal text. Text equality alone is insufficient.
        widget.set_markup("Replacement message");
        widget.set_markup(markup);
        let at = std::time::Instant::now();
        while label.ready.borrow().len() != label.parts.len()
            || label
                .ready
                .borrow()
                .iter()
                .any(|surface| surface.bytes <= 4)
        {
            if at.elapsed() > std::time::Duration::from_secs(15) {
                return Err(format!(
                    "Recycled math label did not render: {:?}",
                    self.smoke_info(widget)
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        if label.epoch.get() <= epoch || label.failed.get() {
            return Err("Old conversion crossed label generations".into());
        }
        widget.set_visible(false);
        if !label.ready.borrow().is_empty() {
            return Err("Unmapped math label retained surfaces".into());
        }
        widget.set_visible(true);
        let at = std::time::Instant::now();
        while label.ready.borrow().len() != label.parts.len() {
            if at.elapsed() > std::time::Duration::from_secs(15) {
                return Err("Remapped math label did not render".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        Ok(
            serde_json::json!({"lateConversionRejected":true,"equalTextGenerationGuard":true,"unmapReleased":true,"remapRendered":true}),
        )
    }
}
#[cfg(debug_assertions)]
pub fn smoke_visible_formulas(root: &gtk::Widget, text: &str) -> (usize, usize) {
    fn count(root: &gtk::Widget, text: &str, renderer: &Renderer) -> (usize, usize) {
        let mut result = (0, 0);
        if let Some(label) = root.downcast_ref::<gtk::Label>().filter(|l| {
            let label = *l;
            let scopes = crate::viewport::scopes(label);
            label.text() == text
                && crate::viewport::intersects(label, &scopes)
                && scopes.iter().all(|s| {
                    s.upgrade().is_some_and(|s| {
                        label.compute_bounds(&s).is_some_and(|r| {
                            let x = r.x() + r.width() / 2.;
                            let y = r.y() + r.height() / 2.;
                            x > 0. && x < s.width() as f32 && y > 0. && y < s.height() as f32
                        })
                    })
                })
        }) {
            result.0 = 1;
            result.1 = usize::from(renderer.smoke_info(label).is_some_and(|s| {
                s["parts"].as_u64().is_some_and(|n| n > 0)
                    && s["parts"] == s["ready"]
                    && s["failed"] == false
                    && s["draws"].as_u64().is_some_and(|n| n > 0)
            }));
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            let next = count(&widget, text, renderer);
            result.0 += next.0;
            result.1 += next.1;
        }
        result
    }
    ACTIVE.with(|active| {
        active
            .borrow()
            .upgrade()
            .map_or((0, 0), |r| count(root, text, &r))
    })
}
pub fn decorate(label: &gtk::Label, content: &studio_markdown_native::RichText) {
    if content.math.is_empty() || content.source_only {
        return;
    }
    ACTIVE.with(|active| {
        if let Some(owner) = active.borrow().upgrade() {
            owner.bind(label, &content.plain, content.math.iter());
        }
    });
}
/// Excerpts share worker-selected indices and original offsets, without cloning
/// the complete long cell or interpreting truncated TeX on the GTK thread.
pub fn decorate_excerpt(label: &gtk::Label, cell: &studio_markdown_native::Cell) {
    if cell.content.source_only || cell.preview_math.is_empty() {
        return;
    }
    let Some(text) = cell.preview.as_deref().and_then(|s| s.strip_suffix('…')) else {
        return;
    };
    if label.text() != text {
        return;
    }
    ACTIVE.with(|active| {
        if let Some(owner) = active.borrow().upgrade() {
            owner.bind(
                label,
                text,
                cell.preview_math
                    .iter()
                    .filter_map(|index| cell.content.math.get(*index)),
            );
        }
    });
}
impl Renderer {
    pub fn install(sender: mpsc::Sender<Message>) -> Rc<Self> {
        let owner = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            sender,
            generation: NEXT.fetch_add(1, Ordering::Relaxed),
            sequence: Cell::new(0),
            next: Cell::new(0),
            labels: RefCell::new(HashMap::new()),
            pending: RefCell::new(VecDeque::new()),
            active: RefCell::new(HashMap::new()),
            dark: RefCell::new(None),
            budget: BUDGET.get_or_init(|| Arc::new(AtomicUsize::new(0))).clone(),
            viewport: crate::viewport::Tracker::new(Rc::new({
                let weak = weak.clone();
                move || {
                    if let Some(owner) = weak.upgrade() {
                        owner.queue_visibility();
                    }
                }
            })),
            visibility: crate::viewport::FrameUpdate::new(Rc::new({
                let weak = weak.clone();
                move |clock| {
                    if let Some(owner) = weak.upgrade() {
                        owner.viewport_changed(clock);
                    }
                }
            })),
        });
        ACTIVE.with(|active| active.replace(Rc::downgrade(&owner)));
        let weak = Rc::downgrade(&owner);
        let signal = adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(owner) = weak.upgrade() {
                let labels = owner
                    .labels
                    .borrow()
                    .values()
                    .filter_map(Weak::upgrade)
                    .collect::<Vec<_>>();
                for label in labels {
                    owner.cancel(&label);
                    owner.enqueue(label.id);
                }
                owner.queue_visibility();
            }
        });
        owner.dark.replace(Some(signal));
        owner
    }
    fn visible(label: &Label) -> bool {
        label.widget.upgrade().is_some_and(|widget| {
            crate::viewport::intersects(&widget, &crate::viewport::scopes(&widget))
        })
    }
    fn watch(&self, widget: &gtk::Label) {
        self.viewport.watch(widget);
        self.visibility.queue(widget);
    }
    fn queue_visibility(&self) {
        for label in self.labels.borrow().values().filter_map(Weak::upgrade) {
            if let Some(widget) = label.widget.upgrade() {
                self.visibility.queue(&widget);
            }
        }
    }
    fn viewport_changed(self: &Rc<Self>, clock: &gdk::FrameClock) {
        let labels = self
            .labels
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for label in labels {
            if label
                .widget
                .upgrade()
                .is_some_and(|w| w.frame_clock().as_ref() == Some(clock))
            {
                self.enqueue(label.id);
            }
        }
    }
    fn send(&self, request: Request) -> Option<u64> {
        let serial = self.sequence.get().wrapping_add(1);
        self.sequence.set(serial);
        self.sender
            .try_send(Message {
                key: KEY,
                generation: self.generation,
                serial,
                request,
            })
            .ok()
            .map(|_| serial)
    }
    fn bind<'a>(
        self: &Rc<Self>,
        widget: &gtk::Label,
        text: &str,
        math: impl IntoIterator<Item = &'a studio_markdown_native::inline::Math>,
    ) {
        let parts = math
            .into_iter()
            .filter_map(|span| {
                text.get(span.body.clone())
                    .filter(|source| !source.contains(['\n', '\r']))
                    .map(|source| Formula {
                        range: span.range.clone(),
                        source: source.into(),
                        display: span.display,
                    })
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return;
        }
        let id = self.next.get().wrapping_add(1);
        self.next.set(id);
        let label = Rc::new(Label {
            id,
            owner: Rc::downgrade(self),
            widget: widget.downgrade(),
            text: text.to_owned(),
            parts,
            epoch: Cell::new(0),
            failed: Cell::new(false),
            ready: RefCell::new(Vec::new()),
            bytes: Cell::new(0),
            font: Cell::new(0.0),
            base: RefCell::new(widget.attributes()),
            applied: RefCell::new(widget.attributes()),
            setting: Cell::new(false),
            tooltip: RefCell::new(None),
            paintable: gtk::WidgetPaintable::new(None::<&gtk::Widget>),
            draws: RefCell::new(None),
        });
        self.labels
            .borrow_mut()
            .retain(|_, label| label.strong_count() > 0);
        self.labels.borrow_mut().insert(id, Rc::downgrade(&label));
        let current = label.clone();
        widget.connect_map(move |widget| {
            current.paintable.set_widget(Some(widget));
            if let Some(owner) = current.owner.upgrade() {
                owner.watch(widget);
                owner.enqueue(current.id);
            }
        });
        let current = label.clone();
        widget.connect_unmap(move |_| {
            current.paintable.set_widget(None::<&gtk::Widget>);
            if let Some(owner) = current.owner.upgrade() {
                owner.cancel(&current);
                owner.pump();
            }
        });
        let current = label.clone();
        widget.connect_notify_local(Some("label"), move |widget, _| {
            if let Some(owner) = current.owner.upgrade() {
                if widget.text() != current.text {
                    owner.cancel(&current);
                } else {
                    current.refresh(true);
                    owner.enqueue(current.id);
                }
            }
        });
        let current = label.clone();
        widget.connect_notify_local(Some("attributes"), move |widget, _| {
            if current.setting.get() || same(&widget.attributes(), &current.applied.borrow()) {
                return;
            }
            current.base.replace(widget.attributes());
            current.refresh(true);
        });
        let weak = Rc::downgrade(&label);
        label.paintable.connect_invalidate_size(move |_| {
            if let Some(label) = weak.upgrade() {
                if !label.setting.get() {
                    label.refresh(false);
                    if let Some(owner) = label.owner.upgrade() {
                        if let Some(widget) = label.widget.upgrade() {
                            owner.watch(&widget);
                        }
                    }
                }
            }
        });
        if widget.is_mapped() {
            label.paintable.set_widget(Some(widget));
            self.watch(widget);
            self.enqueue(id);
        }
    }
    fn cancel(&self, label: &Label) {
        label.epoch.set(label.epoch.get().wrapping_add(1));
        label.failed.set(false);
        self.pending.borrow_mut().retain(|id| *id != label.id);
        let mut cancel = false;
        self.active.borrow_mut().retain(|_, ticket| {
            if ticket.id != label.id {
                return true;
            }
            cancel = true;
            if ticket.converting {
                ticket.cancelled = true;
                true
            } else {
                false
            }
        });
        if cancel {
            self.send(Request::CancelDiagram { id: label.id });
        }
        label.source();
        label.ready.borrow_mut().clear();
        label.bytes.set(0);
    }
    fn enqueue(self: &Rc<Self>, id: usize) {
        let label = self.labels.borrow().get(&id).and_then(Weak::upgrade);
        let Some(label) = label.filter(|l| {
            l.valid()
                && Self::visible(l)
                && !l.failed.get()
                && l.ready.borrow().len() < l.parts.len()
        }) else {
            return;
        };
        if self.pending.borrow().contains(&id)
            || self
                .active
                .borrow()
                .values()
                .any(|t| t.id == id && !t.cancelled)
        {
            return;
        }
        if self.pending.borrow().len() >= 128 {
            self.fail(
                &label,
                "Too many visible formulas; reopen this section to retry",
            );
            return;
        }
        self.pending.borrow_mut().push_back(id);
        self.pump();
    }
    fn pump(self: &Rc<Self>) {
        let mut remaining = self.pending.borrow().len();
        while self.active.borrow().len() < 4 && remaining > 0 {
            remaining -= 1;
            let Some(id) = self.pending.borrow_mut().pop_front() else {
                break;
            };
            if self.active.borrow().values().any(|t| t.id == id) {
                self.pending.borrow_mut().push_back(id);
                continue;
            }
            let label = self.labels.borrow().get(&id).and_then(Weak::upgrade);
            let Some(label) = label.filter(|l| l.valid() && Self::visible(l) && !l.failed.get())
            else {
                continue;
            };
            let index = label.ready.borrow().len();
            let Some(part) = label.parts.get(index) else {
                continue;
            };
            if let Some(serial) = self.send(Request::Diagram {
                id,
                kind: if part.display {
                    "math-inline-display"
                } else {
                    "math-inline"
                }
                .into(),
                source: part.source.clone(),
                dark: adw::StyleManager::default().is_dark(),
                style: Default::default(),
            }) {
                self.active.borrow_mut().insert(
                    serial,
                    Ticket {
                        id,
                        epoch: label.epoch.get(),
                        index,
                        converting: false,
                        cancelled: false,
                    },
                );
            } else {
                self.fail(
                    &label,
                    "Render queue is unavailable; reopen this section to retry",
                );
            }
        }
        self.labels
            .borrow_mut()
            .retain(|_, label| label.strong_count() > 0);
    }
    fn fail(&self, label: &Label, error: &str) {
        self.cancel(label);
        label.failed.set(true);
        if let Some(widget) = label.widget.upgrade() {
            label.tooltip.replace(widget.tooltip_text());
            widget.set_tooltip_text(Some(&format!(
                "Inline math: {error}. Original TeX is retained."
            )));
        }
    }
    pub fn event(self: &Rc<Self>, response: Response) {
        if response.key != KEY
            || response.generation != self.generation
            || response.kind != "diagram"
        {
            return;
        }
        let serial = response.serial;
        let id = {
            let active = self.active.borrow();
            let Some(ticket) = active
                .get(&serial)
                .filter(|t| !t.converting && !t.cancelled)
            else {
                return;
            };
            ticket.id
        };
        match response.result {
            Ok(Reply::Diagram {
                id: reply_id,
                image,
            }) if reply_id == id && image.pixels.len() <= crate::inline_math_shape::IMAGE_BYTES => {
                if let Some(ticket) = self.active.borrow_mut().get_mut(&serial) {
                    ticket.converting = true;
                }
                let weak = Rc::downgrade(self);
                glib::spawn_future_local(async move {
                    let prepared = gio::spawn_blocking(move || Prepared::new(image))
                        .await
                        .map_err(|_| "Inline pixel worker stopped".to_string())
                        .and_then(|v| v);
                    if let Some(owner) = weak.upgrade() {
                        owner.prepared(serial, prepared);
                    }
                });
            }
            result => {
                self.active.borrow_mut().remove(&serial);
                if let Some(label) = self.labels.borrow().get(&id).and_then(Weak::upgrade) {
                    self.fail(
                        &label,
                        &match result {
                            Err(error) => error,
                            _ => "Formula exceeds inline display limits".into(),
                        },
                    );
                }
                self.pump();
            }
        }
    }
    fn prepared(self: &Rc<Self>, serial: u64, result: Result<Prepared, String>) {
        let Some(ticket) = self.active.borrow_mut().remove(&serial) else {
            return;
        };
        let label = self.labels.borrow().get(&ticket.id).and_then(Weak::upgrade);
        if let Some(label) = label.filter(|l| {
            !ticket.cancelled
                && l.epoch.get() == ticket.epoch
                && l.valid()
                && l.ready.borrow().len() == ticket.index
        }) {
            let result = result.and_then(|image| {
                if label.bytes.get() + image.pixels.len() > GROUP_BYTES {
                    return Err("Formula group exceeds inline display limits".into());
                }
                Surface::new(image, Some(self.budget.clone()))
            });
            match result {
                Ok(surface) => {
                    label.bytes.set(label.bytes.get() + surface.bytes);
                    label.ready.borrow_mut().push(surface);
                    if label.ready.borrow().len() == label.parts.len() {
                        label.refresh(true);
                    } else {
                        self.enqueue(label.id);
                    }
                }
                Err(error) => self.fail(&label, &error),
            }
        }
        self.pump();
    }
}
fn same(a: &Option<pango::AttrList>, b: &Option<pango::AttrList>) -> bool {
    use glib::translate::*;
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let a: *mut pango::ffi::PangoAttrList = a.to_glib_none().0;
            let b: *mut pango::ffi::PangoAttrList = b.to_glib_none().0;
            a == b
        }
        _ => false,
    }
}
impl Label {
    fn valid(&self) -> bool {
        self.widget
            .upgrade()
            .is_some_and(|label| label.is_mapped() && label.text() == self.text)
    }
    fn source(&self) {
        self.font.set(0.0);
        self.draws.borrow_mut().take();
        if let Some(label) = self.widget.upgrade() {
            self.setting.set(true);
            label.set_attributes(self.base.borrow().as_ref());
            self.applied.replace(label.attributes());
            self.setting.set(false);
            if label
                .tooltip_text()
                .is_some_and(|text| text.starts_with("Inline math:"))
            {
                label.set_tooltip_text(self.tooltip.borrow().as_deref());
            }
        }
    }
    fn refresh(&self, force: bool) {
        if !self.valid() || self.ready.borrow().len() != self.parts.len() {
            return;
        }
        let Some(label) = self.widget.upgrade() else {
            return;
        };
        let layout = label.layout();
        let mut run = layout.iter();
        let wanted = self.parts.first().map_or(0, |part| part.range.start);
        let font = loop {
            if let Some(glyphs) = run.run_readonly() {
                let item = glyphs.item();
                if wanted >= item.offset() as usize
                    && wanted < (item.offset() + item.length()) as usize
                {
                    break Some(item.analysis().font().describe_with_absolute_size());
                }
            }
            if !run.next_run() {
                break None;
            }
        }
        .or_else(|| label.pango_context().font_description());
        let Some(font) = font else {
            return;
        };
        let pixels = f64::from(font.size()) / f64::from(pango::SCALE)
            * if font.is_size_absolute() {
                1.0
            } else {
                96.0 / 72.0
            };
        if !force && (pixels - self.font.get()).abs() < 0.01 {
            return;
        }
        let surfaces = self
            .parts
            .iter()
            .zip(self.ready.borrow().iter())
            .map(|(part, image)| (part.range.clone(), image.clone()))
            .collect::<Vec<_>>();
        self.setting.set(true);
        let result = crate::inline_math_shape::apply_surfaces(
            &label,
            &surfaces,
            pixels,
            self.base.borrow().as_ref(),
            false,
        );
        self.applied.replace(label.attributes());
        self.setting.set(false);
        match result {
            Ok(draws) => {
                self.font.set(pixels);
                self.draws.replace(Some(draws));
            }
            Err(error) => {
                if let Some(owner) = self.owner.upgrade() {
                    owner.fail(self, &error);
                }
            }
        }
    }
}
impl Drop for Label {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            owner.cancel(self);
            owner.labels.borrow_mut().remove(&self.id);
        }
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some(signal) = self.dark.get_mut().take() {
            adw::StyleManager::default().disconnect(signal);
        }
        self.send(Request::CancelDiagrams);
    }
}
