//! In-memory reading position for a recreated, read-only native TextView.
//! GTK owns layout: retain a Unicode character and its clipped-line inset,
//! rather than a scroll percentage that changes when text wraps differently.
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy)]
pub struct Position {
    character: i32,
    inset: i32,
    insertion: i32,
    selection: i32,
    primary: Option<bool>,
}

impl Position {
    pub(crate) fn primary_owner(&self) -> Option<bool> {
        self.primary
    }
}
#[derive(Default)]
pub struct Memory {
    view: glib::WeakRef<gtk::TextView>,
    position: Cell<Option<Position>>,
    serial: Cell<u64>,
    busy: Cell<bool>,
    pending: RefCell<Option<gtk::TickCallbackId>>,
    mapping: RefCell<Option<std::sync::Arc<studio_markdown_native::flow::QuickText>>>,
    // A temporary ordinary buffer cannot restore a selected range without
    // publishing GTK's primary offer. Keep the desired source range until the
    // projected buffer returns, or until real user input replaces it.
    deferred_selection: Cell<Option<(i32, i32)>>,
}
impl Memory {
    fn source_offset(&self, offset: i32) -> i32 {
        self.mapping
            .borrow()
            .as_ref()
            .and_then(|m| m.source_range(offset.max(0) as usize..offset.max(0) as usize))
            .map_or(offset, |r| r.start as i32)
    }
    pub fn owns(&self, view: &gtk::TextView) -> bool {
        self.view.upgrade().as_ref() == Some(view)
    }
    pub fn reset(&self) {
        self.cancel();
        self.deferred_selection.set(None);
        self.position.set(None);
    }
    fn cancel(&self) {
        self.serial.set(self.serial.get().wrapping_add(1));
        self.busy.set(false);
        if let Some(pending) = self.pending.borrow_mut().take() {
            pending.remove();
        }
    }
    fn remember(&self, view: &gtk::TextView) {
        if self.busy.get() || !self.owns(view) || !view.is_mapped() || view.width() <= 0 {
            return;
        }
        let visible = view.visible_rect();
        if visible.height() <= 0 {
            return;
        }
        let mut iter = view
            .iter_at_location(view.left_margin(), visible.y() + 1)
            .unwrap_or_else(|| view.buffer().start_iter());
        if !view.starts_display_line(&iter) {
            view.backward_display_line_start(&mut iter);
        }
        let glyph = view.iter_location(&iter);
        if glyph.height() <= 0 {
            return;
        }
        let deferred = self.deferred_selection.get();
        self.position.set(Some(Position {
            primary: if deferred.is_some() {
                Some(false)
            } else {
                crate::text_flow::owns_primary(view)
            },
            character: self.source_offset(iter.offset()),
            // GTK buffer coordinates can be negative at the top margin, and
            // tag-defined spacing also precedes the first glyph. Preserve that
            // signed gap instead of gradually scrolling the margin away.
            inset: visible
                .y()
                .saturating_sub(glyph.y())
                .min(glyph.height() - 1),
            insertion: deferred.map_or_else(
                || self.source_offset(view.buffer().cursor_position()),
                |p| p.0,
            ),
            selection: deferred.map_or_else(
                || {
                    self.source_offset(
                        view.buffer()
                            .iter_at_mark(&view.buffer().selection_bound())
                            .offset(),
                    )
                },
                |p| p.1,
            ),
        }));
    }
    pub fn attach(self: &Rc<Self>, view: &gtk::TextView, scroller: &gtk::ScrolledWindow) {
        self.attach_mapped(view, scroller, None);
    }
    pub fn attach_mapped(
        self: &Rc<Self>,
        view: &gtk::TextView,
        scroller: &gtk::ScrolledWindow,
        mapping: Option<std::sync::Arc<studio_markdown_native::flow::QuickText>>,
    ) {
        if let Some(old) = self.view.upgrade() {
            self.remember(&old);
        }
        self.cancel();
        if mapping.is_some() {
            self.deferred_selection.set(None);
        }
        self.mapping.replace(mapping);
        self.view.set(Some(view));
        let memory = Rc::downgrade(self);
        let weak = view.downgrade();
        scroller.vadjustment().connect_value_changed(move |_| {
            if let (Some(memory), Some(view)) = (memory.upgrade(), weak.upgrade()) {
                memory.remember(&view);
            }
        });
        let memory = Rc::downgrade(self);
        let weak = view.downgrade();
        view.buffer().connect_mark_set(move |buffer, _, mark| {
            if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                if let (Some(memory), Some(view)) = (memory.upgrade(), weak.upgrade()) {
                    memory.remember(&view);
                }
            }
        });
        // User navigation wins over a deferred restore; do not fight a wheel,
        // key or pointer action while GTK is completing the first allocation.
        let input = gtk::EventControllerLegacy::new();
        input.set_propagation_phase(gtk::PropagationPhase::Capture);
        let memory = Rc::downgrade(self);
        let weak = view.downgrade();
        input.connect_event(move |_, event| {
            if matches!(
                event.event_type(),
                gtk::gdk::EventType::ButtonPress
                    | gtk::gdk::EventType::TouchBegin
                    | gtk::gdk::EventType::Scroll
                    | gtk::gdk::EventType::KeyPress
            ) {
                if let (Some(memory), Some(view)) = (memory.upgrade(), weak.upgrade()) {
                    memory.interrupt(&view);
                }
            }
            glib::Propagation::Proceed
        });
        scroller.add_controller(input);
    }

    fn interrupt(&self, view: &gtk::TextView) {
        if self.owns(view) {
            self.deferred_selection.set(None);
            self.cancel();
            self.remember(view);
        }
    }
    pub fn freeze(&self) -> Option<Position> {
        if let Some(owner) = self
            .view
            .upgrade()
            .and_then(|v| crate::text_flow::owns_primary(&v))
        {
            self.position.set(self.position.get().map(|mut p| {
                p.primary = Some(owner);
                p
            }));
        }
        self.cancel();
        self.busy.set(true);
        self.position.get()
    }
    pub fn settle(self: &Rc<Self>, view: &gtk::TextView, position: Option<Position>) {
        let memory = self.clone();
        let serial = self.serial.get();
        let frames = Cell::new(0);
        let mark = position.map(|p| {
            let buffer = view.buffer();
            let mapped = |offset: i32| {
                self.mapping
                    .borrow()
                    .as_ref()
                    .and_then(|m| m.projected_range(offset.max(0) as usize..offset.max(0) as usize))
                    .map_or(offset, |r| r.start as i32)
            };
            let iter = buffer.iter_at_offset(mapped(p.character).clamp(0, buffer.char_count()));
            let range = self.mapping.borrow().as_ref().and_then(|m| {
                m.projected_range(
                    p.insertion.min(p.selection).max(0) as usize
                        ..p.insertion.max(p.selection).max(0) as usize,
                )
            });
            let (insertion, selection) = range.map_or((p.insertion, p.selection), |r| {
                if p.insertion == p.selection {
                    (r.end as i32, r.end as i32)
                } else if p.insertion >= p.selection {
                    (r.end as i32, r.start as i32)
                } else {
                    (r.start as i32, r.end as i32)
                }
            });
            let a = buffer.iter_at_offset(insertion.clamp(0, buffer.char_count()));
            let b = buffer.iter_at_offset(selection.clamp(0, buffer.char_count()));
            if p.primary == Some(false) {
                if crate::text_flow::owns_primary(view).is_none() && p.insertion != p.selection {
                    self.deferred_selection
                        .set(Some((p.insertion, p.selection)));
                }
                crate::text_flow::restore_without_primary(view, &a, &b);
            } else {
                buffer.select_range(&a, &b);
            }
            let mark = buffer.mark("studio-reading-position").unwrap_or_else(|| {
                buffer.create_mark(Some("studio-reading-position"), &iter, true)
            });
            buffer.move_mark(&mark, &iter);
            mark
        });
        let pending = view.add_tick_callback(move |view, _| {
            if !memory.owns(view) || memory.serial.get() != serial {
                return glib::ControlFlow::Break;
            }
            frames.set(frames.get() + 1);
            if view.width() > 0 && view.height() > 0 {
                if let Some(position) = position {
                    let iter = view.buffer().iter_at_mark(mark.as_ref().unwrap());
                    if frames.get() == 1 {
                        // GTK defers this until text line heights are validated.
                        view.scroll_to_mark(mark.as_ref().unwrap(), 0.0, true, 0.0, 0.0);
                    } else {
                        let glyph = view.iter_location(&iter);
                        let visible = view.visible_rect();
                        let delta = glyph.y() + position.inset.min((glyph.height() - 1).max(0))
                            - visible.y();
                        if glyph.height() > 0 && delta.abs() > 1 && frames.get() < 8 {
                            if let Some(adjustment) = view.vadjustment() {
                                adjustment.set_value(adjustment.value() + f64::from(delta));
                            }
                            return glib::ControlFlow::Continue;
                        }
                    }
                    if frames.get() == 1 {
                        return glib::ControlFlow::Continue;
                    }
                }
                memory.busy.set(false);
                memory.pending.borrow_mut().take();
                memory.remember(view);
                return glib::ControlFlow::Break;
            }
            if frames.get() >= 8 {
                memory.busy.set(false);
                memory.pending.borrow_mut().take();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        self.pending.replace(Some(pending));
    }
}
