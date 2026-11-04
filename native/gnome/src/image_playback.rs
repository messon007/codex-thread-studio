//! Shared visible-only GTK playback of worker-validated image frames.
use crate::image_animation::Timeline;
use adw::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

mod canvas {
    use super::*;
    use gtk::gdk::subclass::prelude::*;
    #[derive(Default)]
    pub struct Canvas {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub size: Cell<(i32, i32)>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Canvas {
        const NAME: &'static str = "StudioImageAnimationPaintable";
        type Type = super::Canvas;
        type Interfaces = (gdk::Paintable,);
    }
    impl ObjectImpl for Canvas {}
    impl PaintableImpl for Canvas {
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_SIZE
        }
        fn intrinsic_width(&self) -> i32 {
            self.size.get().0
        }
        fn intrinsic_height(&self) -> i32 {
            self.size.get().1
        }
        fn intrinsic_aspect_ratio(&self) -> f64 {
            let (w, h) = self.size.get();
            if h > 0 {
                w as f64 / h as f64
            } else {
                0.
            }
        }
        fn current_image(&self) -> gdk::Paintable {
            self.texture
                .borrow()
                .as_ref()
                .map(|t| t.clone().upcast())
                .unwrap_or_else(|| gdk::Paintable::new_empty(1, 1))
        }
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            if let Some(texture) = self.texture.borrow().as_ref() {
                texture.snapshot(snapshot, width, height);
            }
        }
    }
}
glib::wrapper! {pub struct Canvas(ObjectSubclass<canvas::Canvas>) @implements gdk::Paintable;}
impl Canvas {
    fn new(data: &Timeline, index: usize) -> Self {
        use gtk::subclass::prelude::*;
        let canvas: Self = glib::Object::new();
        canvas
            .imp()
            .size
            .set((data.width as i32, data.height as i32));
        canvas.frame(data, index);
        canvas
    }
    fn frame(&self, data: &Timeline, index: usize) {
        use gtk::subclass::prelude::*;
        let texture = gdk::MemoryTexture::new(
            data.width as i32,
            data.height as i32,
            gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &glib::Bytes::from_owned(data.frames[index].pixels.clone()),
            data.width as usize * 4,
        );
        self.imp().texture.replace(Some(texture.upcast()));
        self.invalidate_contents();
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Empty,
    Limited,
    Replay,
    Playing,
    Paused,
}

pub struct Player {
    owner: Weak<Self>,
    picture: glib::WeakRef<gtk::Picture>,
    button: glib::WeakRef<gtk::Button>,
    timeline: RefCell<Option<Arc<Timeline>>>,
    visibility: RefCell<Option<Rc<crate::viewport::Visibility>>>,
    settings: Option<gtk::Settings>,
    setting_handler: RefCell<Option<glib::SignalHandlerId>>,
    timer: RefCell<Option<glib::SourceId>>,
    due: Cell<Option<Instant>>,
    remaining: Cell<Duration>,
    visible: Cell<bool>,
    manual: Cell<Option<bool>>,
    index: Cell<usize>,
    completed: Cell<u32>,
    finished: Cell<bool>,
    limited: Cell<bool>,
    canvas: RefCell<Option<Canvas>>,
    last_control: Cell<Option<Control>>,
}
impl Player {
    pub fn new(picture: &gtk::Picture, button: &gtk::Button) -> Rc<Self> {
        let player = Rc::new_cyclic(|owner| Self {
            owner: owner.clone(),
            picture: picture.downgrade(),
            button: button.downgrade(),
            timeline: RefCell::default(),
            visibility: RefCell::default(),
            settings: gtk::Settings::default(),
            setting_handler: RefCell::default(),
            timer: RefCell::default(),
            due: Cell::new(None),
            remaining: Cell::new(Duration::ZERO),
            visible: Cell::new(false),
            manual: Cell::new(None),
            index: Cell::new(0),
            completed: Cell::new(0),
            finished: Cell::new(false),
            limited: Cell::new(false),
            canvas: RefCell::default(),
            last_control: Cell::new(None),
        });
        let weak = Rc::downgrade(&player);
        button.connect_clicked(move |_| {
            if let Some(player) = weak.upgrade() {
                player.toggle();
            }
        });
        if let Some(settings) = &player.settings {
            let weak = Rc::downgrade(&player);
            player
                .setting_handler
                .replace(Some(settings.connect_gtk_enable_animations_notify(
                    move |_| {
                        if let Some(player) = weak.upgrade() {
                            player.update();
                        }
                    },
                )));
        }
        let weak = Rc::downgrade(&player);
        player
            .visibility
            .replace(Some(crate::viewport::Visibility::new(
                picture,
                Rc::new(move |visible| {
                    if let Some(player) = weak.upgrade() {
                        player.visible.set(visible);
                        player.update();
                    }
                }),
            )));
        player.controls();
        player
    }
    fn wanted(&self) -> bool {
        self.manual.get().unwrap_or_else(|| {
            self.settings
                .as_ref()
                .is_none_or(|s| s.is_gtk_enable_animations())
        })
    }
    pub fn clear(&self, reset_choice: bool) {
        self.stop();
        self.timeline.borrow_mut().take();
        self.canvas.borrow_mut().take();
        self.finished.set(false);
        self.index.set(0);
        self.completed.set(0);
        self.limited.set(false);
        if reset_choice {
            self.manual.set(None);
        }
        self.controls();
    }
    pub fn set(&self, timeline: Option<Arc<Timeline>>, limited: bool) {
        self.clear(false);
        self.limited.set(limited);
        if let Some(data) = &timeline {
            self.remaining.set(data.frames[0].delay);
            let canvas = Canvas::new(data, 0);
            if let Some(picture) = self.picture.upgrade() {
                picture.set_paintable(Some(&canvas));
            }
            self.canvas.replace(Some(canvas));
        }
        self.timeline.replace(timeline);
        self.update();
    }
    fn stop(&self) {
        if let Some(due) = self.due.take() {
            self.remaining
                .set(due.saturating_duration_since(Instant::now()));
        }
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
    }
    fn update(&self) {
        self.controls();
        if !self.visible.get()
            || !self.wanted()
            || self.finished.get()
            || self.timeline.borrow().is_none()
        {
            self.stop();
            return;
        }
        if self.timer.borrow().is_some() {
            return;
        }
        let delay = self.remaining.get().max(Duration::from_millis(1));
        self.due.set(Some(Instant::now() + delay));
        let weak = self.owner.clone();
        let timer = glib::timeout_add_local_once(delay, move || {
            if let Some(player) = weak.upgrade() {
                player.timer.borrow_mut().take();
                player.due.set(None);
                player.advance();
            }
        });
        self.timer.replace(Some(timer));
    }
    fn advance(&self) {
        if !self.visible.get() || !self.wanted() {
            self.update();
            return;
        }
        let Some(data) = self.timeline.borrow().clone() else {
            return;
        };
        let mut index = self.index.get() + 1;
        if index == data.frames.len() {
            if data.plays > 0 {
                self.completed.set(self.completed.get().saturating_add(1));
            }
            if data.plays > 0 && self.completed.get() >= data.plays {
                self.finished.set(true);
                self.controls();
                return;
            }
            index = 0;
        }
        self.index.set(index);
        self.paint(&data, index);
        self.remaining.set(data.frames[index].delay);
        self.update();
    }
    fn paint(&self, data: &Timeline, index: usize) {
        if let Some(canvas) = self.canvas.borrow().as_ref() {
            canvas.frame(data, index);
        }
    }
    fn toggle(&self) {
        if self.timeline.borrow().is_none() {
            return;
        }
        if self.finished.replace(false) {
            self.index.set(0);
            self.completed.set(0);
            self.manual.set(Some(true));
            if let Some(data) = self.timeline.borrow().as_ref() {
                self.paint(data, 0);
                self.remaining.set(data.frames[0].delay);
            }
        } else {
            self.manual.set(Some(!self.wanted()));
        }
        self.update();
    }
    fn controls(&self) {
        let Some(button) = self.button.upgrade() else {
            return;
        };
        let animated = self.timeline.borrow().is_some();
        let control = if self.limited.get() {
            Control::Limited
        } else if !animated {
            Control::Empty
        } else if self.finished.get() {
            Control::Replay
        } else if self.wanted() {
            Control::Playing
        } else {
            Control::Paused
        };
        if self.last_control.replace(Some(control)) == Some(control) {
            return;
        }
        button.set_visible(animated || self.limited.get());
        button.set_sensitive(animated);
        let (icon, text) = if self.limited.get() {
            (
                "media-playback-start-symbolic",
                "Animation unavailable; showing still preview",
            )
        } else if self.finished.get() {
            ("media-playback-start-symbolic", "Replay animation")
        } else if self.wanted() {
            ("media-playback-pause-symbolic", "Pause animation")
        } else {
            ("media-playback-start-symbolic", "Play animation")
        };
        button.set_icon_name(icon);
        crate::i18n_ui::bind(&button, "tooltip-text", text);
        crate::i18n_ui::accessible_label(&button, text);
    }
    #[cfg(debug_assertions)]
    pub fn state(&self) -> serde_json::Value {
        serde_json::json!({"frames":self.timeline.borrow().as_ref().map_or(0,|t|t.frames.len()),"index":self.index.get(),"completed":self.completed.get(),"finished":self.finished.get(),"timer":self.timer.borrow().is_some(),"visible":self.visible.get(),"wanted":self.wanted(),"limited":self.limited.get()})
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.get_mut().take() {
            timer.remove();
        }
        if let (Some(settings), Some(handler)) =
            (&self.settings, self.setting_handler.get_mut().take())
        {
            settings.disconnect(handler);
        }
    }
}
