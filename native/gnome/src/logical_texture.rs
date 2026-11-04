//! A texture with stable logical dimensions, independent of raster resolution.
use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};
    #[derive(Default)]
    pub struct LogicalTexture {
        pub size: Cell<(i32, i32)>,
        pub texture: RefCell<Option<gdk::Texture>>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for LogicalTexture {
        const NAME: &'static str = "StudioLogicalTexture";
        type Type = super::LogicalTexture;
        type Interfaces = (gdk::Paintable,);
    }
    impl ObjectImpl for LogicalTexture {}
    impl PaintableImpl for LogicalTexture {
        fn intrinsic_width(&self) -> i32 {
            self.size.get().0
        }
        fn intrinsic_height(&self) -> i32 {
            self.size.get().1
        }
        fn intrinsic_aspect_ratio(&self) -> f64 {
            self.size.get().0 as f64 / self.size.get().1.max(1) as f64
        }
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_SIZE
        }
        fn current_image(&self) -> gdk::Paintable {
            let value: super::LogicalTexture = glib::Object::new();
            value.imp().size.set(self.size.get());
            value.imp().texture.replace(self.texture.borrow().clone());
            value.upcast()
        }
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            if let Some(texture) = self.texture.borrow().as_ref() {
                texture.snapshot(snapshot, width, height);
            }
        }
    }
}
glib::wrapper! {
    pub struct LogicalTexture(ObjectSubclass<imp::LogicalTexture>) @implements gdk::Paintable;
}
impl LogicalTexture {
    pub fn new(texture: &gdk::Texture) -> Self {
        let value: Self = glib::Object::new();
        value.imp().size.set((texture.width(), texture.height()));
        value.imp().texture.replace(Some(texture.clone()));
        value
    }
    pub fn texture(&self) -> Option<gdk::Texture> {
        self.imp().texture.borrow().clone()
    }
    pub fn set_texture(&self, texture: Option<&gdk::Texture>) {
        self.imp().texture.replace(texture.cloned());
        self.invalidate_contents();
    }
}
