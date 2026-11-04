//! GTK consumes worker-prepared syntax ranges. Theme changes replay cached tags;
//! no source parsing, text replacement or syntax-engine calls happen here.
use adw::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};
use studio_markdown_native::syntax::{Page, Paint};

pub struct Presentation {
    view: glib::WeakRef<gtk::TextView>,
    buffer: glib::WeakRef<gtk::TextBuffer>,
    page: Arc<Page>,
    tags: RefCell<Vec<gtk::TextTag>>,
    dark: Cell<Option<bool>>,
    valid: Cell<bool>,
    handler: RefCell<Option<glib::SignalHandlerId>>,
}
fn rgba(color: [u8; 4]) -> gdk::RGBA {
    gdk::RGBA::new(
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        color[3] as f32 / 255.0,
    )
}
impl Presentation {
    pub fn attach(view: &gtk::TextView, page: Arc<Page>) -> Rc<Self> {
        let buffer = view.buffer();
        let state = Rc::new(Self {
            view: view.downgrade(),
            buffer: buffer.downgrade(),
            page,
            tags: RefCell::new(Vec::new()),
            dark: Cell::new(None),
            valid: Cell::new(true),
            handler: RefCell::new(None),
        });
        if let Some(anchored) = view.downcast_ref::<crate::anchored_text_view::AnchoredTextView>() {
            let weak = Rc::downgrade(&state);
            anchored.set_below_text(Rc::new(move |view, snapshot| {
                if let Some(state) = weak.upgrade() {
                    state.draw_background(view, snapshot);
                }
            }));
        }
        let weak = Rc::downgrade(&state);
        crate::code_gutter::attach(
            view,
            state.page.clone(),
            Rc::new(move || weak.upgrade().is_some_and(|s| s.is_current())),
        );
        let weak = Rc::downgrade(&state);
        buffer.connect_changed(move |_| {
            if let Some(state) = weak.upgrade() {
                state.valid.set(false);
                state.clear();
            }
        });
        let weak = Rc::downgrade(&state);
        *state.handler.borrow_mut() = Some(adw::StyleManager::default().connect_dark_notify(
            move |manager| {
                if let Some(state) = weak.upgrade() {
                    state.apply(manager.is_dark());
                }
            },
        ));
        // The widget owns its presentation even if a caller retains only the
        // widget. The reverse reference is weak; no signal/widget cycle forms.
        let owned = state.clone();
        view.connect_map(move |_| owned.apply(adw::StyleManager::default().is_dark()));
        state.apply(adw::StyleManager::default().is_dark());
        state
    }
    pub fn is_current(&self) -> bool {
        self.valid.get()
            && self
                .view
                .upgrade()
                .zip(self.buffer.upgrade())
                .is_some_and(|(view, buffer)| view.buffer() == buffer)
    }
    fn draw_background(&self, view: &gtk::TextView, snapshot: &gtk::Snapshot) {
        if !self.is_current() {
            return;
        }
        let Some(base) = self.dark.get().and_then(|dark| {
            if dark {
                self.page.base_dark
            } else {
                self.page.base_light
            }
        }) else {
            return;
        };
        let visible = view.visible_rect();
        let buffer = view.buffer();
        let first = view
            .iter_at_location(visible.x(), visible.y())
            .map_or(0, |i| i.offset());
        let last = view
            .iter_at_location(
                visible.x() + visible.width(),
                visible.y() + visible.height(),
            )
            .map_or(buffer.char_count(), |i| i.offset());
        for block in &self.page.blocks {
            if block.characters.is_empty()
                || block.characters.end <= first as usize
                || block.characters.start > last as usize
            {
                continue;
            }
            let start = view.iter_location(&buffer.iter_at_offset(block.characters.start as i32));
            let end = view.iter_location(&buffer.iter_at_offset(block.characters.end as i32));
            let tail = view.iter_location(&buffer.iter_at_offset(block.characters.end as i32 - 1));
            let top = start.y().max(visible.y());
            let bottom = end
                .y()
                .max(tail.y() + tail.height())
                .min(visible.y() + visible.height());
            let right = visible.x() + visible.width() - view.right_margin();
            if bottom > top && right > start.x() {
                snapshot.append_color(
                    &rgba(base.background),
                    &gtk::graphene::Rect::new(
                        start.x() as f32,
                        top as f32,
                        (right - start.x()) as f32,
                        (bottom - top) as f32,
                    ),
                );
            }
        }
    }
    fn clear(&self) {
        if let Some(buffer) = self.buffer.upgrade() {
            for tag in self.tags.borrow_mut().drain(..) {
                buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
                buffer.tag_table().remove(&tag);
            }
        }
        self.dark.set(None);
        if let Some(view) = self.view.upgrade() {
            view.queue_draw();
            if let Some(gutter) = crate::code_gutter::widget(&view) {
                gutter.queue_draw();
            }
        }
    }
    fn apply(&self, dark: bool) {
        if !self.is_current() || self.dark.get() == Some(dark) {
            return;
        }
        self.clear();
        let Some(buffer) = self.buffer.upgrade() else {
            return;
        };
        let mut palette = HashMap::<Paint, gtk::TextTag>::new();
        let mut apply = |paint: Paint, range: &std::ops::Range<usize>| {
            let next = palette.len();
            let tag = palette.entry(paint).or_insert_with(|| {
                let tag = gtk::TextTag::builder()
                    .name(format!("studio-flow-syntax-{next}"))
                    .foreground_rgba(&rgba(paint.foreground))
                    .background_rgba(&rgba(paint.background))
                    .weight(if paint.bold { 700 } else { 400 })
                    .style(if paint.italic {
                        gtk::pango::Style::Italic
                    } else {
                        gtk::pango::Style::Normal
                    })
                    .underline(if paint.underline {
                        gtk::pango::Underline::Single
                    } else {
                        gtk::pango::Underline::None
                    })
                    .build();
                buffer.tag_table().add(&tag);
                tag
            });
            buffer.apply_tag(
                tag,
                &buffer.iter_at_offset(range.start as i32),
                &buffer.iter_at_offset(range.end as i32),
            );
        };
        // Base tags precede token tags, keeping token paint at higher priority.
        for block in &self.page.blocks {
            let colors = if dark { &block.dark } else { &block.light };
            if let Some(base) = colors.as_ref().map(|c| c.base).or(if dark {
                self.page.base_dark
            } else {
                self.page.base_light
            }) {
                apply(base, &block.characters);
            }
        }
        for block in &self.page.blocks {
            if let Some(colors) = if dark { &block.dark } else { &block.light } {
                for run in &colors.runs {
                    apply(run.paint, &run.characters);
                }
            }
        }
        self.tags.borrow_mut().extend(palette.into_values());
        self.dark.set(Some(dark));
    }
}
impl Drop for Presentation {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.get_mut().take() {
            adw::StyleManager::default().disconnect(handler);
        }
    }
}
