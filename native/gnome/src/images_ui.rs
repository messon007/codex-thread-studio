//! Shared bounded native image requests and GTK-only presentation.
use crate::{
    i18n_ui::{self, Part, UiText},
    images::Frame,
    protocol::Command,
};
#[cfg(debug_assertions)]
#[path = "image_animation_acceptance.rs"]
mod animation_acceptance;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
type Completed = Box<dyn FnOnce(Result<Frame, String>)>;
#[cfg(debug_assertions)]
#[path = "image_save_acceptance.rs"]
mod save_acceptance;
#[cfg(debug_assertions)]
pub fn loaded_count(widget: &gtk::Widget, mapped_only: bool) -> usize {
    let own = widget
        .clone()
        .downcast::<crate::scaled_picture::ScaledPicture>()
        .ok()
        .is_some_and(|picture| {
            (!mapped_only || picture.is_mapped()) && picture.paintable().is_some()
        }) as usize;
    let mut total = own;
    let mut child = widget.first_child();
    while let Some(current) = child {
        total += loaded_count(&current, mapped_only);
        child = current.next_sibling();
    }
    total
}
struct Job {
    id: u64,
    source: crate::images::Source,
}
pub struct Renderer {
    pub on_open_embedded: RefCell<Option<Rc<dyn Fn(crate::images::Source, String)>>>,
    #[cfg(debug_assertions)]
    pub maximum_callback_ms: Cell<u128>,
    #[cfg(debug_assertions)]
    pub maximum_visibility_ms: Cell<u128>,
    sender: mpsc::Sender<Command>,
    next: Cell<u64>,
    pending: RefCell<VecDeque<Job>>,
    active: RefCell<HashMap<u64, std::sync::Arc<tokio::sync::Notify>>>,
    callbacks: RefCell<HashMap<u64, Completed>>,
    owner: Weak<Renderer>,
    previews: RefCell<HashMap<usize, Weak<Preview>>>,
    watched: RefCell<HashMap<usize, glib::WeakRef<gtk::Adjustment>>>,
    visibility: Rc<crate::viewport::FrameUpdate>,
}
impl Renderer {
    #[cfg(debug_assertions)]
    pub async fn smoke_chat(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        root: &std::path::Path,
        documents: mpsc::Sender<crate::document::Message>,
        capture: impl Fn(&adw::Dialog),
    ) -> Result<serde_json::Value, String> {
        let root = root.to_string_lossy().into_owned();
        let renderer = self.clone();
        let media: crate::chat_ui::ImageAction =
            Rc::new(move |_, content, open| chat(&renderer, content, &root, open));
        let diagrams = crate::chat_diagrams::Renderer::new_with_key(documents, 9900);
        let text = "## Conversation image\n\n![Chat illustration](sample.png)";
        let content = gio::spawn_blocking(move || crate::chat_content::Content::parse(text))
            .await
            .map_err(|_| "Conversation image test worker stopped")??;
        let item = std::sync::Arc::new(crate::model::Item {
            output: None,
            details: None,
            status: None,
            truncated: false,
            images: Vec::new(),
            origin: None,
            id: "private-image-response".into(),
            turn_id: "private-image-turn".into(),
            role: "Assistant".into(),
            body: text.into(),
            markup: None,
            rich: Some(content),
        });
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        crate::chat_ui::reconcile(&model, &[item]);
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(model))),
            Some(crate::chat_ui::factory(
                Rc::new(|_| {}),
                crate::content_widgets::Styles::new(),
                diagrams,
                None,
                Some(media),
            )),
        );
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .child(&list)
                .vexpand(true)
                .build(),
        ));
        let dialog = adw::Dialog::builder()
            .title("Conversation image")
            .content_width(780)
            .content_height(640)
            .child(&toolbar)
            .build();
        dialog.present(Some(parent));
        crate::window_capture::ready(&toolbar).await?;
        capture(&dialog);
        let started = std::time::Instant::now();
        while loaded_count(list.upcast_ref(), true) == 0 {
            if started.elapsed() > std::time::Duration::from_secs(15) {
                capture(&dialog);
                let diagnostic = format!(
                    "Conversation image did not load: mapped={}, pending={}, active={}",
                    list.is_mapped(),
                    self.pending.borrow().len(),
                    self.active.borrow().len()
                );
                dialog.force_close();
                return Err(diagnostic);
            }
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        crate::window_capture::next_frame(&toolbar).await?;
        capture(&dialog);
        dialog.force_close();
        crate::window_capture::next_frame(parent).await?;
        if loaded_count(list.upcast_ref(), false) != 0 {
            return Err("Closing a conversation image retained its texture".into());
        }
        Ok(serde_json::json!({"ok":true,"nativeFactory":true,"unmapReleasesTexture":true}))
    }
    pub fn append_images(
        self: &Rc<Self>,
        container: &gtk::Box,
        content: &studio_markdown_native::RichText,
        root: &std::path::Path,
        document: Option<&std::path::Path>,
        open: crate::content_widgets::OpenLink,
    ) {
        self.append_document_images(container, content, root, document, open, None);
    }
    /// A document-bound renderer shared by text-flow anchors, table cells and
    /// full-cell windows. Reference resolution and decode admission stay here.
    pub fn flow_images(
        self: &Rc<Self>,
        root: std::path::PathBuf,
        document: Option<std::path::PathBuf>,
        open: crate::content_widgets::OpenLink,
        context: Option<(String, String)>,
    ) -> crate::table_cell::RenderImages {
        let renderer = self.clone();
        Rc::new(move |container, content, compact| {
            renderer.append_table_images(
                container,
                content,
                &root,
                document.as_deref(),
                open.clone(),
                context.clone(),
                compact,
            );
        })
    }
    pub fn append_document_images(
        self: &Rc<Self>,
        container: &gtk::Box,
        content: &studio_markdown_native::RichText,
        root: &std::path::Path,
        document: Option<&std::path::Path>,
        open: crate::content_widgets::OpenLink,
        context: Option<(String, String)>,
    ) {
        for image in &content.images {
            let destination = match &image.source {
                studio_markdown_native::ImageSource::Reference(destination) => destination,
                studio_markdown_native::ImageSource::Document { page, image: index } => {
                    let Some((reader, revision)) = context.as_ref() else {
                        continue;
                    };
                    let source = crate::images::Source::Document {
                        reader: reader.clone(),
                        revision: revision.clone(),
                        page: *page,
                        image: *index,
                        original: false,
                    };
                    let open_source = source.clone();
                    let owner = Rc::downgrade(self);
                    let name = image.alternative.clone();
                    let preview = self.preview_source(
                        source,
                        &image.alternative,
                        Rc::new(move || {
                            if let Some(renderer) = owner.upgrade() {
                                if let Some(open) = renderer.on_open_embedded.borrow().as_ref() {
                                    open(open_source.clone(), name.clone());
                                }
                            }
                        }),
                    );
                    preview.set_widget_name("studio-embedded-image");
                    container.append(&preview);
                    continue;
                }
                studio_markdown_native::ImageSource::Embedded(data) => {
                    let source = crate::images::Source::Embedded(data.clone());
                    let open_source = source.clone();
                    let name = image.alternative.clone();
                    let owner = Rc::downgrade(self);
                    let preview = self.preview_source(
                        source,
                        &image.alternative,
                        Rc::new(move || {
                            if let Some(renderer) = owner.upgrade() {
                                if let Some(open) = renderer.on_open_embedded.borrow().as_ref() {
                                    open(open_source.clone(), name.clone());
                                }
                            }
                        }),
                    );
                    preview.set_widget_name("studio-embedded-image");
                    container.append(&preview);
                    continue;
                }
                studio_markdown_native::ImageSource::Unavailable(error) => {
                    let label = gtk::Label::builder()
                        .label(*error)
                        .wrap(true)
                        .xalign(0.0)
                        .build();
                    label.set_widget_name("studio-image-unavailable");
                    label.add_css_class("dim-label");
                    crate::i18n_ui::bind(&label, "label", error);
                    container.append(&label);
                    continue;
                }
            };
            if let Ok(crate::links::Target::External(url)) =
                crate::links::resolve(destination, root, document)
            {
                if crate::remote_images::reference(&url).is_ok() {
                    let source = crate::images::Source::Remote { url };
                    container.append(&self.embedded_preview(source, &image.alternative));
                }
                continue;
            }
            if root.as_os_str().is_empty() {
                continue;
            }
            if let Ok(crate::links::Target::Document {
                path, line: None, ..
            }) = crate::links::resolve(destination, root, document)
            {
                if crate::images::supports(&path) {
                    let destination = destination.clone();
                    let open = open.clone();
                    container.append(&self.preview(
                        &root.to_string_lossy(),
                        &path.to_string_lossy(),
                        &image.alternative,
                        Rc::new(move || open(&destination)),
                    ));
                }
            }
        }
    }
    pub fn new(sender: mpsc::Sender<Command>) -> Rc<Self> {
        Rc::new_cyclic(|owner| Self {
            owner: owner.clone(),
            previews: RefCell::default(),
            watched: RefCell::default(),
            visibility: {
                let owner = owner.clone();
                crate::viewport::FrameUpdate::new(Rc::new(move |clock| {
                    if let Some(owner) = owner.upgrade() {
                        owner.refresh_visible(clock);
                    }
                }))
            },
            on_open_embedded: RefCell::default(),
            #[cfg(debug_assertions)]
            maximum_callback_ms: Cell::new(0),
            #[cfg(debug_assertions)]
            maximum_visibility_ms: Cell::new(0),
            sender,
            next: Cell::new(0),
            pending: RefCell::default(),
            active: RefCell::default(),
            callbacks: RefCell::default(),
        })
    }
    pub fn append_table_images(
        self: &Rc<Self>,
        container: &gtk::Box,
        content: &studio_markdown_native::RichText,
        root: &std::path::Path,
        document: Option<&std::path::Path>,
        open: crate::content_widgets::OpenLink,
        context: Option<(String, String)>,
        compact: bool,
    ) {
        self.append_document_images(container, content, root, document, open, context);
        fn limit(renderer: &Renderer, widget: &gtk::Widget, height: i32, compact: bool) {
            if let Some(preview) = renderer
                .previews
                .borrow()
                .get(&(widget.as_ptr() as usize))
                .and_then(Weak::upgrade)
            {
                preview.table.set(true);
                preview.fixed_height.set(if compact { 120 } else { 0 });
            }
            if let Some(picture) = widget.downcast_ref::<crate::scaled_picture::ScaledPicture>() {
                picture.set_max_height(height);

                let owner = renderer.owner.clone();
                picture.on_allocate(Rc::new(move || {
                    if let Some(renderer) = owner.upgrade() {
                        renderer.queue_visible();
                    }
                }));
                if compact {
                    picture.set_size_request(-1, 120);
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                limit(renderer, &widget, height, compact);
            }
        }
        limit(
            self,
            container.upcast_ref(),
            if compact { 120 } else { 320 },
            compact,
        );
    }
    fn watch_adjustment(&self, adjustment: &gtk::Adjustment) {
        let key = adjustment.as_ptr() as usize;
        let mut watched = self.watched.borrow_mut();
        watched.retain(|_, a| a.upgrade().is_some());
        if watched.contains_key(&key) {
            return;
        }
        watched.insert(key, adjustment.downgrade());
        drop(watched);
        let owner = self.owner.clone();
        adjustment.connect_changed(move |_| {
            if let Some(owner) = owner.upgrade() {
                owner.queue_visible();
            }
        });
        let owner = self.owner.clone();
        adjustment.connect_value_changed(move |_| {
            if let Some(owner) = owner.upgrade() {
                owner.queue_visible();
            }
        });
    }
    fn queue_visible(&self) {
        for widget in self
            .previews
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .filter(|p| p.table.get())
            .filter_map(|p| p.widget.upgrade())
        {
            self.visibility.queue(&widget);
        }
    }
    fn refresh_visible(&self, clock: &gtk::gdk::FrameClock) {
        #[cfg(debug_assertions)]
        let started = std::time::Instant::now();
        let previews: Vec<_> = {
            let mut previews = self.previews.borrow_mut();
            previews.retain(|_, p| p.upgrade().is_some());
            previews
                .values()
                .filter_map(Weak::upgrade)
                .filter(|p| {
                    p.table.get()
                        && p.widget.upgrade().and_then(|w| w.frame_clock()).as_ref() == Some(clock)
                })
                .collect()
        };
        for preview in &previews {
            if !preview.in_view() {
                preview.unload();
            }
        }
        for preview in previews {
            let admitted = self.callbacks.borrow().len() < 32;
            if admitted
                && preview.in_view()
                && !preview.loading.get()
                && !preview.failed.get()
                && preview.picture.paintable().is_none()
            {
                preview.load();
            }
        }
        #[cfg(debug_assertions)]
        self.maximum_visibility_ms.set(
            self.maximum_visibility_ms
                .get()
                .max(started.elapsed().as_millis()),
        );
    }
    fn request_source(&self, source: crate::images::Source, callback: Completed) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        if self.callbacks.borrow().len() >= 32 {
            callback(Err(
                "Too many image previews are pending; retry after scrolling".into(),
            ));
            return id;
        }
        self.callbacks.borrow_mut().insert(id, callback);
        self.pending.borrow_mut().push_back(Job { id, source });
        self.flush();
        id
    }
    fn cancel(&self, id: u64) {
        self.callbacks.borrow_mut().remove(&id);
        self.pending.borrow_mut().retain(|job| job.id != id);
        if let Some(cancel) = self.active.borrow().get(&id) {
            cancel.notify_one();
        }
        // A running request keeps its admission slot until its reply arrives.
        // Offscreen/replaced widgets cannot increase concurrent decode work.
    }
    fn flush(&self) {
        while self.active.borrow().len() < 2 {
            let Some(job) = self.pending.borrow_mut().pop_front() else {
                break;
            };
            let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
            self.active.borrow_mut().insert(job.id, cancel.clone());
            if self
                .sender
                .try_send(Command::NativeImage {
                    id: job.id,
                    source: job.source,
                    cancel,
                })
                .is_err()
            {
                self.event(job.id, Err("Image request queue is busy; try again".into()));
            }
        }
    }
    pub fn event(&self, id: u64, result: Result<Frame, String>) {
        self.active.borrow_mut().remove(&id);
        let callback = self.callbacks.borrow_mut().remove(&id);
        if let Some(callback) = callback {
            #[cfg(debug_assertions)]
            let started = std::time::Instant::now();
            callback(result);
            #[cfg(debug_assertions)]
            self.maximum_callback_ms.set(
                self.maximum_callback_ms
                    .get()
                    .max(started.elapsed().as_millis()),
            );
        }
        self.flush();
        self.queue_visible();
    }
    pub fn preview(
        self: &Rc<Self>,
        root: &str,
        path: &str,
        alternative: &str,
        open: Rc<dyn Fn()>,
    ) -> gtk::Box {
        self.preview_source(
            crate::images::Source::File {
                root: root.into(),
                path: path.into(),
            },
            alternative,
            open,
        )
    }
    fn embedded_preview(self: &Rc<Self>, source: crate::images::Source, name: &str) -> gtk::Box {
        let open_source = source.clone();
        let title = name.to_owned();
        let owner = Rc::downgrade(self);
        let preview = self.preview_source(
            source,
            name,
            Rc::new(move || {
                if let Some(renderer) = owner.upgrade() {
                    if let Some(open) = renderer.on_open_embedded.borrow().as_ref() {
                        open(open_source.clone(), title.clone());
                    }
                }
            }),
        );
        preview.set_widget_name("studio-remote-image");
        preview
    }
    pub fn preview_source(
        self: &Rc<Self>,
        source: crate::images::Source,
        alternative: &str,
        open: Rc<dyn Fn()>,
    ) -> gtk::Box {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let picture = crate::scaled_picture::ScaledPicture::new(alternative);
        if alternative.is_empty() {
            i18n_ui::bind(&picture.animation_picture(), "alternative-text", "Image");
        }
        picture.set_size_request(-1, 160);
        widget.append(&picture);
        let caption = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let label = gtk::Label::builder()
            .label(if alternative.is_empty() {
                "Image"
            } else {
                alternative
            })
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .build();
        label.add_css_class("caption");
        if alternative.is_empty() {
            i18n_ui::bind(&label, "label", "Image");
        }
        caption.append(&label);
        caption.append(&gtk::Box::builder().hexpand(true).build());
        let retry = gtk::Button::new().ui_text("label", "Retry");
        retry.set_visible(false);
        caption.append(&retry);
        let motion = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .visible(false)
            .build();
        motion.add_css_class("flat");
        caption.append(&motion);
        let expand = gtk::Button::builder()
            .icon_name("view-fullscreen-symbolic")
            .tooltip_text("Open image viewer")
            .build()
            .ui_text("tooltip-text", "Open image viewer");
        expand.add_css_class("flat");
        expand.connect_clicked(move |button| {
            if button.is_mapped() {
                // The workspace viewer must be reachable after leaving a modal
                // full-cell reader. Its source lease is independent of that UI.
                if let Some(window) = button
                    .root()
                    .and_downcast::<gtk::Window>()
                    .filter(|w| w.widget_name() == "studio-selection-window")
                {
                    window.close();
                }
                open();
            }
        });
        caption.append(&expand);
        widget.append(&caption);
        let preview = Rc::new(Preview {
            renderer: Rc::downgrade(self),
            source,
            id: Cell::new(0),
            picture,
            label,
            retry,
            motion,
            player: RefCell::default(),
            widget: widget.downgrade(),
            table: Cell::new(false),
            fixed_height: Cell::new(0),
            loading: Cell::new(false),
            failed: Cell::new(false),
            viewports: RefCell::new(Vec::new()),
        });
        self.previews
            .borrow_mut()
            .retain(|_, p| p.upgrade().is_some());
        self.previews
            .borrow_mut()
            .insert(widget.as_ptr() as usize, Rc::downgrade(&preview));
        let current = preview.clone();
        widget.connect_map(move |_| {
            if current.table.get() {
                current.watch_viewports();
            } else {
                current.load();
            }
        });
        let current = preview.clone();
        widget.connect_unmap(move |_| current.unload());
        let weak = Rc::downgrade(&preview);
        preview.retry.connect_clicked(move |_| {
            if let Some(preview) = weak.upgrade() {
                preview.load();
            }
        });
        widget
    }
}
struct Preview {
    renderer: Weak<Renderer>,
    source: crate::images::Source,
    id: Cell<u64>,
    picture: crate::scaled_picture::ScaledPicture,
    label: gtk::Label,
    retry: gtk::Button,
    motion: gtk::Button,
    player: RefCell<Option<Rc<crate::image_playback::Player>>>,
    widget: glib::WeakRef<gtk::Box>,
    table: Cell<bool>,
    fixed_height: Cell<i32>,
    loading: Cell<bool>,
    failed: Cell<bool>,
    viewports: RefCell<Vec<glib::WeakRef<gtk::ScrolledWindow>>>,
}
impl Preview {
    fn watch_viewports(&self) {
        let Some(renderer) = self.renderer.upgrade() else {
            return;
        };
        self.viewports.borrow_mut().clear();
        if let Some(widget) = self.widget.upgrade() {
            let scopes = crate::viewport::scopes(&widget);
            for scroll in scopes.iter().filter_map(glib::WeakRef::upgrade) {
                renderer.watch_adjustment(&scroll.hadjustment());
                renderer.watch_adjustment(&scroll.vadjustment());
            }
            *self.viewports.borrow_mut() = scopes;
        }
        renderer.queue_visible();
    }
    fn in_view(&self) -> bool {
        self.widget
            .upgrade()
            .is_some_and(|widget| crate::viewport::intersects(&widget, &self.viewports.borrow()))
    }

    fn load(self: &Rc<Self>) {
        if let Some(player) = self.player.borrow().as_ref() {
            player.clear(false);
        }
        let Some(renderer) = self.renderer.upgrade() else {
            return;
        };
        renderer.cancel(self.id.get());
        self.loading.set(true);
        self.failed.set(false);
        self.retry.set_visible(false);
        self.label.set_visible(true);
        i18n_ui::bind(&self.label, "label", "Loading image…");
        let weak = Rc::downgrade(self);
        let id = renderer.request_source(
            self.source.clone(),
            Box::new(move |result| {
                let Some(preview) = weak.upgrade().filter(|preview| {
                    preview
                        .widget
                        .upgrade()
                        .is_some_and(|widget| widget.is_mapped())
                }) else {
                    return;
                };
                preview.loading.set(false);
                preview.id.set(0);
                match result {
                    Ok(mut frame) => {
                        let width = frame.original.0;
                        let timeline = frame.animation.take();
                        let limited = frame.animation_limited;
                        preview.picture.set_texture(&texture(frame));
                        if timeline.is_some() || limited || preview.player.borrow().is_some() {
                            let player = preview
                                .player
                                .borrow_mut()
                                .get_or_insert_with(|| {
                                    crate::image_playback::Player::new(
                                        &preview.picture.animation_picture(),
                                        &preview.motion,
                                    )
                                })
                                .clone();
                            player.set(timeline, limited);
                        }
                        preview.picture.set_logical_width(width);
                        preview.picture.set_size_request(
                            -1,
                            if preview.fixed_height.get() > 0 {
                                preview.fixed_height.get()
                            } else {
                                -1
                            },
                        );
                        preview.picture.set_visible(true);
                        preview.label.set_visible(false);
                    }
                    Err(error) => {
                        preview.failed.set(true);
                        preview.picture.set_visible(false);
                        preview.label.set_visible(true);
                        i18n_ui::literal(&preview.label, "label", &error);
                        preview.retry.set_visible(true);
                    }
                }
            }),
        );
        self.id.set(id);
    }
    fn unload(&self) {
        if let Some(player) = self.player.borrow().as_ref() {
            player.clear(false);
        }
        self.loading.set(false);
        self.failed.set(false);
        let id = self.id.replace(0);
        if id != 0 {
            if let Some(renderer) = self.renderer.upgrade() {
                renderer.cancel(id);
            }
        }
        if self.picture.paintable().is_some() {
            self.picture.clear_texture();
        }
    }
}
pub fn texture(frame: Frame) -> gdk::Texture {
    gdk::MemoryTexture::new(
        frame.width as i32,
        frame.height as i32,
        gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &glib::Bytes::from_owned(frame.pixels),
        frame.width as usize * 4,
    )
    .upcast()
}
pub fn rich(
    renderer: &Rc<Renderer>,
    content: &studio_markdown_native::RichText,
    root: &str,
    open: crate::content_widgets::OpenLink,
    reading: Rc<crate::content_widgets::ParagraphState>,
) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    container.append(&crate::content_widgets::formatted_at(
        content,
        open.clone(),
        reading,
    ));
    renderer.append_images(&container, content, std::path::Path::new(root), None, open);
    container.upcast()
}
pub fn chat(
    renderer: &Rc<Renderer>,
    content: crate::chat_ui::ImageContent<'_>,
    root: &str,
    open: crate::content_widgets::OpenLink,
) -> gtk::Widget {
    use crate::{chat_ui::ImageContent, conversation_images::Source};
    match content {
        ImageContent::Markdown(content, reading) => rich(renderer, content, root, open, reading),
        ImageContent::Cell(content, compact) => {
            let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
            renderer.append_table_images(
                &container,
                content,
                std::path::Path::new(root),
                None,
                open,
                None,
                compact,
            );
            container.upcast()
        }
        ImageContent::Attachment(image) => match &image.source {
            Source::Remote { url } => renderer
                .embedded_preview(
                    crate::images::Source::Remote { url: url.clone() },
                    &image.name,
                )
                .upcast(),
            Source::Inline { token } => {
                let token = token.clone();
                let uri = format!("studio-attachment:{token}");
                let preview = renderer.preview_source(
                    crate::images::Source::Inline {
                        token: token.clone(),
                    },
                    &image.name,
                    Rc::new(move || open(&uri)),
                );
                preview.set_widget_name(&format!("attachment:{token}"));
                preview.upcast()
            }
            Source::Local { path } => {
                let destination = path.clone();
                renderer
                    .preview(root, path, &image.name, Rc::new(move || open(&destination)))
                    .upcast()
            }
            Source::Unavailable { reason } => gtk::Label::builder()
                .label(format!("{}: {reason}", image.name))
                .wrap(true)
                .xalign(0.0)
                .build()
                .upcast(),
        },
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Location {
    source: crate::images::Source,
    title: String,
    subtitle: String,
    // Explicit interface keys; actual file names, paths and alt text stay literal.
    title_key: Option<&'static str>,
    subtitle_key: Option<&'static str>,
}
pub struct Viewer {
    pub widget: adw::ToolbarView,
    renderer: Rc<Renderer>,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    title: adw::WindowTitle,
    picture: gtk::Picture,
    scroll: gtk::ScrolledWindow,
    status: gtk::Label,
    fit: gtk::ToggleButton,
    actual: gtk::Button,
    plus: gtk::Button,
    minus: gtk::Button,
    copy: gtk::Button,
    open: gtk::Button,
    save: gtk::Button,
    motion: gtk::Button,
    player: RefCell<Option<Rc<crate::image_playback::Player>>>,
    open_picker: RefCell<Option<gio::Cancellable>>,
    save_picker: RefCell<Option<gio::Cancellable>>,
    identity: RefCell<Option<crate::images::Identity>>,
    feedback: Rc<crate::write_feedback::Queue>,
    scope: Cell<u64>,
    export_id: Cell<u64>,
    exports: RefCell<HashMap<u64, ImageExport>>,
    id: Cell<u64>,
    location: RefCell<Option<Location>>,
    zoom: Cell<f64>,
    size: Cell<(u32, u32)>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
}
struct ImageExport {
    scope: u64,
    title: String,
    file: gio::File,
}
impl Viewer {
    #[cfg(debug_assertions)]
    pub async fn smoke_inline(
        self: &Rc<Self>,
        token: &str,
        capture: impl Fn(&str),
    ) -> Result<serde_json::Value, String> {
        if self.location.borrow().as_ref().is_none_or(|location| {
            location.source
                != crate::images::Source::Inline {
                    token: token.into(),
                }
        }) {
            self.open_inline(token);
        }
        let start = std::time::Instant::now();
        while !self.copy.is_sensitive() {
            if start.elapsed() > std::time::Duration::from_secs(12) {
                return Err(format!("Inline viewer: {}", self.status.text()));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.size.get() != (2048, 683) {
            return Err(format!(
                "Inline image dimensions wrong: {:?}",
                self.size.get()
            ));
        }
        self.fit.set_active(false);
        self.plus.emit_clicked();
        if self.zoom.get() <= 1.0 {
            return Err("Inline viewer zoom failed".into());
        }
        self.fit.set_active(true);
        capture("history-viewer-controls.png");
        Ok(serde_json::json!({"preview":[2048,683],"zoomAndFit":true,"copyAvailable":true}))
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        root: &std::path::Path,
        capture: impl Fn(&str),
    ) -> Result<serde_json::Value, String> {
        async fn until(check: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !check() {
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err("Native image viewer timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            Ok(())
        }
        let ticks = Rc::new(Cell::new(0usize));
        let gap = Rc::new(Cell::new(0u128));
        let last = Rc::new(Cell::new(std::time::Instant::now()));
        let (count, maximum, previous) = (ticks.clone(), gap.clone(), last.clone());
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let now = std::time::Instant::now();
            maximum.set(
                maximum
                    .get()
                    .max(now.duration_since(previous.replace(now)).as_millis()),
            );
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        });
        let started = std::time::Instant::now();
        let loaded = until(|| self.picture.paintable().is_some()).await;
        let elapsed = started.elapsed().as_millis();
        if loaded.is_ok() {
            glib::timeout_future(std::time::Duration::from_millis(60)).await;
        }
        timer.remove();
        loaded?;
        if elapsed > 100 && ticks.get() == 0 {
            return Err("Image decoding blocked the GTK timer".into());
        }
        if self.size.get() != (2048, 683)
            || !self.fit.is_active()
            || !self.status.text().contains("3000 × 1000")
        {
            return Err("Image preview lost intrinsic dimensions or fit mode".into());
        }
        capture("image-fit.png");
        let steady_ticks = Rc::new(Cell::new(0usize));
        let steady_gap = Rc::new(Cell::new(0u128));
        let steady_last = Rc::new(Cell::new(std::time::Instant::now()));
        let (count, maximum, previous) = (
            steady_ticks.clone(),
            steady_gap.clone(),
            steady_last.clone(),
        );
        let steady_timer =
            glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                let now = std::time::Instant::now();
                maximum.set(
                    maximum
                        .get()
                        .max(now.duration_since(previous.replace(now)).as_millis()),
                );
                count.set(count.get() + 1);
                glib::ControlFlow::Continue
            });
        let steady_started = std::time::Instant::now();
        self.open(
            &root.to_string_lossy(),
            &root.join("fresh.png").to_string_lossy(),
        );
        let steady_loaded = until(|| self.picture.paintable().is_some()).await;
        let steady_elapsed = steady_started.elapsed().as_millis();
        if steady_loaded.is_ok() {
            glib::timeout_future(std::time::Duration::from_millis(60)).await;
        }
        steady_timer.remove();
        steady_loaded?;
        self.actual.emit_clicked();
        until(|| self.scroll.hadjustment().upper() > self.scroll.hadjustment().page_size()).await?;
        if self.fit.is_active() || self.picture.width_request() != 2048 {
            return Err("Image actual-size zoom did not resize the native canvas".into());
        }
        self.plus.emit_clicked();
        if self.zoom.get() != 1.25 {
            return Err("Image zoom-in control failed".into());
        }
        self.minus.emit_clicked();
        if (self.zoom.get() - 1.0).abs() > 0.001 {
            return Err("Image zoom-out control failed".into());
        }
        capture("image-zoom.png");
        self.copy.emit_clicked();
        let copied = self
            .copy
            .clipboard()
            .read_texture_future()
            .await
            .map_err(|error| error.to_string())?
            .ok_or("Image clipboard has no texture")?;
        if copied.width() != 2048 || copied.height() != 683 {
            return Err("Image clipboard did not contain the displayed pixels".into());
        }
        self.fit.set_active(true);
        self.open(
            &root.to_string_lossy(),
            &root.join("sample.png").to_string_lossy(),
        );
        let svg_ticks = Rc::new(Cell::new(0usize));
        let svg_gap = Rc::new(Cell::new(0u128));
        let svg_last = Rc::new(Cell::new(std::time::Instant::now()));
        let (count, maximum, previous) = (svg_ticks.clone(), svg_gap.clone(), svg_last);
        let svg_timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let now = std::time::Instant::now();
            maximum.set(
                maximum
                    .get()
                    .max(now.duration_since(previous.replace(now)).as_millis()),
            );
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        });
        let svg_started = std::time::Instant::now();
        self.open(
            &root.to_string_lossy(),
            &root.join("vector.svg").to_string_lossy(),
        );
        until(|| self.picture.paintable().is_some() && self.title.title() == "vector.svg").await?;
        let svg_elapsed = svg_started.elapsed().as_millis();
        svg_timer.remove();
        if svg_elapsed > 100 && svg_ticks.get() == 0 {
            return Err("SVG bitmap preparation blocked the GTK timer".into());
        }
        if self.size.get() != (2048, 1229) {
            return Err("A late raster image replaced the newer SVG".into());
        }
        let texture = self
            .picture
            .paintable()
            .and_downcast::<gdk::Texture>()
            .ok_or("SVG viewer has no native texture")?;
        let mut download = gdk::TextureDownloader::new(&texture);
        download.set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied);
        let (pixels, stride) = download.download_bytes();
        let offset = 870 * stride + 1740 * 4;
        if pixels.as_ref().get(offset..offset + 4) != Some(&[220, 40, 10, 255]) {
            return Err("SVG embedded bitmap did not reach the native viewer texture".into());
        }
        capture("image-svg.png");
        self.open(
            &root.to_string_lossy(),
            &root.join("broken.png").to_string_lossy(),
        );
        until(|| !self.status.text().contains("Opening image")).await?;
        if self.picture.paintable().is_some() || self.copy.is_sensitive() {
            return Err("Invalid image retained stale pixels".into());
        }
        self.open(
            &root.to_string_lossy(),
            &root.join("sample.png").to_string_lossy(),
        );
        until(|| self.picture.paintable().is_some()).await?;
        Ok(
            serde_json::json!({"ok":true,"fit":true,"zoom":true,"copy":true,"svg":true,"svgBitmapPixels":true,"svgReadyMs":svg_elapsed,"svgMainLoopTicks":svg_ticks.get(),"svgMaxMainLoopGapMs":svg_gap.get(),"lateScope":true,"invalidRecovery":true,"openMs":elapsed,"mainLoopTicks":ticks.get(),"maxMainLoopGapMs":gap.get(),"steadyOpenMs":steady_elapsed,"steadyMainLoopTicks":steady_ticks.get(),"steadyMaxMainLoopGapMs":steady_gap.get()}),
        )
    }
    pub fn new(
        parent: &adw::ApplicationWindow,
        renderer: Rc<Renderer>,
        feedback: Rc<crate::write_feedback::Queue>,
    ) -> Rc<Self> {
        let widget = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        let title = adw::WindowTitle::new("Image", "");
        i18n_ui::bind(&title, "title", "Image");
        header.set_title_widget(Some(&title));
        let open = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Open image")
            .build()
            .ui_text("tooltip-text", "Open image");
        header.pack_start(&open);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload image")
            .build()
            .ui_text("tooltip-text", "Reload image");
        let close = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text("Close image")
            .build()
            .ui_text("tooltip-text", "Close image");
        header.pack_end(&close);
        header.pack_end(&reload);
        widget.add_top_bar(&header);
        let controls = crate::adaptive::toolbar(6);
        controls.set_margin_start(12);
        controls.set_margin_end(12);
        controls.set_margin_bottom(8);
        let minus = gtk::Button::builder()
            .icon_name("zoom-out-symbolic")
            .tooltip_text("Zoom out")
            .build()
            .ui_text("tooltip-text", "Zoom out");
        let plus = gtk::Button::builder()
            .icon_name("zoom-in-symbolic")
            .tooltip_text("Zoom in")
            .build()
            .ui_text("tooltip-text", "Zoom in");
        let actual = gtk::Button::with_label("100%");
        i18n_ui::bind(
            &actual,
            "tooltip-text",
            "Show one preview pixel per logical pixel",
        );
        let fit = gtk::ToggleButton::new().ui_text("label", "Fit");
        fit.set_active(true);
        let copy = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy image preview")
            .build()
            .ui_text("tooltip-text", "Copy image preview");
        let save = gtk::Button::builder()
            .icon_name("document-save-as-symbolic")
            .tooltip_text("Save original image…")
            .build()
            .ui_text("tooltip-text", "Save original image…");
        for button in [&minus, &actual, &plus, &copy, &save] {
            controls.append(button);
        }
        controls.append(&fit);
        let motion = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .visible(false)
            .build();
        controls.append(&motion);
        widget.add_top_bar(&controls);
        let picture = gtk::Picture::builder()
            .can_shrink(true)
            .content_fit(gtk::ContentFit::Contain)
            .hexpand(true)
            .vexpand(true)
            .build();
        i18n_ui::bind(&picture, "alternative-text", "Image preview");
        let scroll = gtk::ScrolledWindow::builder()
            .child(&picture)
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .build();
        widget.set_content(Some(&scroll));
        let status = gtk::Label::builder()
            .label("Open an image")
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        status.add_css_class("caption");
        i18n_ui::bind(&status, "label", "Open an image");
        widget.add_bottom_bar(&status);
        let view = Rc::new(Self {
            widget,
            renderer,
            parent: parent.downgrade(),
            title,
            picture,
            scroll,
            status,
            fit,
            actual,
            plus,
            minus,
            copy,
            open: open.clone(),
            save,
            motion,
            player: RefCell::default(),
            open_picker: RefCell::new(None),
            save_picker: RefCell::new(None),
            identity: RefCell::new(None),
            feedback,
            scope: Cell::new(0),
            export_id: Cell::new(0),
            exports: RefCell::new(HashMap::new()),
            id: Cell::new(0),
            location: RefCell::default(),
            zoom: Cell::new(1.0),
            size: Cell::new((0, 0)),
            on_close: RefCell::default(),
            on_open: RefCell::default(),
        });
        let weak = Rc::downgrade(&view);
        open.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.choose();
            }
        });
        let weak = Rc::downgrade(&view);
        close.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.close();
                if let Some(callback) = view.on_close.borrow().as_ref() {
                    callback();
                }
            }
        });
        let weak = Rc::downgrade(&view);
        reload.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                let location = view.location.borrow().clone();
                if let Some(location) = location {
                    view.open_location(location);
                }
            }
        });
        let weak = Rc::downgrade(&view);
        view.fit.connect_toggled(move |_| {
            if let Some(view) = weak.upgrade() {
                view.layout();
            }
        });
        for (button, multiplier) in [(&view.plus, 1.25), (&view.minus, 0.8), (&view.actual, 0.0)] {
            let weak = Rc::downgrade(&view);
            button.connect_clicked(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.zoom.set(if multiplier == 0.0 {
                        1.0
                    } else {
                        (view.zoom.get() * multiplier).clamp(0.1, 4.0)
                    });
                    view.fit.set_active(false);
                    view.layout();
                }
            });
        }
        let weak = Rc::downgrade(&view);
        view.copy.connect_clicked(move |button| {
            if let Some(view) = weak.upgrade() {
                if let Some(texture) = view
                    .picture
                    .paintable()
                    .map(|p| p.current_image())
                    .and_downcast::<gdk::Texture>()
                {
                    button.clipboard().set_texture(&texture);
                }
            }
        });
        let weak = Rc::downgrade(&view);
        view.save.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.choose_save();
            }
        });
        let weak = Rc::downgrade(&view);
        view.widget.connect_unmap(move |_| {
            if let Some(view) = weak.upgrade() {
                view.cancel_pending_choices();
            }
        });
        view.enabled(false);
        view
    }
    fn enabled(&self, value: bool) {
        for button in [&self.plus, &self.minus, &self.actual, &self.copy] {
            button.set_sensitive(value);
        }
        self.fit.set_sensitive(value);
        self.update_save();
    }
    fn current_export(&self) -> bool {
        self.exports
            .borrow()
            .values()
            .any(|e| e.scope == self.scope.get())
    }
    fn update_save(&self) {
        self.save.set_sensitive(
            self.identity.borrow().is_some()
                && self.picture.paintable().is_some()
                && self.save_picker.borrow().is_none()
                && self.open_picker.borrow().is_none()
                && !self.current_export()
                && self.exports.borrow().len() < 2,
        );
    }
    fn layout(&self) {
        let fit = self.fit.is_active();
        let (width, height) = self.size.get();
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
        self.picture.set_size_request(
            if fit {
                -1
            } else {
                (width as f64 * self.zoom.get()).round() as i32
            },
            if fit {
                -1
            } else {
                (height as f64 * self.zoom.get()).round() as i32
            },
        );
        let policy = if fit {
            gtk::PolicyType::Never
        } else {
            gtk::PolicyType::Automatic
        };
        self.scroll.set_policy(policy, policy);
        self.actual
            .set_label(&format!("{:.0}%", self.zoom.get() * 100.0));
    }
    pub fn open(self: &Rc<Self>, root: &str, path: &str) {
        self.open_location(Location {
            source: crate::images::Source::File {
                root: root.into(),
                path: path.into(),
            },
            title: std::path::Path::new(path)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("Image")
                .into(),
            subtitle: path.into(),
            title_key: None,
            subtitle_key: None,
        });
    }
    pub fn open_inline(self: &Rc<Self>, token: &str) {
        self.open_location(Location {
            source: crate::images::Source::Inline {
                token: token.into(),
            },
            title: "Attached image".into(),
            subtitle: "Conversation image".into(),
            title_key: Some("Attached image"),
            subtitle_key: Some("Conversation image"),
        });
    }
    pub fn open_embedded(self: &Rc<Self>, mut source: crate::images::Source, title: String) {
        if let crate::images::Source::Document { original, .. } = &mut source {
            *original = true;
        }
        let title_key = title.is_empty().then_some("Embedded image");
        self.open_location(Location {
            source,
            title: if title.is_empty() {
                "Embedded image".into()
            } else {
                title
            },
            subtitle: "Markdown image".into(),
            title_key,
            subtitle_key: Some("Markdown image"),
        });
    }
    fn open_location(self: &Rc<Self>, location: Location) {
        self.cancel_pending_choices();
        self.scope.set(self.scope.get().wrapping_add(1));
        self.identity.borrow_mut().take();
        self.renderer.cancel(self.id.get());
        let new_source = self
            .location
            .borrow()
            .as_ref()
            .is_none_or(|old| old.source != location.source);
        if let Some(player) = self.player.borrow().as_ref() {
            player.clear(new_source);
        }
        self.location.replace(Some(location.clone()));
        self.picture.set_paintable(None::<&gdk::Paintable>);
        self.enabled(false);
        self.size.set((0, 0));
        self.zoom.set(1.0);
        self.fit.set_active(true);
        self.layout();
        for (property, literal, key) in [
            ("title", &location.title, location.title_key),
            ("subtitle", &location.subtitle, location.subtitle_key),
        ] {
            if let Some(key) = key {
                i18n_ui::bind(&self.title, property, key);
            } else {
                i18n_ui::literal(&self.title, property, literal);
            }
        }
        self.status_message("Opening image…", &[]);
        let weak = Rc::downgrade(self);
        let expected = location.clone();
        let id = self.renderer.request_source(
            location.source,
            Box::new(move |result| {
                let Some(view) = weak.upgrade() else { return };
                if view.location.borrow().as_ref() != Some(&expected) {
                    return;
                }
                match result {
                    Ok(mut frame) => {
                        if let Some(source) = frame.source.take() {
                            if let Some(location) = view.location.borrow_mut().as_mut() {
                                location.source = crate::images::Source::Embedded(source);
                            }
                        }
                        view.identity.replace(frame.identity.clone());
                        let original = frame.original;
                        let preview = (frame.width, frame.height);
                        view.size.set(preview);
                        let mut parts = vec![Part::message(
                            "{width} × {height} pixels",
                            &[
                                ("width", &original.0.to_string()),
                                ("height", &original.1.to_string()),
                            ],
                        )];
                        if original != preview {
                            parts.push(Part::message(
                                "Preview {width} × {height}",
                                &[
                                    ("width", &preview.0.to_string()),
                                    ("height", &preview.1.to_string()),
                                ],
                            ));
                        }
                        for property in ["label", "tooltip-text"] {
                            i18n_ui::bind_parts(&view.status, property, parts.clone(), " · ");
                        }
                        if let Some(key) = expected.title_key {
                            i18n_ui::bind(&view.picture, "alternative-text", key);
                        } else {
                            i18n_ui::literal(&view.picture, "alternative-text", &expected.title);
                        }
                        let timeline = frame.animation.take();
                        let limited = frame.animation_limited;
                        view.picture.set_paintable(Some(&texture(frame)));
                        if timeline.is_some() || limited || view.player.borrow().is_some() {
                            let player = view
                                .player
                                .borrow_mut()
                                .get_or_insert_with(|| {
                                    crate::image_playback::Player::new(&view.picture, &view.motion)
                                })
                                .clone();
                            player.set(timeline, limited);
                        }
                        view.enabled(true);
                        view.layout();
                    }
                    Err(error) => {
                        for property in ["label", "tooltip-text"] {
                            i18n_ui::literal(&view.status, property, &error);
                        }
                    }
                }
            }),
        );
        self.id.set(id);
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
    }
    fn status_message(&self, source: &str, variables: &[(&str, &str)]) {
        for property in ["label", "tooltip-text"] {
            i18n_ui::bind_format(&self.status, property, source, variables);
        }
    }
    pub fn has_document(&self) -> bool {
        self.location.borrow().is_some()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_ready(&self) -> bool {
        self.picture.paintable().is_some()
    }
    pub fn close(&self) {
        if let Some(player) = self.player.borrow().as_ref() {
            player.clear(true);
        }
        self.cancel_pending_choices();
        self.scope.set(self.scope.get().wrapping_add(1));
        self.identity.borrow_mut().take();
        self.renderer.cancel(self.id.get());
        self.location.borrow_mut().take();
        self.picture.set_paintable(None::<&gdk::Paintable>);
        self.enabled(false);
    }
    fn choose(self: &Rc<Self>) {
        if self.open_picker.borrow().is_some() {
            return;
        }
        self.cancel_pending_choices();
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let cancel = gio::Cancellable::new();
        self.open_picker.replace(Some(cancel.clone()));
        self.open.set_sensitive(false);
        self.update_save();
        let dialog = gtk::FileDialog::builder()
            .title("Open image")
            .accept_label("Open")
            .build()
            .ui_text("title", "Open image")
            .ui_text("accept-label", "Open");
        let filter = gtk::FileFilter::new();
        i18n_ui::bind(&filter, "name", "Images");
        for suffix in ["png", "jpg", "jpeg", "webp", "gif", "svg"] {
            filter.add_suffix(suffix);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let weak = Rc::downgrade(self);
        let request = cancel.clone();
        dialog.open(Some(&parent), Some(&cancel), move |result| {
            if let Some(view) = weak.upgrade() {
                if view.open_picker.borrow().as_ref() != Some(&request) {
                    return;
                }
                view.open_picker.borrow_mut().take();
                view.open.set_sensitive(true);
                view.update_save();
                match result {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            if let Some(root) = path.parent() {
                                view.open(&root.to_string_lossy(), &path.to_string_lossy());
                            }
                        } else {
                            view.status_message(
                                "Choose an image on this server's local filesystem",
                                &[],
                            );
                        }
                    }
                    Err(error) => {
                        if !choice_cancelled(&error) {
                            view.status_message(
                                "Unable to open image chooser: {error}",
                                &[("error", &error.to_string())],
                            );
                        }
                    }
                }
            }
        });
    }
    pub fn cancel_pending_choices(&self) {
        let open = self.open_picker.borrow_mut().take();
        let save = self.save_picker.borrow_mut().take();
        for cancel in [open, save].into_iter().flatten() {
            cancel.cancel();
        }
        self.open.set_sensitive(true);
        self.update_save();
    }
    fn choose_save(self: &Rc<Self>) {
        if !self.save.is_sensitive()
            || !self.widget.is_mapped()
            || self.save_picker.borrow().is_some()
        {
            return;
        }
        let (Some(location), Some(identity), Some(parent)) = (
            self.location.borrow().clone(),
            self.identity.borrow().clone(),
            self.parent.upgrade(),
        ) else {
            return;
        };
        let name = if matches!(
            location.source,
            crate::images::Source::Inline { .. } | crate::images::Source::Embedded(_)
        ) {
            format!("image.{}", identity.extension())
        } else {
            location.title.clone()
        };
        let chooser = gtk::FileDialog::builder()
            .title("Save original image")
            .initial_name(&name)
            .build()
            .ui_text("title", "Save original image");
        let cancel = gio::Cancellable::new();
        let request = cancel.clone();
        self.save_picker.replace(Some(cancel.clone()));
        self.update_save();
        let scope = self.scope.get();
        let weak = Rc::downgrade(self);
        chooser.save(Some(&parent), Some(&cancel), move |result| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            if view.save_picker.borrow().as_ref() != Some(&request) || view.scope.get() != scope {
                return;
            }
            view.save_picker.borrow_mut().take();
            view.update_save();
            match result {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        view.export_to(location, identity, path);
                    } else {
                        view.status_message(
                            "Choose a local destination for the original image",
                            &[],
                        );
                    }
                }
                Err(error) => {
                    if !choice_cancelled(&error) {
                        view.status_message(
                            "Unable to open image Save chooser: {error}",
                            &[("error", &error.to_string())],
                        );
                    }
                }
            }
        });
    }
    fn export_to(
        &self,
        location: Location,
        identity: crate::images::Identity,
        path: std::path::PathBuf,
    ) {
        if self.current_export() || self.exports.borrow().len() >= 2 {
            return;
        }
        let id = self.export_id.get().wrapping_add(1);
        self.export_id.set(id);
        self.exports.borrow_mut().insert(
            id,
            ImageExport {
                scope: self.scope.get(),
                title: location.title,
                file: gio::File::for_path(&path),
            },
        );
        self.status_message("Saving original image…", &[]);
        self.update_save();
        if self
            .renderer
            .sender
            .try_send(Command::ImageExport {
                id,
                source: location.source,
                identity,
                path,
            })
            .is_err()
        {
            self.exported(
                id,
                Err("Native workers are busy; retry saving the image".into()),
            );
        }
    }
    pub fn exported(&self, id: u64, result: Result<(), String>) {
        let Some(context) = self.exports.borrow_mut().remove(&id) else {
            return;
        };
        if context.scope == self.scope.get() && self.location.borrow().is_some() {
            match &result {
                Ok(()) => self.status_message("Original image saved", &[]),
                Err(error) => {
                    self.status_message("Image save failed: {error}", &[("error", error)])
                }
            }
        }
        if let Err(error) = result {
            self.feedback
                .export_failed(&context.title, &context.file, &error);
        }
        if self.exports.borrow().is_empty() {
            self.feedback.finished("image");
        }
        self.update_save();
    }
    pub fn allow_window_close(&self) -> bool {
        self.cancel_pending_choices();
        if self.exports.borrow().is_empty() {
            true
        } else {
            self.feedback
                .waiting("image", "Wait for the image save to finish before closing");
            false
        }
    }
}
fn choice_cancelled(error: &glib::Error) -> bool {
    error.matches(gtk::DialogError::Dismissed)
        || error.matches(gtk::DialogError::Cancelled)
        || error.matches(gio::IOErrorEnum::Cancelled)
}
impl Drop for Viewer {
    fn drop(&mut self) {
        for cancel in [
            self.open_picker.get_mut().take(),
            self.save_picker.get_mut().take(),
        ]
        .into_iter()
        .flatten()
        {
            cancel.cancel();
        }
    }
}
