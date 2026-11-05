//! Fill-width child anchors without making their previous width the text view's
//! minimum. GTK owns allocation/reflow; no frame polling or text replacement.
use gtk::{glib, prelude::*, subclass::prelude::*};
use std::rc::Rc;
type TextLayer = Rc<dyn Fn(&gtk::TextView, &gtk::Snapshot)>;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};
    #[derive(Default)]
    pub struct AnchoredTextView {
        pub children: RefCell<Vec<(glib::WeakRef<gtk::Widget>, i32)>>,
        pub width: Cell<i32>,
        pub below_text: RefCell<Option<TextLayer>>,
        caret_object: RefCell<Option<(glib::WeakRef<gtk::Widget>, glib::SignalHandlerId)>>,
        #[cfg(debug_assertions)]
        pub updates: Cell<u64>,
        #[cfg(debug_assertions)]
        pub timings: [Cell<u128>; 6],
    }
    #[glib::object_subclass]
    impl ObjectSubclass for AnchoredTextView {
        const NAME: &'static str = "StudioAnchoredTextView";
        type Type = super::AnchoredTextView;
        type ParentType = gtk::TextView;
        type Interfaces = (gtk::AccessibleText,);
    }
    impl ObjectImpl for AnchoredTextView {}
    impl AccessibleTextImpl for AnchoredTextView {
        fn contents(&self, start: u32, end: u32) -> Option<glib::Bytes> {
            // Keep the toolkit's character coordinate space, including child
            // anchors. Omitting those slots makes accessible counts disagree
            // with caret/selection offsets and truncates ranged text reads.
            let buffer = self.obj().buffer();
            let count = buffer.char_count() as u32;
            let end = end.min(count);
            if start > end {
                return None;
            }
            let text = buffer.slice(
                &buffer.iter_at_offset(start as i32),
                &buffer.iter_at_offset(end as i32),
                true,
            );
            Some(glib::Bytes::from_owned(text.as_bytes().to_vec()))
        }
        fn contents_at(
            &self,
            offset: u32,
            granularity: gtk::AccessibleTextGranularity,
        ) -> Option<(u32, u32, glib::Bytes)> {
            if offset > self.obj().buffer().char_count() as u32 {
                return None;
            }
            // GTK owns language, word/sentence and visual line boundaries.
            let (start, end, _) = self.parent_contents_at(offset, granularity)?;
            Some((start, end, self.contents(start, end)?))
        }
    }
    impl TextViewImpl for AnchoredTextView {
        fn snapshot_layer(&self, layer: gtk::TextViewLayer, snapshot: gtk::Snapshot) {
            #[cfg(debug_assertions)]
            let started = std::time::Instant::now();
            self.parent_snapshot_layer(layer, snapshot.clone());
            if layer == gtk::TextViewLayer::BelowText {
                if let Some(draw) = self.below_text.borrow().as_ref() {
                    draw(self.obj().upcast_ref(), &snapshot);
                }
            }
            #[cfg(debug_assertions)]
            self.record(4, started.elapsed().as_micros());
        }
    }
    impl WidgetImpl for AnchoredTextView {
        fn map(&self) {
            self.parent_map();
            self.update_caret_object();
        }
        fn unmap(&self) {
            self.set_caret_object(None);
            self.parent_unmap();
        }
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            #[cfg(debug_assertions)]
            let started = std::time::Instant::now();
            // GtkTextView's measurement also invalidates anchor layout when
            // a child's requisition changes. Keep those side effects even
            // when replacing its horizontal minimum to permit narrow panes.
            let measured = self.parent_measure(orientation, for_size);
            let result = if orientation == gtk::Orientation::Horizontal
                && self.obj().wrap_mode() != gtk::WrapMode::None
            {
                let view = self.obj();
                let minimum = (view.left_margin() + view.right_margin() + 1).max(1);
                (minimum, minimum, -1, -1)
            } else {
                measured
            };
            #[cfg(debug_assertions)]
            self.record(0, started.elapsed().as_micros());
            result
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            #[cfg(debug_assertions)]
            let started = std::time::Instant::now();
            if self.width.replace(width) != width {
                let owner = self.obj();
                let children = {
                    let mut children = self.children.borrow_mut();
                    children.retain(|(child, _)| {
                        child
                            .upgrade()
                            .is_some_and(|c| c.parent().as_ref() == Some(owner.upcast_ref()))
                    });
                    children.clone()
                };
                for (child, inset) in children {
                    if let Some(child) = child.upgrade() {
                        self.fit(&child, inset, width);
                    }
                }
            }
            self.parent_size_allocate(width, height, baseline);
            self.update_caret_object();
            #[cfg(debug_assertions)]
            self.record(2, started.elapsed().as_micros());
        }
    }
    impl AnchoredTextView {
        pub fn update_caret_object(&self) {
            let view = self.obj();
            let buffer = view.buffer();
            if !view.is_mapped() || !view.has_focus() {
                self.set_caret_object(None);
                return;
            }
            // Inspect just the insertion slot and its predecessor, including
            // EOF immediately after an anchor. Never walk the document/children.
            let at = buffer.iter_at_mark(&buffer.get_insert());
            let anchor = at.child_anchor();
            let mut previous = at;
            let anchors = [
                anchor,
                previous
                    .backward_char()
                    .then(|| previous.child_anchor())
                    .flatten(),
            ];
            if anchors.iter().all(Option::is_none) {
                self.set_caret_object(None);
                return;
            }
            let metrics = view.pango_context().metrics(None, None);
            let line_height =
                (metrics.ascent().saturating_add(metrics.descent()) / gtk::pango::SCALE).max(1);
            let child = anchors
                .into_iter()
                .flatten()
                .flat_map(|anchor| anchor.widgets())
                .find(|child| {
                    child.parent().as_ref() == Some(view.upcast_ref())
                        && child.is_mapped()
                        && child.height() > line_height.saturating_mul(2)
                });
            self.set_caret_object(child);
        }
        fn set_caret_object(&self, child: Option<gtk::Widget>) {
            let view = self.obj();
            if let Some(child) = child.as_ref() {
                // The insert endpoint can remain beside an anchor during range
                // selection. Suppress its paint, but leave the range outline
                // to flow_object_selection rather than showing an object cue.
                if view.buffer().selection_bounds().is_none() {
                    child.add_css_class("studio-anchor-caret-object");
                } else {
                    child.remove_css_class("studio-anchor-caret-object");
                }
            }
            let current = self
                .caret_object
                .borrow()
                .as_ref()
                .and_then(|(weak, _)| weak.upgrade());
            if current.as_ref() == child.as_ref()
                && (child.is_some() || self.caret_object.borrow().is_none())
            {
                return;
            }
            if let Some((weak, handler)) = self.caret_object.borrow_mut().take() {
                if let Some(old) = weak.upgrade() {
                    old.disconnect(handler);
                    old.remove_css_class("studio-anchor-caret-object");
                }
            }
            if let Some(child) = child {
                // A table can be detached/reused before the owning view unmaps.
                let weak = view.downgrade();
                let handler = child.connect_unmap(move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.imp().set_caret_object(None);
                    }
                });
                self.caret_object
                    .replace(Some((child.downgrade(), handler)));
                view.add_css_class("studio-anchor-caret");
            } else {
                view.remove_css_class("studio-anchor-caret");
            }
        }
        #[cfg(debug_assertions)]
        fn record(&self, offset: usize, elapsed: u128) {
            self.timings[offset].set(self.timings[offset].get().saturating_add(elapsed));
            self.timings[offset + 1].set(self.timings[offset + 1].get().max(elapsed));
        }
        pub fn fit(&self, child: &gtk::Widget, inset: i32, width: i32) {
            let width = width.saturating_sub(inset).max(1);
            if child.width_request() != width {
                child.set_width_request(width);
                #[cfg(debug_assertions)]
                self.updates.set(self.updates.get().saturating_add(1));
            }
        }
    }
}
glib::wrapper! {
    pub struct AnchoredTextView(ObjectSubclass<imp::AnchoredTextView>)
        @extends gtk::TextView,gtk::Widget,
        @implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget,gtk::AccessibleText,gtk::Scrollable;
}
impl AnchoredTextView {
    pub fn new(buffer: &gtk::TextBuffer) -> Self {
        let view: Self = glib::Object::builder().property("buffer", buffer).build();
        let weak = view.downgrade();
        buffer.connect_mark_set(move |buffer, _, mark| {
            if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                if let Some(view) = weak.upgrade().filter(|v| v.buffer() == *buffer) {
                    view.imp().update_caret_object();
                }
            }
        });
        view.connect_has_focus_notify(|view| view.imp().update_caret_object());
        view
    }
    /// GTK supplies buffer coordinates, before native text/selection painting.
    pub fn set_below_text(&self, draw: TextLayer) {
        self.imp().below_text.replace(Some(draw));
        self.queue_draw();
    }
    pub fn fill_width(&self, child: &gtk::Widget, inset: i32) {
        // GtkTextView reserves one additional pixel for its insertion cursor
        // when setting the layout width, including in a read-only view.
        let inset = inset.max(0).saturating_add(1);
        self.imp()
            .children
            .borrow_mut()
            .push((child.downgrade(), inset));
        self.imp().fit(child, inset, self.width());
    }
    #[cfg(debug_assertions)]
    pub fn layout_updates(&self) -> u64 {
        self.imp().updates.get()
    }
    /// Microsecond total/max pairs for measure, allocation and text snapshot layers.
    #[cfg(debug_assertions)]
    pub fn layout_timings(&self) -> [u128; 6] {
        std::array::from_fn(|i| self.imp().timings[i].get())
    }
}

// Opt-in anchor adapter. GtkTextView allocates child anchors at their minimum
// requisition; a scroller's natural height alone is therefore insufficient.
mod natural_height_imp {
    use super::*;
    use std::cell::OnceCell;

    #[derive(Default)]
    pub struct NaturalHeightChild {
        pub child: OnceCell<gtk::Widget>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for NaturalHeightChild {
        const NAME: &'static str = "StudioNaturalHeightChild";
        type Type = super::NaturalHeightChild;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for NaturalHeightChild {
        fn dispose(&self) {
            if let Some(child) = self.child.get().filter(|child| child.parent().is_some()) {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for NaturalHeightChild {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            self.child
                .get()
                .map_or(gtk::SizeRequestMode::ConstantSize, |child| {
                    child.request_mode()
                })
        }
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.get() else {
                return (0, 0, -1, -1);
            };
            let measured = child.measure(orientation, for_size);
            if orientation == gtk::Orientation::Vertical {
                let (_, natural, _, baseline) = measured;
                (natural, natural, baseline, baseline)
            } else {
                measured
            }
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.child.get() {
                child.allocate(width, height, baseline, None);
            }
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(child) = self.child.get() {
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
}

glib::wrapper! {
    pub struct NaturalHeightChild(ObjectSubclass<natural_height_imp::NaturalHeightChild>)
        @extends gtk::Widget,
        @implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget;
}
impl NaturalHeightChild {
    /// Promote a child's measured vertical natural size for a minimum-sized
    /// anchor. The child must supply its own content bounds (for example a
    /// ScrolledWindow max-content-height). GTK retains resize/reflow ownership.
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let result: Self = glib::Object::new();
        let child = child.as_ref();
        result.imp().child.set(child.clone()).unwrap();
        child.set_parent(&result);
        result
    }
}
