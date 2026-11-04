//! Reveal a newly loaded text context after GTK owns its real layout. A pending
//! request belongs to one buffer/selection and yields immediately to user input.
use adw::prelude::*;
use gtk::{gdk, glib};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Target {
    buffer: glib::WeakRef<gtk::TextBuffer>,
    marks: (i32, i32),
    start: bool,
}
pub struct Reveal {
    view: glib::WeakRef<gtk::TextView>,
    target: RefCell<Option<Target>>,
    frame: Rc<crate::viewport::FrameUpdate>,
    watch: RefCell<Option<(glib::WeakRef<gtk::Adjustment>, [glib::SignalHandlerId; 2])>>,
}
impl Reveal {
    pub fn new(view: &impl IsA<gtk::TextView>) -> Rc<Self> {
        let view = view.as_ref();
        let state = Rc::new_cyclic(|weak: &std::rc::Weak<Self>| {
            let owner = weak.clone();
            Self {
                view: view.downgrade(),
                target: RefCell::new(None),
                frame: crate::viewport::FrameUpdate::new(Rc::new(move |_| {
                    if let Some(state) = owner.upgrade() {
                        state.apply();
                    }
                })),
                watch: RefCell::new(None),
            }
        });
        let weak = Rc::downgrade(&state);
        view.connect_map(move |view| {
            if let Some(state) = weak.upgrade() {
                state.watch(view);
                if state.target.borrow().is_some() {
                    state.frame.queue(view);
                }
            }
        });
        let weak = Rc::downgrade(&state);
        view.connect_unmap(move |_| {
            if let Some(state) = weak.upgrade() {
                state.cancel();
            }
        });
        let weak = Rc::downgrade(&state);
        view.connect_buffer_notify(move |_| {
            if let Some(state) = weak.upgrade() {
                state.cancel();
            }
        });
        let weak = Rc::downgrade(&state);
        view.buffer().connect_changed(move |_| {
            if let Some(state) = weak.upgrade() {
                state.cancel();
            }
        });
        let input = gtk::EventControllerLegacy::new();
        input.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&state);
        input.connect_event(move |_, event| {
            if matches!(
                event.event_type(),
                gdk::EventType::ButtonPress
                    | gdk::EventType::KeyPress
                    | gdk::EventType::Scroll
                    | gdk::EventType::TouchBegin
            ) {
                if let Some(state) = weak.upgrade() {
                    state.cancel();
                }
            }
            glib::Propagation::Proceed
        });
        view.parent()
            .unwrap_or_else(|| view.clone().upcast())
            .add_controller(input);
        state
    }
    fn unwatch(&self) {
        if let Some((weak, ids)) = self.watch.borrow_mut().take() {
            if let Some(adjustment) = weak.upgrade() {
                for id in ids {
                    adjustment.disconnect(id);
                }
            }
        }
    }
    fn watch(self: &Rc<Self>, view: &gtk::TextView) {
        let Some(adjustment) = view.vadjustment() else {
            return;
        };
        if self
            .watch
            .borrow()
            .as_ref()
            .is_some_and(|(a, _)| a.upgrade().as_ref() == Some(&adjustment))
        {
            return;
        }
        self.unwatch();
        let weak = Rc::downgrade(self);
        let changed = adjustment.connect_changed(move |_| {
            if let Some(state) = weak.upgrade() {
                state.queue_pending();
            }
        });
        let weak = Rc::downgrade(self);
        let value = adjustment.connect_value_changed(move |_| {
            if let Some(state) = weak.upgrade() {
                state.queue_pending();
            }
        });
        self.watch
            .replace(Some((adjustment.downgrade(), [changed, value])));
    }
    fn queue_pending(&self) {
        if self.target.borrow().is_some() {
            if let Some(view) = self.view.upgrade() {
                self.frame.queue(&view);
            }
        }
    }
    pub fn queue(self: &Rc<Self>, start: bool) {
        let Some(view) = self.view.upgrade() else {
            return;
        };
        let buffer = view.buffer();
        self.watch(&view);
        let marks = (
            buffer.cursor_position(),
            buffer.iter_at_mark(&buffer.selection_bound()).offset(),
        );
        self.target.replace(Some(Target {
            buffer: buffer.downgrade(),
            marks,
            start,
        }));
        self.frame.queue(&view);
    }
    fn cancel(&self) {
        self.target.borrow_mut().take();
        self.frame.clear();
    }
    fn apply(&self) {
        let Some(target) = self.target.borrow().clone() else {
            return;
        };
        let Some(view) = self.view.upgrade().filter(|v| v.is_mapped()) else {
            return;
        };
        let buffer = view.buffer();
        if target.buffer.upgrade().as_ref() != Some(&buffer)
            || target.marks
                != (
                    buffer.cursor_position(),
                    buffer.iter_at_mark(&buffer.selection_bound()).offset(),
                )
        {
            self.cancel();
            return;
        }
        let Some(adjustment) = view.vadjustment() else {
            return;
        };
        let visible = view.visible_rect();
        let cursor = view.iter_location(&buffer.iter_at_mark(&buffer.get_insert()));
        let end = view.iter_location(&buffer.end_iter());
        let wanted = if target.start || end.y() + end.height() <= visible.height() {
            adjustment.lower()
        } else if cursor.y() < visible.y() {
            adjustment.value() + f64::from(cursor.y() - visible.y())
        } else if cursor.y() + cursor.height() > visible.y() + visible.height() {
            adjustment.value()
                + f64::from(cursor.y() + cursor.height() - visible.y() - visible.height())
        } else {
            return;
        };
        let maximum = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        adjustment.set_value(wanted.clamp(adjustment.lower(), maximum));
    }
}
impl Drop for Reveal {
    fn drop(&mut self) {
        self.unwatch();
    }
}
