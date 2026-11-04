//! Keep native Home/End navigation at its endpoint while deferred anchors resize.
//! Adjustment changes schedule one real frame; user input ends the hold. No poll.
use adw::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
#[derive(Clone, Copy)]
enum Edge {
    Start,
    End,
}
type Watch = (glib::WeakRef<gtk::Adjustment>, [glib::SignalHandlerId; 2]);
struct State {
    owner: Weak<Self>,
    view: glib::WeakRef<gtk::TextView>,
    buffer: glib::WeakRef<gtk::TextBuffer>,
    edge: Cell<Option<Edge>>,
    applying: Cell<bool>,
    frame: Rc<crate::viewport::FrameUpdate>,
    watch: RefCell<Option<Watch>>,
    input: RefCell<Option<(glib::WeakRef<gtk::Widget>, gtk::EventControllerLegacy)>>,
}
impl State {
    fn current(&self) -> Option<gtk::TextView> {
        self.view
            .upgrade()
            .filter(|v| v.is_mapped() && self.buffer.upgrade().is_some_and(|b| v.buffer() == b))
    }
    fn cancel(&self) {
        self.edge.set(None);
        self.frame.clear();
    }
    fn queue(&self) {
        if self.edge.get().is_some() {
            if let Some(view) = self.current() {
                self.frame.queue(&view);
            }
        }
    }
    fn apply(&self) {
        let Some(edge) = self.edge.get() else { return };
        let Some(view) = self.current() else {
            self.cancel();
            return;
        };
        let buffer = view.buffer();
        let wanted = match edge {
            Edge::Start => 0,
            Edge::End => buffer.char_count(),
        };
        // Let GTK's normal key handler own caret movement/Shift selection.
        // A changed caret, including accessibility input, supersedes the hold.
        if buffer.cursor_position() != wanted {
            self.cancel();
            return;
        }
        if let Some(adjustment) = view.vadjustment() {
            let value = match edge {
                Edge::Start => adjustment.lower(),
                Edge::End => (adjustment.upper() - adjustment.page_size()).max(adjustment.lower()),
            };
            self.applying.set(true);
            adjustment.set_value(value);
            self.applying.set(false);
        }
    }
    fn detach(&self) {
        self.cancel();
        if let Some((owner, controller)) = self.input.borrow_mut().take() {
            if let Some(owner) = owner.upgrade() {
                owner.remove_controller(&controller);
            }
        }
        if let Some((adjustment, ids)) = self.watch.borrow_mut().take() {
            if let Some(adjustment) = adjustment.upgrade() {
                for id in ids {
                    adjustment.disconnect(id);
                }
            }
        }
    }
    fn mapped(self: &Rc<Self>, view: &gtk::TextView) {
        self.detach();
        self.watch_adjustment(view);
        self.attach_input(view);
    }
    fn watch_adjustment(&self, view: &gtk::TextView) {
        if let Some((adjustment, ids)) = self.watch.borrow_mut().take() {
            if let Some(adjustment) = adjustment.upgrade() {
                for id in ids {
                    adjustment.disconnect(id);
                }
            }
        }
        if let Some(adjustment) = view.vadjustment() {
            let owner = self.owner.clone();
            let changed = adjustment.connect_changed(move |_| {
                if let Some(state) = owner.upgrade() {
                    state.queue();
                }
            });
            let owner = self.owner.clone();
            let value = adjustment.connect_value_changed(move |_| {
                if let Some(state) = owner.upgrade() {
                    // GTK may correct the value after reallocating a child,
                    // without a corresponding range change. Hold the edge
                    // through that correction; captured user input cancels it.
                    if !state.applying.get() {
                        state.queue();
                    }
                }
            });
            self.watch
                .replace(Some((adjustment.downgrade(), [changed, value])));
        }
        self.queue();
    }
    fn attach_input(&self, view: &gtk::TextView) {
        let mut host: gtk::Widget = view.clone().upcast();
        let mut parent = host.parent();
        while let Some(widget) = parent {
            parent = widget.parent();
            if widget.is::<gtk::ScrolledWindow>() {
                host = widget;
                break;
            }
        }
        let input = gtk::EventControllerLegacy::new();
        input.set_propagation_phase(gtk::PropagationPhase::Capture);
        let owner = self.owner.clone();
        input.connect_event(move |_, event| {
            if let Some(state) = owner.upgrade() {
                match event.event_type() {
                    gdk::EventType::KeyPress => {
                        let edge = state.current().filter(|v| v.has_focus()).and_then(|_| {
                            let key = event.downcast_ref::<gdk::KeyEvent>()?;
                            let modifiers = event.modifier_state();
                            if !modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                                || modifiers.intersects(
                                    gdk::ModifierType::ALT_MASK
                                        | gdk::ModifierType::SUPER_MASK
                                        | gdk::ModifierType::META_MASK,
                                )
                            {
                                return None;
                            }
                            match key.keyval() {
                                gdk::Key::Home | gdk::Key::KP_Home => Some(Edge::Start),
                                gdk::Key::End | gdk::Key::KP_End => Some(Edge::End),
                                _ => None,
                            }
                        });
                        state.cancel();
                        state.edge.set(edge);
                        state.queue();
                    }
                    gdk::EventType::ButtonPress
                    | gdk::EventType::TouchBegin
                    | gdk::EventType::Scroll => state.cancel(),
                    _ => {}
                }
            }
            glib::Propagation::Proceed
        });
        host.add_controller(input.clone());
        self.input.replace(Some((host.downgrade(), input)));
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.detach();
    }
}
pub fn bind(view: &gtk::TextView) {
    let state: Rc<State> = Rc::new_cyclic(|owner: &Weak<State>| {
        let weak = owner.clone();
        State {
            owner: owner.clone(),
            view: view.downgrade(),
            buffer: view.buffer().downgrade(),
            edge: Cell::new(None),
            applying: Cell::new(false),
            frame: crate::viewport::FrameUpdate::new(Rc::new(move |_| {
                if let Some(state) = weak.upgrade() {
                    state.apply();
                }
            })),
            watch: RefCell::new(None),
            input: RefCell::new(None),
        }
    });
    let owner = state.clone();
    view.connect_map(move |view| owner.mapped(view));
    let owner = state.clone();
    view.connect_unmap(move |_| owner.detach());
    let owner = Rc::downgrade(&state);
    // GtkScrolledWindow can replace this property after the child maps.
    // Observing only the adjustment seen at map misses later layout changes.
    view.connect_vadjustment_notify(move |view| {
        if let Some(state) = owner.upgrade() {
            if view.is_mapped() {
                state.watch_adjustment(view);
            }
        }
    });
    let owner = Rc::downgrade(&state);
    view.buffer().connect_changed(move |_| {
        if let Some(state) = owner.upgrade() {
            state.cancel();
        }
    });
    if view.is_mapped() {
        state.mapped(view);
    }
}
