//! Layout observations only: never infer a read from model presence or selection.
use crate::{
    chat_ui::{self, Part},
    model::Item,
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
struct Bound {
    widget: glib::WeakRef<gtk::Widget>,
    item: String,
    turn: String,
    last: bool,
    header: bool,
}
#[derive(Default)]
pub struct Visibility {
    rows: RefCell<HashMap<usize, Bound>>,
    pub on_change: RefCell<Option<Box<dyn Fn()>>>,
}
impl Visibility {
    #[cfg(debug_assertions)]
    pub fn smoke_visible(&self, item: &str, viewport: &gtk::Widget) -> bool {
        self.rows.borrow().values().any(|row| {
            row.item == item
                && row.widget.upgrade().is_some_and(|widget| {
                    widget.is_mapped()
                        && widget.compute_bounds(viewport).is_some_and(|bounds| {
                            bounds.width() > 0.0
                                && bounds.height() > 0.0
                                && bounds.y() + bounds.height() > 12.0
                                && bounds.y() < viewport.height() as f32 - 12.0
                        })
                })
        })
    }
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
    pub fn bind(&self, widget: &gtk::Widget, item: &Item, part: Part) {
        self.rows.borrow_mut().insert(
            widget.as_ptr() as usize,
            Bound {
                widget: widget.downgrade(),
                item: item.id.clone(),
                turn: item.turn_id.clone(),
                last: chat_ui::parts(item).last() == Some(part),
                header: part == Part::Header,
            },
        );
        if let Some(changed) = self.on_change.borrow().as_ref() {
            changed();
        }
    }
    pub fn unbind(&self, widget: &gtk::Widget) {
        self.rows.borrow_mut().remove(&(widget.as_ptr() as usize));
        if let Some(changed) = self.on_change.borrow().as_ref() {
            changed();
        }
    }
    pub fn turn_at_marker(
        &self,
        viewport: &gtk::Widget,
        turns: &HashMap<String, u32>,
    ) -> Option<u32> {
        let marker = (viewport.height() as f32 * 0.28).min(160.0);
        let mut before: Option<(f32, u32)> = None;
        let mut after: Option<(f32, u32)> = None;
        let mut rows = self.rows.borrow_mut();
        rows.retain(|_, row| row.widget.upgrade().is_some());
        for row in rows.values() {
            let Some(index) = turns.get(&row.turn) else {
                continue;
            };
            let Some(widget) = row.widget.upgrade() else {
                continue;
            };
            if !widget.is_mapped() || !widget.is_visible() {
                continue;
            }
            let Some(bounds) = widget.compute_bounds(viewport) else {
                continue;
            };
            let top = bounds.y();
            if bounds.width() <= 0.0
                || bounds.height() <= 0.0
                || top + bounds.height() <= 0.0
                || top >= viewport.height() as f32
            {
                continue;
            }
            if top <= marker {
                if before.is_none_or(|(y, _)| top > y) {
                    before = Some((top, *index));
                }
            } else if after.is_none_or(|(y, _)| top < y) {
                after = Some((top, *index));
            }
        }
        before.or(after).map(|(_, index)| index)
    }
    pub fn header(&self, item: &str, turn: &str) -> Option<gtk::Widget> {
        self.rows.borrow().values().find_map(|row| {
            (row.header && row.item == item && row.turn == turn)
                .then(|| row.widget.upgrade())
                .flatten()
        })
    }
    pub fn visible_ends(&self, viewport: &gtk::Widget) -> HashSet<(String, String)> {
        let mut rows = self.rows.borrow_mut();
        rows.retain(|_, row| row.widget.upgrade().is_some());
        rows.values()
            .filter(|row| row.last)
            .filter_map(|row| {
                let widget = row.widget.upgrade()?;
                if !widget.is_mapped() || !widget.is_visible() {
                    return None;
                }
                let bounds = widget.compute_bounds(viewport)?;
                (bounds.width() > 0.0
                    && bounds.height() > 0.0
                    && bounds.y() + bounds.height() > 0.0
                    && bounds.y() + bounds.height() <= viewport.height() as f32 + 1.0)
                    .then(|| (row.item.clone(), row.turn.clone()))
            })
            .collect()
    }
}
