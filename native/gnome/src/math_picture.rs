//! Native inline math object with a real text baseline and charged pixel lifetime.
use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use std::{
    cell::{Cell, OnceCell},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};
const BUDGET: usize = 32 * 1024 * 1024;
struct Pixels {
    data: Arc<[u8]>,
    budget: Arc<AtomicUsize>,
}
impl AsRef<[u8]> for Pixels {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}
impl Drop for Pixels {
    fn drop(&mut self) {
        self.budget.fetch_sub(self.data.len(), Ordering::AcqRel);
    }
}
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct MathPicture {
        pub child: OnceCell<gtk::Picture>,
        pub size: Cell<(i32, i32, i32)>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for MathPicture {
        const NAME: &'static str = "StudioInlineMathPicture";
        type Type = super::MathPicture;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for MathPicture {
        fn constructed(&self) {
            self.parent_constructed();
            let child = gtk::Picture::builder()
                .can_shrink(true)
                .can_target(false)
                .content_fit(gtk::ContentFit::Contain)
                .build();
            child.set_parent(&*self.obj());
            self.child.set(child).unwrap();
        }
        fn dispose(&self) {
            if let Some(child) = self.child.get() {
                if child.parent().is_some() {
                    child.unparent();
                }
            }
        }
    }
    impl WidgetImpl for MathPicture {
        fn measure(&self, orientation: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            let (w, h, b) = self.size.get();
            if orientation == gtk::Orientation::Horizontal {
                (w.max(1), w.max(1), -1, -1)
            } else {
                (h.max(1), h.max(1), b, b)
            }
        }
        fn size_allocate(&self, w: i32, h: i32, _: i32) {
            if let Some(child) = self.child.get() {
                child.allocate(w, h, -1, None);
            }
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(child) = self.child.get() {
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
}
glib::wrapper! {pub struct MathPicture(ObjectSubclass<imp::MathPicture>) @extends gtk::Widget, @implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget;}
impl MathPicture {
    pub fn new(image: &crate::diagram::Image, alternative: &str) -> Result<Self, String> {
        let metrics = image.math.as_ref().ok_or("Missing inline math metrics")?;
        if image.width == 0
            || image.height == 0
            || image.pixels.len() > 2 * 1024 * 1024
            || (image.width as usize)
                .checked_mul(image.height as usize)
                .and_then(|n| n.checked_mul(4))
                != Some(image.pixels.len())
            || [metrics.width, metrics.height, metrics.baseline, metrics.em]
                .iter()
                .any(|v| !v.is_finite())
            || metrics.em <= 0.0
            || metrics.width <= 0.0
            || metrics.height <= 0.0
        {
            return Err("Invalid inline math object bounds".into());
        }
        static USED: OnceLock<Arc<AtomicUsize>> = OnceLock::new();
        let budget = USED.get_or_init(|| Arc::new(AtomicUsize::new(0))).clone();
        let bytes = image.pixels.len();
        budget
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |n| {
                n.checked_add(bytes).filter(|n| *n <= BUDGET)
            })
            .map_err(|_| "Too much inline math is visible".to_string())?;
        let texture = gdk::MemoryTexture::new(
            image.width as i32,
            image.height as i32,
            gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &glib::Bytes::from_owned(Pixels {
                data: image.pixels.clone(),
                budget,
            }),
            image.width as usize * 4,
        );
        let result: Self = glib::Object::new();
        let child = result.imp().child.get().unwrap();
        child.set_paintable(Some(&texture));
        child.set_alternative_text(Some(alternative));
        result.set_can_target(false);
        Ok(result)
    }
    pub fn fit(&self, metrics: &crate::math::Metrics, font: f64) {
        let factor = (font / metrics.em).min(8192.0 / metrics.width.max(metrics.height));
        let width = (metrics.width * factor).ceil().clamp(1.0, 8192.0) as i32;
        let height = (metrics.height * factor).ceil().clamp(1.0, 8192.0) as i32;
        let baseline = (metrics.baseline * factor)
            .round()
            .clamp(0.0, height as f64) as i32;
        if self.imp().size.replace((width, height, baseline)) != (width, height, baseline) {
            self.queue_resize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pixel_charge_survives_original_bytes_and_owner_until_final_slice() {
        let budget = Arc::new(AtomicUsize::new(64));
        let bytes = glib::Bytes::from_owned(Pixels {
            data: Arc::from([0u8; 64]),
            budget: budget.clone(),
        });
        let slice = glib::Bytes::from_bytes(&bytes, 16..32);
        drop(bytes);
        assert_eq!(budget.load(Ordering::Acquire), 64);
        assert_eq!(slice.as_ref(), &[0u8; 16]);
        drop(slice);
        assert_eq!(budget.load(Ordering::Acquire), 0);
    }
}
