//! Native full-cell Find. Matching stays on bounded workers, layout stays in GTK.
use super::*;
use std::{
    cell::Cell,
    ops::Range,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
#[path = "table_cell_search.rs"]
mod search;

type Navigate = Rc<dyn Fn(Range<usize>)>;
pub struct Find {
    pub bar: adw::WrapBox,
    toggle: gtk::ToggleButton,
    entry: gtk::SearchEntry,
    case: gtk::ToggleButton,
    previous: gtk::Button,
    next: gtk::Button,
    count: gtk::Label,
    source: Arc<studio_markdown_native::RichText>,
    formatted: glib::WeakRef<gtk::Widget>,
    navigate: Navigate,
    matches: RefCell<search::Matches>,
    index: Cell<Option<usize>>,
    serial: Cell<u64>,
    cancelled: RefCell<Option<Arc<AtomicBool>>>,
    timer: RefCell<Option<glib::SourceId>>,
}
impl Find {
    pub fn install(
        source: Arc<studio_markdown_native::RichText>,
        reading: Rc<ParagraphState>,
        formatted: &gtk::Widget,
        window: &adw::Window,
        root: &adw::ToolbarView,
        header: &adw::HeaderBar,
    ) {
        let label = formatted
            .clone()
            .downcast::<gtk::Label>()
            .ok()
            .map(|l| l.downgrade());
        let attributes = source.attributes.clone();
        let weak = Rc::downgrade(&reading);
        crate::rich_paragraph::when_ready(
            formatted,
            Rc::new(move |_| {
                if let Some(reading) = weak.upgrade().filter(|r| r.target.borrow().is_some()) {
                    if let Some(view) = reading.view.upgrade() {
                        reveal_view(&view);
                    }
                }
            }),
        );
        let navigate: Navigate = Rc::new(move |range| {
            if let Some(label) = label
                .as_ref()
                .and_then(glib::WeakRef::upgrade)
                .filter(|l| l.is_mapped())
            {
                reveal(&label, range);
            } else if let Some(attributes) = &attributes {
                let page = attributes
                    .pages
                    .partition_point(|p| p.character <= range.start)
                    .saturating_sub(1);
                reading.focus(page, range);
                if let Some(view) = reading.view.upgrade() {
                    reveal_view(&view);
                }
            }
        });
        let toggle = gtk::ToggleButton::builder()
            .icon_name("edit-find-symbolic")
            .build();
        crate::i18n_ui::bind(&toggle, "tooltip-text", "Find in cell (Ctrl+F)");
        crate::i18n_ui::accessible_label(&toggle, "Find in cell");
        toggle.set_widget_name("studio-cell-find-toggle");
        header.pack_end(&toggle);
        let bar = crate::adaptive::toolbar(4);
        bar.set_visible(false);
        bar.set_margin_start(12);
        bar.set_margin_end(12);
        let entry = gtk::SearchEntry::builder()
            .search_delay(0)
            .hexpand(true)
            .width_chars(1)
            .build();
        entry.set_widget_name("studio-cell-find-entry");
        crate::i18n_ui::bind(&entry, "placeholder-text", "Find in cell");
        crate::i18n_ui::accessible_label(&entry, "Find in cell");
        let case = gtk::ToggleButton::with_label("Aa");
        case.set_widget_name("studio-cell-find-case");
        crate::i18n_ui::bind(&case, "tooltip-text", "Match case");
        crate::i18n_ui::accessible_label(&case, "Match case");
        let previous = gtk::Button::from_icon_name("go-up-symbolic");
        let next = gtk::Button::from_icon_name("go-down-symbolic");
        previous.set_widget_name("studio-cell-find-previous");
        next.set_widget_name("studio-cell-find-next");
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        for (button, key) in [
            (&previous, "Previous match"),
            (&next, "Next match"),
            (&close, "Close search"),
        ] {
            crate::i18n_ui::bind(button, "tooltip-text", key);
            crate::i18n_ui::accessible_label(button, key);
        }
        previous.set_sensitive(false);
        next.set_sensitive(false);
        let count = gtk::Label::new(None);
        count.add_css_class("caption");
        count.set_widget_name("studio-cell-find-count");
        for widget in [
            entry.upcast_ref::<gtk::Widget>(),
            case.upcast_ref(),
            previous.upcast_ref(),
            next.upcast_ref(),
            count.upcast_ref(),
            close.upcast_ref(),
        ] {
            bar.append(widget);
        }
        root.add_top_bar(&bar);
        let find = Rc::new(Self {
            bar,
            toggle,
            entry,
            case,
            previous,
            next,
            count,
            source,
            formatted: formatted.downgrade(),
            navigate,
            matches: RefCell::new(search::Matches::default()),
            index: Cell::new(None),
            serial: Cell::new(0),
            cancelled: RefCell::new(None),
            timer: RefCell::new(None),
        });
        let weak = Rc::downgrade(&find);
        find.toggle.connect_toggled(move |toggle| {
            if let Some(find) = weak.upgrade() {
                find.bar.set_visible(toggle.is_active());
                if toggle.is_active() {
                    find.entry.grab_focus();
                    find.schedule();
                } else {
                    find.cancel();
                    if let Some(formatted) = find.formatted.upgrade() {
                        focus_selection(&formatted);
                    }
                }
            }
        });
        let weak = Rc::downgrade(&find);
        find.entry.connect_changed(move |_| {
            if let Some(find) = weak.upgrade() {
                find.schedule();
            }
        });
        let weak = Rc::downgrade(&find);
        find.case.connect_toggled(move |_| {
            if let Some(find) = weak.upgrade() {
                find.schedule();
            }
        });
        for (button, forward) in [(&find.previous, false), (&find.next, true)] {
            let weak = Rc::downgrade(&find);
            button.connect_clicked(move |_| {
                if let Some(find) = weak.upgrade() {
                    find.step(forward);
                }
            });
        }
        let weak = Rc::downgrade(&find);
        find.entry.connect_activate(move |_| {
            if let Some(find) = weak.upgrade() {
                let empty = find.matches.borrow().ranges.is_empty();
                if empty {
                    find.schedule();
                } else {
                    find.step(true);
                }
            }
        });
        let weak = Rc::downgrade(&find);
        close.connect_clicked(move |_| {
            if let Some(find) = weak.upgrade() {
                find.toggle.set_active(false);
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&find);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(find) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F)
            {
                find.toggle.set_active(true);
                find.entry.grab_focus();
                return glib::Propagation::Stop;
            }
            if find.toggle.is_active() && key == gtk::gdk::Key::Escape {
                find.toggle.set_active(false);
                return glib::Propagation::Stop;
            }
            if find.toggle.is_active() && key == gtk::gdk::Key::F3 {
                find.step(!modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK));
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(keys);
        let weak = Rc::downgrade(&find);
        window.connect_unmap(move |_| {
            if let Some(find) = weak.upgrade() {
                find.cancel();
            }
        });
        // The toplevel owns the controller; callbacks only hold weak references.
        window.connect_destroy(move |_| {
            find.cancel();
        });
    }
    fn cancel(&self) {
        self.serial.set(self.serial.get().wrapping_add(1));
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        if let Some(cancelled) = self.cancelled.borrow_mut().take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        self.matches.replace(search::Matches::default());
        self.index.set(None);
        self.previous.set_sensitive(false);
        self.next.set_sensitive(false);
    }
    fn schedule(self: &Rc<Self>) {
        self.cancel();
        self.count.set_tooltip_text(None);
        if !self.toggle.is_active() || !self.bar.is_mapped() {
            return;
        }
        let query = self.entry.text().to_string();
        if query.is_empty() {
            crate::i18n_ui::literal(&self.count, "label", "");
            return;
        }
        crate::i18n_ui::bind(&self.count, "label", "Searching…");
        let weak = Rc::downgrade(self);
        self.timer.replace(Some(glib::timeout_add_local_once(
            Duration::from_millis(120),
            move || {
                if let Some(find) = weak.upgrade() {
                    find.timer.borrow_mut().take();
                    find.start(query);
                }
            },
        )));
    }
    fn start(self: &Rc<Self>, query: String) {
        let serial = self.serial.get();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancelled.replace(Some(cancelled.clone()));
        let source = self.source.clone();
        let case = self.case.is_active();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            static LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
            let result = match LIMIT
                .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
                .clone()
                .try_acquire_owned()
            {
                Ok(permit) => gio::spawn_blocking(move || {
                    let _permit = permit;
                    search::find(&source.plain, &query, case, &cancelled)
                })
                .await
                .map_err(|_| "Cell search worker stopped".to_string())
                .and_then(|r| r),
                Err(_) => Err("Search workers are busy; try again".into()),
            };
            let Some(find) = weak
                .upgrade()
                .filter(|f| f.serial.get() == serial && f.bar.is_mapped() && f.toggle.is_active())
            else {
                return;
            };
            find.cancelled.borrow_mut().take();
            match result {
                Ok(matches) => {
                    let any = !matches.ranges.is_empty();
                    find.matches.replace(matches);
                    find.previous.set_sensitive(any);
                    find.next.set_sensitive(any);
                    if any {
                        find.step(true);
                    } else {
                        crate::i18n_ui::bind(&find.count, "label", "No matches found");
                    }
                }
                Err(error) => {
                    crate::i18n_ui::bind(
                        &find.count,
                        "label",
                        if error == "Search workers are busy; try again" {
                            "Search workers are busy; try again"
                        } else {
                            "Cell search failed"
                        },
                    );
                    find.count.set_tooltip_text(Some(&error));
                }
            }
        });
    }
    fn step(&self, forward: bool) {
        if self.timer.borrow().is_some()
            || self.cancelled.borrow().is_some()
            || self.entry.text().is_empty()
        {
            return;
        }
        let matches = self.matches.borrow();
        let count = matches.ranges.len();
        if count == 0 {
            return;
        }
        if !self.bar.is_mapped() {
            return;
        }
        let index = self
            .index
            .get()
            .map_or(if forward { 0 } else { count - 1 }, |i| {
                if forward {
                    (i + 1) % count
                } else {
                    (i + count - 1) % count
                }
            });
        self.index.set(Some(index));
        crate::i18n_ui::bind_format(
            &self.count,
            "label",
            if matches.limited {
                "{index} of {count} matches · Limit reached"
            } else {
                "{index} of {count} matches"
            },
            &[
                ("index", &(index + 1).to_string()),
                ("count", &count.to_string()),
            ],
        );
        (self.navigate)(matches.ranges[index].clone());
    }
}
fn focus_selection(widget: &gtk::Widget) -> bool {
    if !widget.is_mapped() {
        return false;
    }
    if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        if view.buffer().selection_bounds().is_some() {
            return view.grab_focus();
        }
    }
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        if let Some((a, b)) = label.selection_bounds() {
            let focused = label.grab_focus();
            label.select_region(a, b);
            return focused;
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if focus_selection(&widget) {
            return true;
        }
    }
    false
}
/// Select original scalar positions, then reveal after GTK has allocated text.
pub(super) fn reveal(label: &gtk::Label, range: Range<usize>) {
    label.select_region(range.start as i32, range.end as i32);
    let character = range.start;
    let frames = Cell::new(0);
    label.add_tick_callback(move |label, _| {
        if !label.is_mapped() {
            return glib::ControlFlow::Break;
        }
        let text = label.text();
        let byte = text
            .char_indices()
            .nth(character)
            .map_or(text.len(), |(b, _)| b);
        let rect = label.layout().index_to_pos(byte as i32);
        let (_, offset) = label.layout_offsets();
        reveal_point(
            label.upcast_ref(),
            offset as f32 + rect.y() as f32 / gtk::pango::SCALE as f32,
            rect.height() as f32 / gtk::pango::SCALE as f32,
        );
        frames.set(frames.get() + 1);
        if frames.get() < 2 {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
}
fn reveal_view(view: &gtk::TextView) {
    let frames = Cell::new(0);
    view.add_tick_callback(move |view, _| {
        if !view.is_mapped() {
            return glib::ControlFlow::Break;
        }
        if let Some((start, _)) = view.buffer().selection_bounds() {
            let glyph = view.iter_location(&start);
            let (_, y) =
                view.buffer_to_window_coords(gtk::TextWindowType::Widget, glyph.x(), glyph.y());
            reveal_point(view.upcast_ref(), y as f32, glyph.height() as f32);
        }
        frames.set(frames.get() + 1);
        if frames.get() < 2 {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
}
fn reveal_point(widget: &gtk::Widget, y: f32, height: f32) {
    let mut parent = widget.parent();
    while let Some(ancestor) = parent {
        parent = ancestor.parent();
        if let Some(scroll) = ancestor.downcast_ref::<gtk::ScrolledWindow>() {
            if let Some(point) = widget.compute_point(scroll, &gtk::graphene::Point::new(0.0, y)) {
                if point.y() < 0.0 || point.y() + height > scroll.height() as f32 {
                    let adjustment = scroll.vadjustment();
                    adjustment.set_value(
                        adjustment.value() + point.y() as f64 - scroll.height() as f64 * 0.35,
                    );
                }
            }
        }
    }
}

#[cfg(debug_assertions)]
#[path = "table_cell_find_acceptance.rs"]
mod acceptance;
#[cfg(debug_assertions)]
pub(super) async fn smoke(window: &gtk::Window, query: &str, expected: &str) -> Result<(), String> {
    acceptance::smoke(window, query, expected).await
}
