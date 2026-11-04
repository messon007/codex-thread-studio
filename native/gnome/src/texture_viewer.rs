//! Native diagram inspection. Rasterize retained vectors on bounded Rust workers.
use crate::i18n_ui::{self, UiText};
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
};

struct Viewer {
    picture: gtk::Picture,
    scroll: gtk::ScrolledWindow,
    fit: gtk::ToggleButton,
    actual: gtk::Button,
    zoom: Cell<f64>,
    size: (i32, i32),
    base: RefCell<Option<gdk::Texture>>,
    vector: RefCell<Option<Arc<crate::diagram::Vector>>>,
    paintable: crate::logical_texture::LogicalTexture,
    caption: gtk::Label,
    copy: gtk::Button,
    pending: RefCell<Option<glib::SourceId>>,
    task: RefCell<Option<glib::JoinHandle<()>>>,
    cancel: RefCell<Option<Arc<AtomicBool>>>,
    sequence: Cell<u64>,
    wanted: Cell<(u32, u32)>,
    closed: Cell<bool>,
    _size_watch: gtk::WidgetPaintable,
}
impl Viewer {
    fn layout(self: &Rc<Self>) {
        let fit = self.fit.is_active();
        self.picture.set_content_fit(if fit {
            gtk::ContentFit::ScaleDown
        } else {
            gtk::ContentFit::Contain
        });
        self.picture.set_halign(if fit {
            gtk::Align::Fill
        } else {
            gtk::Align::Center
        });
        self.picture.set_valign(if fit {
            gtk::Align::Fill
        } else {
            gtk::Align::Center
        });
        let size = if fit {
            (-1, -1)
        } else {
            (
                (self.size.0 as f64 * self.zoom.get()).round() as i32,
                (self.size.1 as f64 * self.zoom.get()).round() as i32,
            )
        };
        self.picture.set_size_request(size.0, size.1);
        let policy = if fit {
            gtk::PolicyType::Never
        } else {
            gtk::PolicyType::Automatic
        };
        self.scroll.set_policy(policy, policy);
        self.actual
            .set_label(&format!("{:.0}%", self.zoom.get() * 100.0));
        self.schedule();
    }
    fn zoom(self: &Rc<Self>, multiplier: f64) {
        self.zoom.set(if multiplier == 0.0 {
            1.0
        } else {
            (self.zoom.get() * multiplier).clamp(0.1, 4.0)
        });
        self.fit.set_active(false);
        self.layout();
    }
    fn cancel_work(&self) {
        if let Some(cancel) = self.cancel.borrow_mut().take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(timer) = self.pending.borrow_mut().take() {
            timer.remove();
        }
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
    }
    fn schedule(self: &Rc<Self>) {
        if self.closed.get() {
            return;
        }
        let Some(vector) = self.vector.borrow().clone() else {
            return;
        };
        let factor = if self.fit.is_active() {
            (self.scroll.width().max(1) as f64 / self.size.0 as f64)
                .min(self.scroll.height().max(1) as f64 / self.size.1 as f64)
                .min(1.0)
        } else {
            self.zoom.get()
        };
        let factor = (factor * self.picture.scale_factor() as f64).clamp(1.0, 32.0);
        let Ok(wanted) = vector.dimensions(factor) else {
            return;
        };
        if self.wanted.replace(wanted) == wanted {
            return;
        }
        let sequence = self.sequence.get().wrapping_add(1);
        self.sequence.set(sequence);
        self.cancel_work();
        if wanted == (self.size.0 as u32, self.size.1 as u32) {
            self.paintable.set_texture(self.base.borrow().as_ref());
            self.dimensions(self.size.0 as u32, self.size.1 as u32);
            self.copy.set_sensitive(true);
            return;
        }
        self.copy.set_sensitive(false);
        i18n_ui::bind(&self.caption, "label", "Rendering diagram at this zoom…");
        let weak = Rc::downgrade(self);
        self.pending.replace(Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(80),
            move || {
                let Some(view) = weak.upgrade() else {
                    return;
                };
                view.pending.borrow_mut().take();
                view.launch(vector, factor, sequence);
            },
        )));
    }
    fn launch(self: &Rc<Self>, vector: Arc<crate::diagram::Vector>, factor: f64, sequence: u64) {
        static WORKERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
        let workers = WORKERS
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
            .clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel.replace(Some(cancel.clone()));
        let weak = Rc::downgrade(self);
        let task = glib::spawn_future_local(async move {
            let Ok(permit) = workers.acquire_owned().await else {
                return;
            };
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            // The closure retains the permit even if its GTK waiter is aborted.
            // Superseding a render cannot increase actual CPU concurrency.
            let result = gio::spawn_blocking(move || {
                let _permit = permit;
                vector.rasterize(factor, &cancel)
            })
            .await
            .map_err(|_| "Vector raster worker stopped".to_string())
            .and_then(|r| r);
            let Some(view) = weak
                .upgrade()
                .filter(|v| !v.closed.get() && v.sequence.get() == sequence)
            else {
                return;
            };
            view.task.borrow_mut().take();
            view.cancel.borrow_mut().take();
            match result {
                Ok(image) => {
                    let texture = gdk::MemoryTexture::new(
                        image.width as i32,
                        image.height as i32,
                        gdk::MemoryFormat::R8g8b8a8Premultiplied,
                        &glib::Bytes::from_owned(image.pixels),
                        image.width as usize * 4,
                    );
                    view.paintable.set_texture(Some(texture.upcast_ref()));
                    view.dimensions(image.width, image.height);
                }
                Err(error) => i18n_ui::bind_format(
                    &view.caption,
                    "label",
                    "Unable to render at this zoom: {error}",
                    &[("error", &error)],
                ),
            }
            view.copy.set_sensitive(true);
        });
        self.task.replace(Some(task));
    }
    fn close(&self) {
        self.closed.set(true);
        self.cancel_work();
        self.vector.borrow_mut().take();
        self.base.borrow_mut().take();
        self.paintable.set_texture(None);
        self.picture.set_paintable(None::<&gdk::Paintable>);
    }
    fn dimensions(&self, width: u32, height: u32) {
        i18n_ui::bind_format(
            &self.caption,
            "label",
            "Rendered image · {width} × {height} pixels",
            &[
                ("width", &width.to_string()),
                ("height", &height.to_string()),
            ],
        );
    }
}

pub fn open(
    parent: &impl IsA<gtk::Widget>,
    texture: &gdk::Texture,
    vector: Option<Arc<crate::diagram::Vector>>,
    kind: &str,
) -> adw::Dialog {
    let title = format!("{kind} diagram");
    let dialog = adw::Dialog::builder()
        .title(&title)
        .content_width(1100)
        .content_height(760)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    i18n_ui::bind_format(&dialog, "title", "{kind} diagram", &[("kind", kind)]);
    let heading = adw::WindowTitle::new(&title, "");
    i18n_ui::bind_format(&heading, "title", "{kind} diagram", &[("kind", kind)]);
    header.set_title_widget(Some(&heading));
    toolbar.add_top_bar(&header);
    let controls = crate::adaptive::toolbar(6);
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    controls.set_margin_top(6);
    controls.set_margin_bottom(6);
    let minus = gtk::Button::builder()
        .icon_name("zoom-out-symbolic")
        .tooltip_text("Zoom out")
        .build()
        .ui_text("tooltip-text", "Zoom out");
    let actual = gtk::Button::with_label("100%");
    i18n_ui::bind(
        &actual,
        "tooltip-text",
        "Actual size: one rendered pixel per display pixel",
    );
    let plus = gtk::Button::builder()
        .icon_name("zoom-in-symbolic")
        .tooltip_text("Zoom in")
        .build()
        .ui_text("tooltip-text", "Zoom in");
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy rendered image")
        .build()
        .ui_text("tooltip-text", "Copy rendered image");
    let fit = gtk::ToggleButton::with_label("Fit");
    i18n_ui::bind(&fit, "label", "Fit");
    i18n_ui::bind(&fit, "tooltip-text", "Fit diagram to the available space");
    fit.set_active(true);
    for control in [
        minus.upcast_ref::<gtk::Widget>(),
        actual.upcast_ref(),
        plus.upcast_ref(),
        copy.upcast_ref(),
        fit.upcast_ref(),
    ] {
        controls.append(control);
    }
    toolbar.add_top_bar(&controls);
    let paintable = crate::logical_texture::LogicalTexture::new(texture);
    let picture = gtk::Picture::builder()
        .paintable(&paintable)
        .alternative_text(&title)
        .can_shrink(true)
        .content_fit(gtk::ContentFit::Contain)
        .build();
    i18n_ui::bind_format(
        &picture,
        "alternative-text",
        "{kind} diagram",
        &[("kind", kind)],
    );
    let scroll = gtk::ScrolledWindow::builder()
        .child(&picture)
        .vexpand(true)
        .hexpand(true)
        .build();
    toolbar.set_content(Some(&scroll));
    let caption = gtk::Label::builder()
        .label(format!(
            "Rendered image · {} × {} pixels",
            texture.width(),
            texture.height()
        ))
        .xalign(0.0)
        .wrap(true)
        .build();
    caption.add_css_class("caption");
    caption.set_margin_start(12);
    caption.set_margin_end(12);
    caption.set_margin_top(6);
    caption.set_margin_bottom(6);
    toolbar.add_bottom_bar(&caption);
    dialog.set_child(Some(&toolbar));
    let view = Rc::new(Viewer {
        _size_watch: gtk::WidgetPaintable::new(Some(&scroll)),
        base: RefCell::new(Some(texture.clone())),
        vector: RefCell::new(vector),
        paintable,
        caption,
        copy: copy.clone(),
        pending: RefCell::default(),
        task: RefCell::default(),
        cancel: RefCell::default(),
        sequence: Cell::new(0),
        wanted: Cell::new((texture.width() as u32, texture.height() as u32)),
        closed: Cell::new(false),
        picture,
        scroll,
        fit,
        actual,
        zoom: Cell::new(1.0),
        size: (texture.width(), texture.height()),
    });
    view.dimensions(texture.width() as u32, texture.height() as u32);
    let weak = Rc::downgrade(&view);
    view.fit.connect_toggled(move |_| {
        if let Some(view) = weak.upgrade() {
            view.layout();
        }
    });
    for (button, multiplier) in [(&minus, 0.8), (&view.actual, 0.0), (&plus, 1.25)] {
        let weak = Rc::downgrade(&view);
        button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.zoom(multiplier);
            }
        });
    }
    let weak = Rc::downgrade(&view);
    copy.connect_clicked(move |button| {
        if let Some(texture) = weak.upgrade().and_then(|v| v.paintable.texture()) {
            button.clipboard().set_texture(&texture);
        }
    });
    let weak = Rc::downgrade(&view);
    view._size_watch.connect_invalidate_size(move |_| {
        if let Some(view) = weak.upgrade() {
            view.schedule();
        }
    });
    let weak = Rc::downgrade(&view);
    view.picture.connect_scale_factor_notify(move |_| {
        if let Some(view) = weak.upgrade() {
            view.schedule();
        }
    });
    view.layout();
    // The dialog owns state; state owns no dialog. Clear pixels on close even
    // when an accessibility client or test retains the closed dialog object.
    dialog.connect_closed(move |_| view.close());
    dialog.present(Some(parent));
    dialog
}

#[cfg(debug_assertions)]
pub async fn smoke(
    source: &crate::scaled_picture::ScaledPicture,
    inspect: &gtk::Button,
    name: &str,
    capture: impl Fn(&str),
) -> Result<serde_json::Value, String> {
    use gtk::glib;
    use std::time::Duration;
    async fn measured(
        test: impl std::future::Future<Output = Result<(), String>>,
    ) -> Result<serde_json::Value, String> {
        let ticks = Rc::new(Cell::new(0u64));
        let gap = Rc::new(Cell::new(0u128));
        let last = Rc::new(Cell::new(std::time::Instant::now()));
        let (count, maximum, previous) = (ticks.clone(), gap.clone(), last.clone());
        let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
            maximum.set(
                maximum.get().max(
                    previous
                        .replace(std::time::Instant::now())
                        .elapsed()
                        .as_millis(),
                ),
            );
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        });
        let at = std::time::Instant::now();
        let result = test.await;
        timer.remove();
        result?;
        Ok(
            serde_json::json!({"readyMs":at.elapsed().as_millis(),"mainLoopTicks":ticks.get(),"maximumGapMs":gap.get().max(last.get().elapsed().as_millis())}),
        )
    }
    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
        let at = std::time::Instant::now();
        while !test() {
            if at.elapsed() > Duration::from_secs(5) {
                return Err("Diagram inspector did not become ready".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    fn find(widget: &gtk::Widget, test: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
        if test(widget) {
            return Some(widget.clone());
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            if let Some(found) = find(&current, test) {
                return Some(found);
            }
        }
        None
    }
    let parent = inspect
        .root()
        .and_downcast::<adw::ApplicationWindow>()
        .ok_or("No inspector window")?;
    let original = source
        .paintable()
        .and_downcast::<gdk::Texture>()
        .ok_or("No diagram texture")?;
    let vector = source.vector().ok_or("Diagram lost its vector output")?;
    inspect.emit_clicked();
    let dialog = parent
        .visible_dialog()
        .ok_or("Inspect button did not open a dialog")?;
    capture(&format!("inspect-{name}-fit.png"));
    let picture = find(dialog.upcast_ref(), &|w| w.is::<gtk::Picture>())
        .and_downcast::<gtk::Picture>()
        .ok_or("Inspector picture missing")?;
    let raster = picture
        .paintable()
        .and_downcast::<crate::logical_texture::LogicalTexture>()
        .ok_or("Inspector logical paintable missing")?;
    let button = |tooltip: &str| {
        find(dialog.upcast_ref(), &|w| {
            w.tooltip_text().as_deref() == Some(tooltip)
        })
        .and_downcast::<gtk::Button>()
        .ok_or("Inspector control missing")
    };
    let actual = button("Actual size: one rendered pixel per display pixel")?;
    actual.emit_clicked();
    until(|| {
        picture.width_request() == original.width() && picture.height_request() == original.height()
    })
    .await?;
    let plus = button("Zoom in")?;
    let expected = vector.dimensions((4.0 * picture.scale_factor() as f64).min(32.0))?;
    let timing = measured(async {
        for _ in 0..10 {
            plus.emit_clicked();
        }
        if actual.label().as_deref() != Some("400%")
            || picture.width_request() != original.width() * 4
        {
            return Err("Inspector zoom did not stop at 400%".into());
        }
        until(|| {
            raster
                .texture()
                .is_some_and(|t| (t.width() as u32, t.height() as u32) == expected)
        })
        .await?;
        if raster.intrinsic_width() != original.width()
            || raster.intrinsic_height() != original.height()
        {
            return Err("Vector raster changed logical layout size".into());
        }
        Ok(())
    })
    .await?;
    capture(&format!("inspect-{name}-zoom.png"));
    let scroll = picture
        .parent()
        .and_downcast::<gtk::Viewport>()
        .and_then(|v| v.parent())
        .and_downcast::<gtk::ScrolledWindow>()
        .or_else(|| {
            find(dialog.upcast_ref(), &|w| w.is::<gtk::ScrolledWindow>())
                .and_downcast::<gtk::ScrolledWindow>()
        })
        .ok_or("Inspector scroll viewport missing")?;
    until(|| picture.width() >= picture.width_request()).await?;
    if picture.width_request() > scroll.width() {
        until(|| scroll.hadjustment().upper() > scroll.hadjustment().page_size()).await?;
    }
    // An open inspector is an immutable view, independent of virtualized rows.
    source.clear_texture();
    if inspect.is_sensitive() || raster.texture().is_none() {
        return Err("Source unload invalidated the open inspector or retained its button".into());
    }
    actual.emit_clicked();
    for _ in 0..10 {
        plus.emit_clicked();
    }
    until(|| {
        raster
            .texture()
            .is_some_and(|t| (t.width() as u32, t.height() as u32) == expected)
            && button("Copy rendered image").is_ok_and(|b| b.is_sensitive())
    })
    .await?;
    button("Copy rendered image")?.emit_clicked();
    let copied = inspect
        .clipboard()
        .read_texture_future()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Inspector Copy returned no texture")?;
    if (copied.width() as u32, copied.height() as u32) != expected {
        return Err("Inspector Copy did not retain complete vector-zoom pixels".into());
    }
    let fit = find(dialog.upcast_ref(), &|w| {
        w.clone()
            .downcast::<gtk::ToggleButton>()
            .is_ok_and(|b| b.label().as_deref() == Some("Fit"))
    })
    .and_downcast::<gtk::ToggleButton>()
    .ok_or("Inspector Fit control missing")?;
    fit.set_active(true);
    if picture.width_request() != -1 || picture.height_request() != -1 {
        return Err("Inspector Fit retained a zoom size request".into());
    }
    dialog.close();
    until(|| parent.visible_dialog().is_none()).await?;
    if picture.paintable().is_some() || raster.texture().is_some() {
        return Err("Closed inspector retained pixels".into());
    }
    source.smoke_restore(&original, Some(vector));
    if !inspect.is_sensitive() {
        return Err("Restored diagram cannot open the inspector".into());
    }
    Ok(
        serde_json::json!({"ok":true,"vectorRaster":true,"pixels":[expected.0,expected.1],"scaleFactor":picture.scale_factor(),"zoom":timing}),
    )
}
