//! Debug captures of real mapped window nodes; never drive or force GTK layout.
use adw::prelude::*;
use gtk::glib;
use std::{cell::RefCell, collections::HashMap, time::Duration};

type Observers = HashMap<usize, (glib::WeakRef<gtk::Widget>, gtk::WidgetPaintable)>;
thread_local! {
    static OBSERVERS: RefCell<Observers> = RefCell::new(HashMap::new());
}
fn observe(widget: &gtk::Widget) -> gtk::WidgetPaintable {
    let key = widget.as_ptr() as usize;
    if let Some(p) = OBSERVERS.with_borrow(|m| {
        m.get(&key)
            .filter(|(w, _)| w.upgrade().as_ref() == Some(widget))
            .map(|(_, p)| p.clone())
    }) {
        return p;
    }
    let p = gtk::WidgetPaintable::new(Some(widget));
    OBSERVERS.with_borrow_mut(|m| {
        m.retain(|_, (w, _)| w.upgrade().is_some());
        m.insert(key, (widget.downgrade(), p.clone()));
    });
    widget.connect_unmap(move |w| {
        let p = OBSERVERS.with_borrow_mut(|m| {
            if m.get(&key)
                .is_some_and(|(current, _)| current.upgrade().as_ref() == Some(w))
            {
                m.remove(&key)
            } else {
                None
            }
        });
        if let Some((_, p)) = p {
            p.set_widget(None::<&gtk::Widget>);
        }
    });
    p
}
/// Register before the first normal frame so every debug workflow can capture
/// the root, including workflows that skip the conversation acceptance steps.
pub fn observe_window(window: &impl IsA<gtk::Window>) {
    if let Some(content) = gtk::prelude::GtkWindowExt::child(window.as_ref()) {
        observe(&content);
    }
}
/// Optional passive screenshots for private AT-SPI workflows. This accepts only
/// short output names and never changes a widget, selection or application state.
pub fn observe_requests(window: &impl IsA<gtk::Window>, directory: &std::path::Path) {
    observe_requests_for(window, None, directory, "native-capture-request");
}
/// Observe an actual mapped popup/content surface independently of its parent
/// window. Window snapshots do not include separate Wayland popup surfaces.
pub fn observe_widget_requests(
    window: &impl IsA<gtk::Window>,
    target: &impl IsA<gtk::Widget>,
    directory: &std::path::Path,
    request_name: &'static str,
) {
    observe_requests_for(
        window,
        Some(target.as_ref().downgrade()),
        directory,
        request_name,
    );
}
fn observe_requests_for(
    window: &impl IsA<gtk::Window>,
    target: Option<glib::WeakRef<gtk::Widget>>,
    directory: &std::path::Path,
    request_name: &'static str,
) {
    let weak = window.as_ref().downgrade();
    let directory = directory.to_path_buf();
    glib::spawn_future_local(async move {
        loop {
            let Some(window) = weak.upgrade() else {
                break;
            };
            if !window.is_visible() {
                break;
            }
            let request = directory.join(request_name);
            let name = gtk::gio::spawn_blocking(move || {
                let name = std::fs::read_to_string(&request).ok()?;
                std::fs::remove_file(&request).ok()?;
                Some(name)
            })
            .await
            .ok()
            .flatten();
            if let Some(name) = name.filter(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            }) {
                let path = directory.join(format!("{name}.png"));
                let result = if let Some(target) = target.as_ref() {
                    match target.upgrade().filter(|w| w.is_mapped()) {
                        Some(widget) => match next_frame(&widget).await {
                            Ok(()) => capture_current(&window, &widget, &path),
                            Err(error) => Err(error),
                        },
                        None => Err("Capture target is not mapped".into()),
                    }
                } else {
                    capture(&window, &path).await
                };
                if let Err(error) = result {
                    let _ = std::fs::write(directory.join(format!("{name}.error")), error);
                }
            }
            glib::timeout_future(Duration::from_millis(50)).await;
        }
    });
}
/// Wait for GTK's own layout/paint cycle. Requesting a phase is public GDK
/// scheduling; emitting its signals or allocating children here is not.
pub async fn next_frame(widget: &impl IsA<gtk::Widget>) -> Result<(), String> {
    observe(widget.as_ref());
    if let Some(root) = widget.as_ref().root().and_downcast::<gtk::Window>() {
        if let Some(content) = gtk::prelude::GtkWindowExt::child(&root) {
            observe(&content);
            content.queue_draw();
        }
    }
    let clock = widget
        .as_ref()
        .frame_clock()
        .ok_or("Mapped widget has no frame clock")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let send = std::cell::RefCell::new(Some(send));
    let id = clock.connect_after_paint(move |_| {
        if let Some(send) = send.borrow_mut().take() {
            let _ = send.send(());
        }
    });
    struct Watch(gtk::gdk::FrameClock, Option<glib::SignalHandlerId>);
    impl Drop for Watch {
        fn drop(&mut self) {
            if let Some(id) = self.1.take() {
                self.0.disconnect(id);
            }
        }
    }
    let _watch = Watch(clock.clone(), Some(id));
    clock.request_phase(gtk::gdk::FrameClockPhase::PAINT);
    match futures_util::future::select(
        Box::pin(receive),
        Box::pin(glib::timeout_future(Duration::from_secs(5))),
    )
    .await
    {
        futures_util::future::Either::Left((Ok(()), _)) => Ok(()),
        _ => Err("GTK did not paint the mapped test widget within five seconds".into()),
    }
}
/// A dialog can have loaded its model before its sheet has actually mapped.
/// Let normal parent frames open it, then observe its allocated content.
pub async fn ready(widget: &impl IsA<gtk::Widget>) -> Result<(), String> {
    let widget = widget.as_ref();
    let root = widget
        .root()
        .and_downcast::<gtk::Window>()
        .ok_or("Capture widget is not attached to a window")?;
    let at = std::time::Instant::now();
    while !widget.is_mapped() || widget.width() <= 0 || widget.height() <= 0 {
        if at.elapsed() > Duration::from_secs(5) {
            return Err("Capture widget did not map within five seconds".into());
        }
        next_frame(&root).await?;
    }
    next_frame(widget).await
}
/// Capture GTK's mapped renderable content. Never allocate descendants or
/// emit frame-clock signals to manufacture a screenshot.
pub fn capture_current(
    window: &impl IsA<gtk::Window>,
    content: &impl IsA<gtk::Widget>,
    path: &std::path::Path,
) -> Result<(), String> {
    let window: &gtk::Window = window.as_ref();
    let content: &gtk::Widget = content.as_ref();
    if !content.is_mapped() || content.width() <= 0 || content.height() <= 0 {
        return Err("Capture content has not been mapped and allocated".into());
    }
    let paintable = observe(content);
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, content.width() as f64, content.height() as f64);
    let node = snapshot
        .to_node()
        .ok_or("Capture content has not painted a frame")?;
    save(window, content, &node, path)
}
#[allow(deprecated)]
pub async fn capture(window: &impl IsA<gtk::Window>, path: &std::path::Path) -> Result<(), String> {
    let window: &gtk::Window = window.as_ref();
    let content = gtk::prelude::GtkWindowExt::child(window).ok_or("No application content")?;
    capture_widget(window, &content, path).await
}
#[allow(deprecated)]
pub async fn capture_dialog(
    window: &impl IsA<gtk::Window>,
    dialog: &impl IsA<adw::Dialog>,
    path: &std::path::Path,
) -> Result<(), String> {
    let content = dialog.as_ref().child().ok_or("Dialog has no content")?;
    capture_widget(window, &content, path).await
}
#[allow(deprecated)]
pub async fn capture_widget(
    window: &impl IsA<gtk::Window>,
    content: &impl IsA<gtk::Widget>,
    path: &std::path::Path,
) -> Result<(), String> {
    let window: &gtk::Window = window.as_ref();
    let content: &gtk::Widget = content.as_ref();
    ready(content).await?;
    // A mapped child can have painted while its ancestors still await a frame.
    // Require a real content node before adding the opaque theme background.
    let paintable = observe(content);
    let at = std::time::Instant::now();
    let node = loop {
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(content.width()),
            f64::from(content.height()),
        );
        if let Some(node) = snapshot.to_node() {
            break node;
        }
        if at.elapsed() > Duration::from_secs(5) {
            return Err("Mapped application produced no content snapshot".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    };
    save(window, content, &node, path)
}
#[allow(deprecated)]
fn save(
    window: &gtk::Window,
    content: &gtk::Widget,
    node: &gtk::gsk::RenderNode,
    path: &std::path::Path,
) -> Result<(), String> {
    let snapshot = gtk::Snapshot::new();
    let background = window
        .style_context()
        .lookup_color("window_bg_color")
        .unwrap_or_else(|| gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0));
    snapshot.append_color(
        &background,
        &gtk::graphene::Rect::new(0.0, 0.0, content.width() as f32, content.height() as f32),
    );
    snapshot.append_node(node);
    let node = snapshot.to_node().ok_or("No mapped render node")?;
    window
        .renderer()
        .ok_or("No renderer")?
        .render_texture(&node, None)
        .save_to_png(path)
        .map_err(|e| e.to_string())
}
