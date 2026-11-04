//! Height-for-width image widget for virtualized native document rows.
//! GtkPicture's shrinkable minimum height is zero, which is unsuitable for a
//! GtkListView row measured at its minimum. Keep the aspect ratio in measurement.
use gtk::{glib, prelude::*, subclass::prelude::*};

mod imp {
    use super::*;
    use std::cell::{Cell, OnceCell, RefCell};
    #[derive(Default)]
    pub struct ScaledPicture {
        pub child: OnceCell<gtk::Picture>,
        pub size: Cell<(i32, i32)>,
        pub natural_width: Cell<i32>,
        pub max_height: Cell<i32>,
        pub allocated: RefCell<Option<std::rc::Rc<dyn Fn()>>>,
        pub vector: RefCell<Option<std::sync::Arc<crate::diagram::Vector>>>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for ScaledPicture {
        const NAME: &'static str = "StudioScaledPicture";
        type Type = super::ScaledPicture;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for ScaledPicture {
        fn constructed(&self) {
            self.parent_constructed();
            let child = gtk::Picture::builder()
                .can_shrink(true)
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
    impl WidgetImpl for ScaledPicture {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (width, height) = self.size.get();
            if width <= 0 || height <= 0 {
                return (0, 0, -1, -1);
            }
            let natural = self.natural_width.get().clamp(32, 1280);
            if orientation == gtk::Orientation::Horizontal {
                return (32, natural, -1, -1);
            }
            let available = if for_size < 0 {
                natural
            } else {
                for_size.max(1).min(natural)
            };
            let height = (available as f64 * height as f64 / width as f64)
                .ceil()
                .max(1.0) as i32;
            let maximum = self.max_height.get();
            let height = if maximum > 0 {
                height.min(maximum)
            } else {
                height
            };
            (height, height, -1, -1)
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.child.get() {
                child.allocate(width, height, baseline, None);
            }

            if let Some(callback) = self.allocated.borrow().as_ref() {
                callback();
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
    pub struct ScaledPicture(ObjectSubclass<imp::ScaledPicture>)
        @extends gtk::Widget,
        @implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget;
}

impl ScaledPicture {
    #[cfg(debug_assertions)]
    pub fn smoke_signature(&self) -> Option<serde_json::Value> {
        use std::hash::{Hash, Hasher};
        let texture = self.paintable()?.downcast::<gtk::gdk::Texture>().ok()?;
        let mut download = gtk::gdk::TextureDownloader::new(&texture);
        download.set_format(gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied);
        let (bytes, stride) = download.download_bytes();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for row in bytes
            .as_ref()
            .chunks_exact(stride)
            .take(texture.height() as usize)
        {
            row[..texture.width() as usize * 4].hash(&mut hash);
        }
        Some(
            serde_json::json!({"width":texture.width(),"height":texture.height(),"pixels":hash.finish()}),
        )
    }
    pub fn new(alternative: &str) -> Self {
        let value: Self = glib::Object::builder().build();
        value
            .imp()
            .child
            .get()
            .unwrap()
            .set_alternative_text(Some(alternative));
        value
    }
    pub fn set_texture(&self, texture: &gtk::gdk::Texture) {
        self.imp().vector.borrow_mut().take();
        self.imp().size.set((texture.width(), texture.height()));
        self.imp().natural_width.set(texture.width() / 2);
        self.imp().child.get().unwrap().set_paintable(Some(texture));
        self.queue_resize();
    }
    pub(crate) fn animation_picture(&self) -> gtk::Picture {
        self.imp().child.get().unwrap().clone()
    }
    pub fn set_logical_width(&self, width: u32) {
        self.imp().natural_width.set(width.min(1280) as i32);
        self.queue_resize();
    }
    pub fn on_allocate(&self, callback: std::rc::Rc<dyn Fn()>) {
        self.imp().allocated.replace(Some(callback));
    }
    pub fn set_max_height(&self, height: i32) {
        self.imp().max_height.set(height.max(0));
        self.queue_resize();
    }
    pub fn clear_texture(&self) {
        self.imp().vector.borrow_mut().take();
        // Retain the measured aspect ratio while releasing offscreen pixels.
        self.imp()
            .child
            .get()
            .unwrap()
            .set_paintable(None::<&gtk::gdk::Paintable>);
    }
    pub fn inspect_button(&self, kind: &str) -> gtk::Button {
        let picture = self.imp().child.get().unwrap();
        let button = gtk::Button::builder()
            .icon_name("view-fullscreen-symbolic")
            .tooltip_text("Inspect diagram")
            .sensitive(picture.paintable().is_some())
            .build();
        crate::i18n_ui::bind_format(
            &button,
            "tooltip-text",
            "Inspect {kind} diagram",
            &[("kind", kind)],
        );
        crate::i18n_ui::accessible_parts(
            &button,
            vec![crate::i18n_ui::Part::message(
                "Inspect {kind} diagram",
                &[("kind", kind)],
            )],
            "",
        );
        let weak = button.downgrade();
        picture.connect_paintable_notify(move |picture| {
            if let Some(button) = weak.upgrade() {
                button.set_sensitive(picture.paintable().is_some());
            }
        });
        let source = self.downgrade();
        let kind = kind.to_owned();
        button.connect_clicked(move |button| {
            if let (Some(source), Some(parent)) = (source.upgrade(), button.root()) {
                if let Some(texture) = source.paintable().and_downcast::<gtk::gdk::Texture>() {
                    crate::texture_viewer::open(&parent, &texture, source.vector(), &kind);
                }
            }
        });
        button
    }
    pub fn vector(&self) -> Option<std::sync::Arc<crate::diagram::Vector>> {
        self.imp().vector.borrow().clone()
    }
    pub fn set_diagram(&self, image: crate::diagram::Image) {
        let texture = gtk::gdk::MemoryTexture::new(
            image.width as i32,
            image.height as i32,
            gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &glib::Bytes::from_owned(image.pixels),
            image.width as usize * 4,
        );
        self.set_texture(texture.upcast_ref());
        self.imp().vector.replace(image.vector);
    }
    #[cfg(debug_assertions)]
    pub fn smoke_restore(
        &self,
        texture: &gtk::gdk::Texture,
        vector: Option<std::sync::Arc<crate::diagram::Vector>>,
    ) {
        self.set_texture(texture);
        self.imp().vector.replace(vector);
    }
    pub fn paintable(&self) -> Option<gtk::gdk::Paintable> {
        self.imp().child.get().unwrap().paintable()
    }
}
