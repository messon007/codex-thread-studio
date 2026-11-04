//! Decorative GTK TextView gutter; source offsets are prepared on Rust workers.
use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use studio_markdown_native::syntax::Page;
struct Model {
    view: glib::WeakRef<gtk::TextView>,
    page: Arc<Page>,
    current: Rc<dyn Fn() -> bool>,
}
fn layout(widget: &gtk::Widget, text: &str) -> gtk::pango::Layout {
    let layout = widget.create_pango_layout(Some(text));
    let mut font = widget
        .pango_context()
        .font_description()
        .unwrap_or_default();
    font.set_family("monospace");
    layout.set_font_description(Some(&font));
    layout
}
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Gutter {
        pub(super) model: RefCell<Option<Model>>,
        #[cfg(debug_assertions)]
        pub rows: RefCell<Vec<(usize, usize, i32)>>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Gutter {
        const NAME: &'static str = "StudioCodeGutter";
        type Type = super::Gutter;
        type ParentType = gtk::Widget;
        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }
    impl ObjectImpl for Gutter {}
    impl WidgetImpl for Gutter {
        fn measure(&self, orientation: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            let width = if orientation == gtk::Orientation::Horizontal {
                let max = self.model.borrow().as_ref().map_or(0, |m| {
                    m.page
                        .blocks
                        .iter()
                        .map(|b| b.lines.len())
                        .max()
                        .unwrap_or(0)
                });
                if max == 0 {
                    0
                } else {
                    layout(self.obj().upcast_ref(), &max.to_string())
                        .pixel_size()
                        .0
                        + 16
                }
            } else {
                0
            };
            (width, width, -1, -1)
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            #[cfg(debug_assertions)]
            self.rows.borrow_mut().clear();
            let model = self.model.borrow();
            let Some(model) = model.as_ref().filter(|m| (m.current)()) else {
                return;
            };
            let Some(view) = model.view.upgrade() else {
                return;
            };
            let buffer = view.buffer();
            let visible = view.visible_rect();
            let first = view
                .iter_at_location(visible.x(), visible.y())
                .map_or(0, |i| i.offset()) as usize;
            let last = view
                .iter_at_location(
                    visible.x() + visible.width(),
                    visible.y() + visible.height(),
                )
                .map_or(buffer.char_count(), |i| i.offset()) as usize;
            let owner = self.obj();
            let label = layout(owner.upcast_ref(), "");
            let mut ink = owner.color();
            ink.set_alpha(ink.alpha() * 0.6);
            for (index, block) in model.page.blocks.iter().enumerate() {
                if block.lines.is_empty()
                    || block.characters.end <= first
                    || block.characters.start > last
                {
                    continue;
                }
                let start =
                    view.iter_location(&buffer.iter_at_offset(block.characters.start as i32));
                let end =
                    view.iter_location(&buffer.iter_at_offset(block.characters.end as i32 - 1));
                let (_, top) =
                    view.buffer_to_window_coords(gtk::TextWindowType::Left, 0, start.y());
                let (_, bottom) = view.buffer_to_window_coords(
                    gtk::TextWindowType::Left,
                    0,
                    end.y() + end.height(),
                );
                let top = top.max(0);
                let bottom = bottom.min(owner.height());
                if let Some(paint) = if adw::StyleManager::default().is_dark() {
                    block.dark.as_ref().map(|c| c.base).or(model.page.base_dark)
                } else {
                    block
                        .light
                        .as_ref()
                        .map(|c| c.base)
                        .or(model.page.base_light)
                } {
                    if bottom > top {
                        let c = paint.background;
                        snapshot.append_color(
                            &gdk::RGBA::new(
                                c[0] as f32 / 255.,
                                c[1] as f32 / 255.,
                                c[2] as f32 / 255.,
                                c[3] as f32 / 255.,
                            ),
                            &gtk::graphene::Rect::new(
                                0.,
                                top as f32,
                                owner.width() as f32,
                                (bottom - top) as f32,
                            ),
                        );
                    }
                }
                let skip = block
                    .lines
                    .partition_point(|offset| *offset < first)
                    .saturating_sub(1);
                for (number, offset) in block.lines.iter().enumerate().skip(skip) {
                    if *offset > last {
                        break;
                    }
                    let r = view.iter_location(&buffer.iter_at_offset(*offset as i32));
                    let (_, y) = view.buffer_to_window_coords(gtk::TextWindowType::Left, 0, r.y());
                    if y < 0 || y >= owner.height() {
                        continue;
                    }
                    label.set_text(&(number + 1).to_string());
                    let (w, h) = label.pixel_size();
                    let y = y + (r.height() - h).max(0) / 2;
                    snapshot.save();
                    snapshot.translate(&gtk::graphene::Point::new(
                        (owner.width() - w - 8) as f32,
                        y as f32,
                    ));
                    snapshot.append_layout(&label, &ink);
                    snapshot.restore();
                    #[cfg(debug_assertions)]
                    self.rows.borrow_mut().push((index, number + 1, y));
                }
            }
        }
    }
}
glib::wrapper! {
    pub struct Gutter(ObjectSubclass<imp::Gutter>) @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
pub fn attach(view: &gtk::TextView, page: Arc<Page>, current: Rc<dyn Fn() -> bool>) {
    if page.blocks.iter().all(|b| b.lines.is_empty()) {
        return;
    }
    let gutter: Gutter = glib::Object::new();
    gutter.set_can_target(false);
    gutter.set_focusable(false);
    *gutter.imp().model.borrow_mut() = Some(Model {
        view: view.downgrade(),
        page,
        current,
    });
    view.set_gutter(gtk::TextWindowType::Left, Some(&gutter));
    let weak = gutter.downgrade();
    view.connect_vadjustment_notify(move |view| {
        let Some(gutter) = weak.upgrade() else { return };
        gutter.queue_draw();
        if let Some(adjustment) = view.vadjustment() {
            let weak = gutter.downgrade();
            adjustment.connect_value_changed(move |_| {
                if let Some(g) = weak.upgrade() {
                    g.queue_draw();
                }
            });
        }
    });
    if let Some(adjustment) = view.vadjustment() {
        let weak = gutter.downgrade();
        adjustment.connect_value_changed(move |_| {
            if let Some(g) = weak.upgrade() {
                g.queue_draw();
            }
        });
    }
    let weak = gutter.downgrade();
    view.buffer()
        .tag_table()
        .connect_tag_changed(move |_, _, size| {
            if let Some(g) = weak.upgrade() {
                if size {
                    g.queue_resize();
                }
                g.queue_draw();
            }
        });
    let weak = gutter.downgrade();
    view.connect_buffer_notify(move |_| {
        if let Some(g) = weak.upgrade() {
            g.queue_draw();
        }
    });
}
#[cfg(debug_assertions)]
pub fn painted_rows(view: &gtk::TextView) -> Option<Vec<(usize, usize, i32)>> {
    widget(view).map(|g| g.imp().rows.borrow().clone())
}
pub fn widget(view: &gtk::TextView) -> Option<Gutter> {
    let gutter = view.gutter(gtk::TextWindowType::Left)?;
    // GTK 4.22 returns its border container, whose child is the supplied widget.
    // Accept both public getter representations without naming private GTK types.
    gutter
        .clone()
        .downcast::<Gutter>()
        .ok()
        .or_else(|| gutter.first_child()?.downcast().ok())
}
