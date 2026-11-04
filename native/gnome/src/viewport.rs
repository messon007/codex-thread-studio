//! Shared native clipping and adjustment observation. GTK owns geometry; this
//! module schedules no polling and never forces widget allocation.
use gtk::{glib, prelude::*};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub type Scopes = Vec<glib::WeakRef<gtk::ScrolledWindow>>;
pub fn scopes(widget: &impl IsA<gtk::Widget>) -> Scopes {
    let mut result = Vec::new();
    let mut parent = widget.as_ref().parent();
    while let Some(widget) = parent {
        parent = widget.parent();
        if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
            result.push(scroll.downgrade());
        }
    }
    result
}
pub fn intersects(widget: &impl IsA<gtk::Widget>, scopes: &Scopes) -> bool {
    let widget = widget.as_ref();
    widget.is_mapped()
        && widget.width() > 0
        && widget.height() > 0
        && scopes.iter().all(|scope| {
            scope.upgrade().is_some_and(|scroll| {
                widget.compute_bounds(&scroll).is_some_and(|b| {
                    b.width() > 0.0
                        && b.height() > 0.0
                        && b.x() < scroll.width() as f32
                        && b.y() < scroll.height() as f32
                        && b.x() + b.width() > 0.0
                        && b.y() + b.height() > 0.0
                })
            })
        })
}
type Adjustment = (glib::WeakRef<gtk::Adjustment>, [glib::SignalHandlerId; 2]);
pub struct Tracker {
    watched: RefCell<HashMap<usize, Adjustment>>,
    changed: Rc<dyn Fn()>,
}
impl Tracker {
    pub fn new(changed: Rc<dyn Fn()>) -> Self {
        Self {
            watched: RefCell::default(),
            changed,
        }
    }
    pub fn watch(&self, widget: &impl IsA<gtk::Widget>) -> Scopes {
        let scopes = scopes(widget);
        for scroll in scopes.iter().filter_map(glib::WeakRef::upgrade) {
            self.adjustment(&scroll.hadjustment());
            self.adjustment(&scroll.vadjustment());
        }
        scopes
    }
    fn adjustment(&self, adjustment: &gtk::Adjustment) {
        let key = adjustment.as_ptr() as usize;
        let mut watched = self.watched.borrow_mut();
        watched.retain(|_, (a, _)| a.upgrade().is_some());
        if watched.contains_key(&key) {
            return;
        }
        let changed = self.changed.clone();
        let size = adjustment.connect_changed(move |_| changed());
        let changed = self.changed.clone();
        let value = adjustment.connect_value_changed(move |_| changed());
        watched.insert(key, (adjustment.downgrade(), [size, value]));
    }
    pub fn clear(&self) {
        for (weak, handlers) in self.watched.borrow_mut().drain().map(|(_, v)| v) {
            if let Some(adjustment) = weak.upgrade() {
                for handler in handlers {
                    adjustment.disconnect(handler);
                }
            }
        }
    }
}
impl Drop for Tracker {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Coalesce viewport work after each affected window's real layout/paint cycle.
/// A timeout can fire before GTK updates GtkViewport's scroll transform. These
/// handlers run once, then disconnect; inactive windows create no polling work.
pub struct FrameUpdate {
    owner: std::rc::Weak<Self>,
    changed: Rc<dyn Fn(&gtk::gdk::FrameClock)>,
    frames: RefCell<HashMap<usize, (glib::WeakRef<gtk::gdk::FrameClock>, glib::SignalHandlerId)>>,
}
impl FrameUpdate {
    pub fn new(changed: Rc<dyn Fn(&gtk::gdk::FrameClock)>) -> Rc<Self> {
        Rc::new_cyclic(|owner| Self {
            owner: owner.clone(),
            changed,
            frames: RefCell::default(),
        })
    }
    pub fn queue(&self, widget: &impl IsA<gtk::Widget>) {
        let Some(clock) = widget
            .as_ref()
            .frame_clock()
            .filter(|_| widget.as_ref().is_mapped())
        else {
            return;
        };
        let key = clock.as_ptr() as usize;
        let mut frames = self.frames.borrow_mut();
        frames.retain(|_, (clock, _)| clock.upgrade().is_some());
        if frames.contains_key(&key) {
            return;
        }
        let owner = self.owner.clone();
        let id = clock.connect_after_paint(move |clock| {
            if let Some(owner) = owner.upgrade() {
                let entry = owner.frames.borrow_mut().remove(&(clock.as_ptr() as usize));
                if let Some((_, id)) = entry {
                    clock.disconnect(id);
                }
                (owner.changed)(clock);
            }
        });
        frames.insert(key, (clock.downgrade(), id));
        clock.request_phase(gtk::gdk::FrameClockPhase::PAINT);
    }
    pub fn clear(&self) {
        for (clock, id) in self.frames.borrow_mut().drain().map(|(_, v)| v) {
            if let Some(clock) = clock.upgrade() {
                clock.disconnect(id);
            }
        }
    }
    #[cfg(debug_assertions)]
    pub fn pending(&self) -> bool {
        !self.frames.borrow().is_empty()
    }
}
impl Drop for FrameUpdate {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Reusable viewport admission for expensive native children. Geometry is read
/// after the window paints; scroll/size/map changes coalesce without idle polling.
pub struct Visibility {
    widget: glib::WeakRef<gtk::Widget>,
    tracker: Tracker,
    frames: std::rc::Rc<FrameUpdate>,
    _paintable: gtk::WidgetPaintable,
    last: std::cell::Cell<Option<bool>>,
    changed: std::rc::Rc<dyn Fn(bool)>,
}
impl Visibility {
    pub fn new(
        widget: &impl IsA<gtk::Widget>,
        changed: std::rc::Rc<dyn Fn(bool)>,
    ) -> std::rc::Rc<Self> {
        let widget = widget.as_ref();
        let owner = std::rc::Rc::new_cyclic(|weak: &std::rc::Weak<Self>| Self {
            widget: widget.downgrade(),
            tracker: Tracker::new(std::rc::Rc::new({
                let weak = weak.clone();
                move || {
                    if let Some(owner) = weak.upgrade() {
                        owner.queue();
                    }
                }
            })),
            frames: FrameUpdate::new(std::rc::Rc::new({
                let weak = weak.clone();
                move |_| {
                    if let Some(owner) = weak.upgrade() {
                        owner.update();
                    }
                }
            })),
            _paintable: gtk::WidgetPaintable::new(Some(widget)),
            last: std::cell::Cell::new(None),
            changed,
        });
        let weak = std::rc::Rc::downgrade(&owner);
        widget.connect_map(move |_| {
            if let Some(owner) = weak.upgrade() {
                owner.watch();
            }
        });
        let weak = std::rc::Rc::downgrade(&owner);
        widget.connect_unmap(move |_| {
            if let Some(owner) = weak.upgrade() {
                owner.publish(false);
            }
        });
        let weak = std::rc::Rc::downgrade(&owner);
        owner._paintable.connect_invalidate_size(move |_| {
            if let Some(owner) = weak.upgrade() {
                owner.watch();
            }
        });
        owner.watch();
        owner
    }
    fn watch(&self) {
        if let Some(widget) = self.widget.upgrade() {
            self.tracker.watch(&widget);
            self.frames.queue(&widget);
        }
    }
    fn queue(&self) {
        if let Some(widget) = self.widget.upgrade() {
            self.frames.queue(&widget);
        }
    }
    fn update(&self) {
        let visible = self
            .widget
            .upgrade()
            .is_some_and(|w| intersects(&w, &scopes(&w)));
        self.publish(visible);
    }
    fn publish(&self, visible: bool) {
        if self.last.replace(Some(visible)) != Some(visible) {
            (self.changed)(visible);
        }
    }
    #[cfg(debug_assertions)]
    pub fn pending(&self) -> bool {
        self.frames.pending()
    }
}
