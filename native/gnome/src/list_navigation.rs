//! Coordinate explicit list navigation with deferred model-update scrolling.
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub struct Navigation {
    generation: Cell<u64>,
    pending: RefCell<Option<glib::SourceId>>,
    alignment: RefCell<Option<gtk::TickCallbackId>>,
    adjustment: glib::WeakRef<gtk::Adjustment>,
    // GTK may not update the adjustment until the selected row is allocated.
    // Keep subsequent model updates from mistaking that old offset for consent
    // to follow. Returning to the end after leaving it resumes normal following.
    reading: Cell<bool>,
    departed: Cell<bool>,
}
impl Navigation {
    pub fn new(scroll: &gtk::ScrolledWindow) -> Rc<Self> {
        let adjustment = scroll.vadjustment();
        let navigation = Rc::new(Self {
            adjustment: adjustment.downgrade(),
            generation: Cell::new(0),
            pending: RefCell::new(None),
            alignment: RefCell::new(None),
            reading: Cell::new(false),
            departed: Cell::new(false),
        });
        for changed in [false, true] {
            let weak = Rc::downgrade(&navigation);
            let observe = move |adjustment: &gtk::Adjustment| {
                if let Some(navigation) = weak.upgrade() {
                    if navigation.reading.get() {
                        if !Self::at_end(adjustment) {
                            navigation.departed.set(true);
                        } else if navigation.departed.get() {
                            navigation.reading.set(false);
                        }
                    }
                }
            };
            if changed {
                adjustment.connect_changed(observe);
            } else {
                adjustment.connect_value_changed(observe);
            }
        }
        let scrolling = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scrolling.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&navigation);
        scrolling.connect_scroll(move |_, _, _| {
            if let Some(navigation) = weak.upgrade() {
                navigation.cancel();
            }
            glib::Propagation::Proceed
        });
        scroll.add_controller(scrolling);
        navigation
    }
    fn at_end(adjustment: &gtk::Adjustment) -> bool {
        adjustment.value() + adjustment.page_size() >= adjustment.upper() - 80.0
    }
    fn begin_reading(&self) {
        self.reading.set(true);
        self.departed
            .set(self.adjustment.upgrade().is_some_and(|a| !Self::at_end(&a)));
    }
    pub fn reset(&self) {
        self.cancel();
        self.reading.set(false);
        self.departed.set(false);
    }
    pub fn cancel(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        let pending = self.pending.borrow_mut().take();
        if let Some(pending) = pending {
            pending.remove();
        }
        if let Some(alignment) = self.alignment.borrow_mut().take() {
            alignment.remove();
        }
    }
    pub fn jump(&self, view: &gtk::ListView, position: u32, flags: gtk::ListScrollFlags) {
        self.cancel();
        if view.model().is_some_and(|model| position < model.n_items()) {
            self.begin_reading();
            view.scroll_to(position, flags, None);
        }
    }
    /// Ensure the row is realized, then align its actual allocation. GTK's
    /// default ensure-visible behavior may expose only a header at the bottom.
    /// A short, scoped frame observer accommodates virtual-list allocation;
    /// explicit navigation/reset and wheel input always retire it.
    pub fn jump_start(
        self: &Rc<Self>,
        view: &gtk::ListView,
        scroll: &gtk::ScrolledWindow,
        position: u32,
        anchor: impl Fn() -> Option<gtk::Widget> + 'static,
    ) {
        self.jump(view, position, gtk::ListScrollFlags::NONE);
        let generation = self.generation.get();
        let weak = Rc::downgrade(self);
        let viewport = scroll.downgrade();
        let frames = Cell::new(0u8);
        let settled = Cell::new(0u8);
        let previous_offset = Cell::new(scroll.vadjustment().value());
        let callback = view.add_tick_callback(move |_, _| {
            let Some(owner) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if owner.generation.get() != generation {
                return glib::ControlFlow::Break;
            }
            frames.set(frames.get() + 1);
            if let Some(viewport) = viewport.upgrade() {
                let adjustment = viewport.vadjustment();
                let current_offset = adjustment.value();
                // Tick callbacks run before layout. An offset changed since
                // the previous frame can still have the old row allocation;
                // allow a layout before combining its bounds with this offset.
                let stable_offset =
                    (previous_offset.replace(current_offset) - current_offset).abs() <= 1.0;
                if frames.get() >= 2 && stable_offset {
                    if let Some(bounds) = anchor()
                        .filter(|w| w.is_mapped())
                        .and_then(|w| w.compute_bounds(&viewport))
                    {
                        if bounds.height() > 0.0 {
                            let value = (adjustment.value() + bounds.y() as f64 - 16.0)
                                .clamp(0.0, (adjustment.upper() - adjustment.page_size()).max(0.0));
                            if (value - adjustment.value()).abs() <= 1.0 {
                                settled.set(settled.get() + 1);
                            } else {
                                settled.set(0);
                                adjustment.set_value(value);
                            }
                        }
                    }
                }
            }
            if settled.get() >= 2 || frames.get() >= 12 {
                owner.alignment.borrow_mut().take();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
        self.alignment.replace(Some(callback));
    }
    pub fn defer_edge(
        self: &Rc<Self>,
        view: &gtk::ListView,
        scroll: &gtk::ScrolledWindow,
        end: bool,
    ) {
        if end {
            self.reading.set(false);
        } else {
            self.begin_reading();
        }
        self.queue_edge(view, scroll, end);
    }
    pub fn follow_tail(self: &Rc<Self>, view: &gtk::ListView, scroll: &gtk::ScrolledWindow) {
        if !self.reading.get() {
            self.queue_edge(view, scroll, true);
        }
    }
    fn queue_edge(self: &Rc<Self>, view: &gtk::ListView, scroll: &gtk::ScrolledWindow, end: bool) {
        self.cancel();
        let Some(model) = view.model() else {
            return;
        };
        let Some(last) = model.n_items().checked_sub(1) else {
            return;
        };
        let position = if end { last } else { 0 };
        let Some(expected) = model.item(position) else {
            return;
        };
        let generation = self.generation.get();
        let owner = Rc::downgrade(self);
        let view = view.downgrade();
        let scroll = scroll.downgrade();
        let pending = glib::idle_add_local_once(move || {
            let Some(owner) = owner.upgrade() else {
                return;
            };
            owner.pending.borrow_mut().take();
            let (Some(view), Some(scroll)) = (view.upgrade(), scroll.upgrade()) else {
                return;
            };
            if generation != owner.generation.get()
                || view.model().and_then(|model| model.item(position)).as_ref() != Some(&expected)
            {
                return;
            }
            view.scroll_to(position, gtk::ListScrollFlags::NONE, None);
            // A GTK signal during binding may itself navigate or change scope.
            if generation != owner.generation.get() {
                return;
            }
            let adjustment = scroll.vadjustment();
            adjustment.set_value(if end {
                (adjustment.upper() - adjustment.page_size()).max(0.0)
            } else {
                0.0
            });
        });
        self.pending.replace(Some(pending));
    }
}
impl Drop for Navigation {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.get_mut().take() {
            pending.remove();
        }
        if let Some(alignment) = self.alignment.get_mut().take() {
            alignment.remove();
        }
    }
}
