//! Real shared image viewer controls, paintables and clipboard on owned Wayland.
use super::*;
impl Viewer {
    pub async fn smoke_animation(
        self: &Rc<Self>,
        project: &std::path::Path,
        output: &std::path::Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            crate::acceptance_wait::until(step, test).await
        }
        fn player(view: &Viewer) -> serde_json::Value {
            view.player.borrow().as_ref().unwrap().state()
        }
        fn pixel(texture: &gdk::Texture) -> [u8; 4] {
            let mut download = gdk::TextureDownloader::new(texture);
            download.set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied);
            let (bytes, _) = download.download_bytes();
            bytes[..4].try_into().unwrap()
        }
        fn current(view: &Viewer) -> Result<gdk::Texture, String> {
            view.picture
                .paintable()
                .map(|p| p.current_image())
                .and_downcast::<gdk::Texture>()
                .ok_or("Animation has no immutable current image".into())
        }
        let settings = gtk::Settings::default().ok_or("No native animation settings")?;
        let previous = settings.is_gtk_enable_animations();
        settings.set_gtk_enable_animations(false);
        self.open(
            &project.to_string_lossy(),
            &project.join("animated.gif").to_string_lossy(),
        );
        until("GIF timeline and reduced-motion control", || {
            self.title.title() == "animated.gif"
                && self
                    .player
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.state()["frames"] == 2 && p.state()["visible"] == true)
        })
        .await?;
        if player(self)["wanted"] != false
            || player(self)["timer"] != false
            || pixel(&current(self)?) != [255, 0, 0, 255]
        {
            return Err("Reduced motion did not keep the first GIF frame".into());
        }
        let paintable = self.picture.paintable().unwrap();
        if !paintable.flags().contains(gdk::PaintableFlags::STATIC_SIZE) {
            return Err("Animation paintable does not preserve its size".into());
        }
        glib::timeout_future(std::time::Duration::from_millis(150)).await;
        if player(self)["index"] != 0 {
            return Err("Reduced-motion GIF advanced without an explicit Play action".into());
        }
        self.motion.emit_clicked();
        until("second GIF frame", || player(self)["index"] == 1).await?;
        self.motion.emit_clicked();
        if player(self)["timer"] != false
            || self.picture.paintable().as_ref() != Some(&paintable)
            || pixel(&current(self)?) != [0, 0, 255, 255]
        {
            return Err("Pause changed the paintable or lost the current GIF pixels".into());
        }
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if self.motion.tooltip_text().as_deref() != Some("播放动画") {
            return Err("Animation control did not update its native translation".into());
        }
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.copy.emit_clicked();
        let copied = self
            .copy
            .clipboard()
            .read_texture_future()
            .await
            .map_err(|e| e.to_string())?
            .ok_or("No copied animation frame")?;
        if pixel(&copied) != [0, 0, 255, 255] {
            return Err("Copy did not snapshot the paused current GIF frame".into());
        }
        let window = self.parent.upgrade().ok_or("Image test window closed")?;
        crate::window_capture::capture(&window, &output.join("animation-gif-paused.png")).await?;
        self.motion.emit_clicked();
        until("finite GIF completion", || player(self)["finished"] == true).await?;
        if player(self)["completed"] != 3
            || player(self)["index"] != 1
            || player(self)["timer"] != false
        {
            return Err(format!(
                "GIF play count did not finish correctly: {}",
                player(self)
            ));
        }
        crate::window_capture::capture(&window, &output.join("animation-gif-complete.png")).await?;
        self.motion.emit_clicked();
        self.motion.emit_clicked();
        if player(self)["index"] != 0
            || player(self)["timer"] != false
            || pixel(&current(self)?) != [255, 0, 0, 255]
        {
            return Err("Replay did not restart the GIF with working Pause".into());
        }
        let old = self.picture.paintable().unwrap();
        settings.set_gtk_enable_animations(true);
        for extension in ["png", "webp"] {
            let name = format!("animated.{extension}");
            self.open(
                &project.to_string_lossy(),
                &project.join(&name).to_string_lossy(),
            );
            until("new animation file", || {
                self.title.title() == name
                    && self
                        .player
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.state()["frames"] == 2)
            })
            .await?;
            until("automatic animation completion", || {
                player(self)["finished"] == true
            })
            .await?;
            if player(self)["completed"] != 2 || pixel(&current(self)?) != [0, 0, 128, 128] {
                return Err(format!(
                    "{extension} animation pixels or total plays changed"
                ));
            }
            crate::window_capture::capture(
                &window,
                &output.join(format!("animation-{extension}-complete.png")),
            )
            .await?;
        }
        if self.picture.paintable().as_ref() == Some(&old) {
            return Err("Replacing a file retained an old animation paintable".into());
        }
        self.open(
            &project.to_string_lossy(),
            &project.join("infinite.gif").to_string_lossy(),
        );
        until("infinite GIF playback", || {
            self.title.title() == "infinite.gif"
                && player(self)["frames"] == 2
                && player(self)["index"] == 1
                && player(self)["timer"] == true
        })
        .await?;
        window.set_visible(false);
        until("hidden window suspends animation", || {
            player(self)["visible"] == false && player(self)["timer"] == false
        })
        .await?;
        let paused_index = player(self)["index"].clone();
        glib::timeout_future(std::time::Duration::from_millis(160)).await;
        if player(self)["index"] != paused_index || player(self)["finished"] != false {
            return Err("Hidden infinite GIF continued advancing".into());
        }
        window.present();
        until("presented window resumes animation", || {
            player(self)["visible"] == true
                && player(self)["timer"] == true
                && player(self)["index"] != paused_index
        })
        .await?;
        self.open(
            &project.to_string_lossy(),
            &project.join("over-budget.gif").to_string_lossy(),
        );
        until("over-budget animation retains a still preview", || {
            self.title.title() == "over-budget.gif"
                && player(self)["limited"] == true
                && self.picture.paintable().is_some()
        })
        .await?;
        if player(self)["frames"] != 0
            || player(self)["timer"] != false
            || !self.motion.is_visible()
            || self.motion.is_sensitive()
            || pixel(&current(self)?) != [255, 0, 0, 255]
        {
            return Err(
                "Over-budget animation lost its readable still preview or explanation".into(),
            );
        }
        crate::window_capture::capture(&window, &output.join("animation-limited.png")).await?;
        self.close();
        if player(self)["frames"] != 0
            || player(self)["timer"] != false
            || self.picture.paintable().is_some()
        {
            return Err("Closing the viewer retained animation frames or a timer".into());
        }
        settings.set_gtk_enable_animations(previous);
        Ok(
            serde_json::json!({"ok":true,"formats":["gif","png","webp"],"reducedMotion":true,"explicitPlayPause":true,"finitePlays":true,"infinitePlayback":true,"hiddenWindowSuspends":true,"nativeTranslation":true,"limitedStillPreview":true,"replay":true,"currentFrameCopy":true,"stablePaintable":true,"fileReplacement":true,"closeReleasesFrames":true}),
        )
    }
}

impl Renderer {
    fn animation_preview(&self, root: &gtk::Widget) -> Option<Rc<Preview>> {
        self.previews
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .find(|preview| {
                preview
                    .widget
                    .upgrade()
                    .is_some_and(|widget| widget.is_mapped() && widget.is_ancestor(root))
                    && preview
                        .player
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.state()["frames"] == 2)
            })
    }
    pub async fn smoke_snapshot_animation(
        self: &Rc<Self>,
        reader: &Rc<crate::reader::Reader>,
        viewer: &Rc<Viewer>,
        project: &std::path::Path,
        output: &std::path::Path,
    ) -> Result<serde_json::Value, String> {
        let root = reader.widget.upcast_ref::<gtk::Widget>();
        crate::acceptance_wait::until("projected Markdown animation", || {
            reader.is_ready() && self.animation_preview(root).is_some()
        })
        .await?;
        let old = self.animation_preview(root).unwrap();
        if !matches!(old.source, crate::images::Source::Document { .. }) {
            return Err("Large embedded animation did not use the snapshot route".into());
        }
        let old_player = old.player.borrow().as_ref().unwrap().clone();
        reader.show_source();
        crate::acceptance_wait::until("Source releases animated Markdown frames", || {
            old_player.state()["frames"] == 0
                && old_player.state()["timer"] == false
                && old.picture.paintable().is_none()
        })
        .await?;
        crate::acceptance_wait::until("source window ready before mode switch", || {
            reader.is_ready()
        })
        .await?;
        reader.show_preview();
        crate::acceptance_wait::until("Preview restores embedded animation", || {
            self.animation_preview(root).is_some()
        })
        .await?;
        let preview = self.animation_preview(root).unwrap();
        let expand = preview
            .widget
            .upgrade()
            .and_then(|w| w.last_child())
            .and_then(|w| w.last_child())
            .and_downcast::<gtk::Button>()
            .ok_or("No embedded image expand button")?;
        expand.emit_clicked();
        crate::acceptance_wait::until("expanded animation owns its original", || {
            viewer
                .location
                .borrow()
                .as_ref()
                .is_some_and(|l| matches!(&l.source, crate::images::Source::Embedded(_)))
                && viewer
                    .player
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.state()["frames"] == 2)
        })
        .await?;
        let expected_path = project.join("animated.gif");
        let expected = gio::spawn_blocking(move || std::fs::read(expected_path))
            .await
            .map_err(|_| "GIF fixture reader stopped")?
            .map_err(|e| e.to_string())?;
        let location = viewer
            .location
            .borrow()
            .clone()
            .ok_or("Expanded image lost its location")?;
        let crate::images::Source::Embedded(ref original) = location.source else {
            return Err("Expanded image retained a snapshot lease".into());
        };
        if original.bytes.as_ref() != expected.as_slice() {
            return Err("Animated image original bytes changed".into());
        }
        let window = viewer.parent.upgrade().ok_or("Image test window closed")?;
        crate::window_capture::capture(&window, &output.join("animation-snapshot-expanded.png"))
            .await?;
        reader.open_project_path(project, &project.join("nested/images.md"), None, None);
        let saved = output.join("saved-snapshot-animation.gif");
        viewer.export_to(
            location,
            viewer
                .identity
                .borrow()
                .clone()
                .ok_or("No original image identity")?,
            saved.clone(),
        );
        crate::acceptance_wait::until("save animated snapshot original", || {
            viewer.exports.borrow().is_empty()
        })
        .await?;
        let actual = gio::spawn_blocking(move || std::fs::read(saved))
            .await
            .map_err(|_| "Saved GIF reader stopped")?
            .map_err(|e| e.to_string())?;
        if actual != expected {
            return Err(
                "Original animation save lost encoded frames after replacing the Markdown snapshot"
                    .into(),
            );
        }
        viewer.close();
        Ok(
            serde_json::json!({"ok":true,"snapshotRoute":true,"sourceReleasesFrames":true,"previewRestores":true,"expandOwnsOriginal":true,"saveAfterSnapshotReplacement":true}),
        )
    }

    pub async fn smoke_chat_animation(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        project: &std::path::Path,
        documents: mpsc::Sender<crate::document::Message>,
        output: &std::path::Path,
    ) -> Result<serde_json::Value, String> {
        let root = project.to_string_lossy().into_owned();
        let renderer = self.clone();
        let media: crate::chat_ui::ImageAction =
            Rc::new(move |_, content, open| chat(&renderer, content, &root, open));
        let text=format!("## Animated conversation\n\n![GIF](infinite.gif)\n\n![APNG](animated.png)\n\n![WebP](animated.webp)\n\n{}", (0..80).map(|n| format!("Context paragraph {n}.\n\n")).collect::<String>());
        let parse = text.clone();
        let content = gio::spawn_blocking(move || crate::chat_content::Content::parse(&parse))
            .await
            .map_err(|_| "Chat animation parser stopped")??;
        let item = std::sync::Arc::new(crate::model::Item {
            output: None,
            details: None,
            status: None,
            truncated: false,
            images: Vec::new(),
            origin: None,
            id: "private-animated-response".into(),
            turn_id: "private-animated-turn".into(),
            role: "Assistant".into(),
            body: text,
            markup: None,
            rich: Some(content),
        });
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        crate::chat_ui::reconcile(&model, &[item]);
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(model.clone()))),
            Some(crate::chat_ui::factory(
                Rc::new(|_| {}),
                crate::content_widgets::Styles::new(),
                crate::chat_diagrams::Renderer::new_with_key(documents, 9901),
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
            .title("Animated conversation")
            .content_width(780)
            .content_height(640)
            .child(&toolbar)
            .build();
        dialog.present(Some(parent));
        crate::window_capture::ready(&toolbar).await?;
        let root = list.upcast_ref::<gtk::Widget>();
        crate::acceptance_wait::until("conversation animation", || {
            self.animation_preview(root).is_some()
        })
        .await?;
        let preview=self.previews.borrow().values().filter_map(Weak::upgrade).find(|p| p.widget.upgrade().is_some_and(|w|w.is_mapped() && w.is_ancestor(root)) && matches!(&p.source,crate::images::Source::File{path,..} if path.ends_with("infinite.gif"))).ok_or("Chat GIF preview absent")?;
        crate::acceptance_wait::until("chat GIF frames", || {
            preview
                .player
                .borrow()
                .as_ref()
                .is_some_and(|p| p.state()["frames"] == 2)
        })
        .await?;
        let player = preview.player.borrow().as_ref().unwrap().clone();
        let settings = gtk::Settings::default().ok_or("No native animation settings")?;
        let previous = settings.is_gtk_enable_animations();
        settings.set_gtk_enable_animations(true);
        crate::acceptance_wait::until("chat GIF playing", || {
            player.state()["timer"] == true && player.state()["index"] == 1
        })
        .await?;
        crate::window_capture::capture(parent, &output.join("animation-chat.png")).await?;
        list.scroll_to(model.n_items() - 1, gtk::ListScrollFlags::NONE, None);
        crate::acceptance_wait::until("offscreen chat animation suspended", || {
            player.state()["timer"] == false
                && (player.state()["frames"] == 0 || player.state()["visible"] == false)
        })
        .await?;
        let index = player.state()["index"].clone();
        glib::timeout_future(std::time::Duration::from_millis(160)).await;
        if player.state()["index"] != index {
            return Err("Offscreen chat image continued animating".into());
        }
        list.scroll_to(0, gtk::ListScrollFlags::NONE, None);
        crate::acceptance_wait::until("chat animation resumes after scroll", || {
            self.animation_preview(root).is_some_and(|p| {
                p.player
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.state()["timer"] == true)
            })
        })
        .await?;
        settings.set_gtk_enable_animations(previous);
        dialog.force_close();
        crate::window_capture::next_frame(parent).await?;
        if loaded_count(root, false) != 0
            || player.state()["frames"] != 0
            || player.state()["timer"] != false
        {
            return Err("Closing animated chat retained frames or timers".into());
        }
        Ok(
            serde_json::json!({"ok":true,"nativeFactory":true,"offscreenSuspends":true,"scrollRestores":true,"closeReleasesFrames":true}),
        )
    }
}
