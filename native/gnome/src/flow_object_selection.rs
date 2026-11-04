//! Selection outlines for opaque anchored widgets. GTK owns text selection and
//! painting; a bounded pass updates only changed classes after affected frames.
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, OnceCell},
    ops::Range,
    rc::Rc,
};
use studio_markdown_native::flow;
const CLASS: &str = "studio-flow-object-selected";
struct State {
    view: glib::WeakRef<gtk::TextView>,
    buffer: glib::WeakRef<gtk::TextBuffer>,
    children: Vec<(glib::WeakRef<gtk::Widget>, Range<usize>)>,
    valid: Cell<bool>,
    update: OnceCell<Rc<crate::viewport::FrameUpdate>>,
}
impl State {
    fn clear(&self) {
        let view = self.view.upgrade();
        for (child, _) in &self.children {
            if let Some(child) = child.upgrade() {
                // A reused table may already belong to another page's view.
                if child.parent().is_none()
                    || child.parent().as_ref() == view.as_ref().map(|v| v.upcast_ref())
                {
                    child.remove_css_class(CLASS);
                }
            }
        }
    }
    fn refresh(&self) {
        let Some(view) = self.view.upgrade().filter(|v| {
            self.valid.get()
                && v.is_mapped()
                && self.buffer.upgrade().is_some_and(|b| v.buffer() == b)
        }) else {
            self.clear();
            return;
        };
        let selected = view
            .buffer()
            .selection_bounds()
            .map(|(a, b)| a.offset() as usize..b.offset() as usize);
        for (child, range) in &self.children {
            if let Some(child) = child
                .upgrade()
                .filter(|c| c.parent().as_ref() == Some(view.upcast_ref()))
            {
                let wanted = selected
                    .as_ref()
                    .is_some_and(|s| s.start < range.end && range.start < s.end);
                if child.has_css_class(CLASS) != wanted {
                    if wanted {
                        child.add_css_class(CLASS);
                    } else {
                        child.remove_css_class(CLASS);
                    }
                }
            }
        }
    }
    fn queue(&self) {
        if let Some(view) = self.view.upgrade() {
            self.update.get().unwrap().queue(&view);
        }
    }
    fn cancel(&self) {
        self.update.get().unwrap().clear();
        self.clear();
    }
}
pub fn attach(view: &gtk::TextView, document: &flow::Projection, children: &[gtk::Widget]) {
    if document.quick.is_some() || children.is_empty() {
        return;
    }
    for child in children {
        child.remove_css_class(CLASS);
    }
    let buffer = view.buffer();
    let state = Rc::new(State {
        view: view.downgrade(),
        buffer: buffer.downgrade(),
        children: children
            .iter()
            .zip(&document.objects)
            // Transparent inline formulas already show the text selection
            // beneath them. An inset border can obscure their edge glyphs.
            .filter(|(_, o)| !matches!(o.kind, flow::ObjectKind::Math { .. }))
            .map(|(c, o)| (c.downgrade(), o.characters.clone()))
            .collect(),
        valid: Cell::new(true),
        update: OnceCell::new(),
    });
    let weak = Rc::downgrade(&state);
    state
        .update
        .set(crate::viewport::FrameUpdate::new(Rc::new(move |_| {
            if let Some(state) = weak.upgrade() {
                state.refresh();
            }
        })))
        .ok()
        .unwrap();
    let owner = state.clone();
    buffer.connect_mark_set(move |_, _, mark| {
        if matches!(mark.name().as_deref(), Some("insert" | "selection_bound")) {
            owner.queue();
        }
    });
    let owner = state.clone();
    buffer.connect_changed(move |_| {
        owner.valid.set(false);
        owner.cancel();
    });
    let owner = state.clone();
    view.connect_buffer_notify(move |v| {
        if owner.buffer.upgrade().is_none_or(|b| b != v.buffer()) {
            owner.valid.set(false);
            owner.cancel();
        }
    });
    let owner = state.clone();
    view.connect_unmap(move |_| owner.cancel());
    let owner = state.clone();
    view.connect_map(move |_| owner.queue());
    state.queue();
}
