use super::*;
use crate::attachments::{self, Source};

impl Ui {
    pub(super) fn composer_key(&self) -> Option<String> {
        let state = self.state.borrow();
        state
            .selected
            .as_ref()
            .map(|thread| format!("{}:{}", state.backend, thread.id))
    }
    pub(super) fn connect_attachments(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.image_button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.choose_images();
            }
        });
        let remove = gio::SimpleAction::new(
            "remove-image",
            Some(&<(String, String)>::static_variant_type()),
        );
        let weak = Rc::downgrade(self);
        remove.connect_activate(move |_, parameter| {
            if let (Some(ui), Some((key, id))) = (
                weak.upgrade(),
                parameter.and_then(|value| value.get::<(String, String)>()),
            ) {
                ui.state.borrow_mut().images.remove(&key, &[id]);
                ui.update_composer();
            }
        });
        self.window.add_action(&remove);
        let weak = Rc::downgrade(self);
        self.composer.connect_paste_clipboard(move |text| {
            let clipboard = text.clipboard();
            let formats = clipboard.formats();
            // Keep ordinary text/IME paste, including copied file names, native.
            if !formats.contain_mime_type("text/plain")
                && attachments::MIMES
                    .iter()
                    .any(|mime| formats.contain_mime_type(mime))
            {
                text.stop_signal_emission_by_name("paste-clipboard");
                if let Some(ui) = weak.upgrade() {
                    ui.paste_image();
                }
            }
        });
        let drop = gtk::DropTarget::new(
            gtk::gdk::FileList::static_type(),
            gtk::gdk::DragAction::COPY,
        );
        let weak = Rc::downgrade(self);
        drop.connect_drop(move |_, value, _, _| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let Ok(files) = value.get::<gtk::gdk::FileList>() else {
                return false;
            };
            let Some(key) = ui.composer_key() else {
                return false;
            };
            let files = files.files();
            if files.len() > attachments::MAX_IMAGES {
                ui.error("Choose at most four images");
                return false;
            }
            let paths = files
                .iter()
                .map(|file| {
                    file.path()
                        .ok_or_else(|| "Attach local files through the file chooser".to_owned())
                })
                .collect::<Result<Vec<_>, _>>();
            match paths.and_then(|paths| ui.attach_files(&key, paths)) {
                Ok(()) => true,
                Err(error) => {
                    ui.error(&error);
                    false
                }
            }
        });
        self.composer.add_controller(drop);
    }
    fn reserve_images(&self, key: &str) -> Result<u64, String> {
        let mut state = self.state.borrow_mut();
        if state.archived || state.session_mutation || state.selected.is_none() {
            return Err("Select an editable conversation first".into());
        }
        let serial = state.images.begin(key)?;
        drop(state);
        self.update_composer();
        Ok(serial)
    }
    fn send_images(&self, key: &str, serial: u64, source: Source) {
        if !self.command(Command::Images {
            key: key.into(),
            serial,
            source,
        }) {
            let _ = self.state.borrow_mut().images.finish(
                key,
                serial,
                Err("Attachment worker was not started".into()),
            );
            self.update_composer();
        }
    }
    pub(super) fn attach_files(
        &self,
        key: &str,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        if paths.len() > attachments::MAX_IMAGES {
            return Err("Choose at most four images".into());
        }
        let serial = self.reserve_images(key)?;
        self.send_images(key, serial, Source::Files(paths));
        Ok(())
    }
    fn choose_images(self: &Rc<Self>) {
        let Some(key) = self.composer_key() else {
            return;
        };
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images (PNG, JPEG, WebP, GIF)"));
        for mime in attachments::MIMES {
            filter.add_mime_type(mime);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Attach images")
            .accept_label("Attach")
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let parent = self.window.clone();
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let files = match dialog.open_multiple_future(Some(&parent)).await {
                Ok(files) => files,
                Err(error)
                    if error.matches(gtk::DialogError::Dismissed)
                        || error.matches(gtk::DialogError::Cancelled) =>
                {
                    return
                }
                Err(error) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.error(&format!("Cannot select images: {error}"));
                    }
                    return;
                }
            };
            if let Some(ui) = weak.upgrade() {
                if files.n_items() > attachments::MAX_IMAGES as u32 {
                    ui.error("Choose at most four images");
                    return;
                }
                let result = (0..files.n_items())
                    .map(|index| {
                        files
                            .item(index)
                            .and_downcast::<gio::File>()
                            .and_then(|file| file.path())
                            .ok_or_else(|| "Choose local image files".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(|paths| ui.attach_files(&key, paths));
                if let Err(error) = result {
                    ui.error(&error);
                }
            }
        });
    }
    pub(super) fn paste_image(self: &Rc<Self>) {
        let Some(key) = self.composer_key() else {
            return;
        };
        let serial = match self.reserve_images(&key) {
            Ok(serial) => serial,
            Err(error) => {
                self.error(&error);
                return;
            }
        };
        let clipboard = self.composer.clipboard();
        let cancellable = gio::Cancellable::new();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        // Start GTK's read in the paste signal's dispatch, while the selected
        // offer is still current. read_future inside spawn_local starts only
        // on a later poll; another copy/focus change can replace it by then.
        clipboard.read_async(
            attachments::MIMES,
            glib::Priority::DEFAULT,
            Some(&cancellable),
            move |result| {
                if let Err(Ok((stream, _))) = sender.send(result) {
                    stream.close_async(glib::Priority::DEFAULT, gio::Cancellable::NONE, |_| {});
                }
            },
        );
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let read = async {
                let (stream, _) = receiver
                    .await
                    .map_err(|_| "Clipboard image read was interrupted".to_owned())?
                    .map_err(|_| "No supported image is available on the clipboard".to_owned())?;
                let result = async {
                    let mut bytes = Vec::new();
                    loop {
                        let chunk = stream
                            .read_bytes_future(
                                (attachments::MAX_FILE + 1 - bytes.len()).min(64 * 1024),
                                glib::Priority::DEFAULT,
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                        if chunk.is_empty() {
                            break;
                        }
                        bytes.extend_from_slice(&chunk);
                        if bytes.len() > attachments::MAX_FILE {
                            return Err("Clipboard image exceeds 10 MiB".to_owned());
                        }
                    }
                    Ok(bytes)
                }
                .await;
                let _ = stream.close_future(glib::Priority::DEFAULT).await;
                result
            };
            let result = match futures_util::future::select(
                Box::pin(read),
                Box::pin(glib::timeout_future(Duration::from_secs(15))),
            )
            .await
            {
                futures_util::future::Either::Left((result, _)) => result,
                futures_util::future::Either::Right(_) => {
                    Err("Clipboard image timed out; try copying it again".into())
                }
            };
            cancellable.cancel();
            if let Some(ui) = weak.upgrade() {
                match result {
                    Ok(bytes) => ui.send_images(&key, serial, Source::Clipboard(bytes)),
                    Err(error) => {
                        let _ =
                            ui.state
                                .borrow_mut()
                                .images
                                .finish(&key, serial, Err(error.clone()));
                        ui.error(&error);
                        ui.update_composer();
                    }
                }
            }
        });
    }
    pub(super) fn render_attachments(&self) {
        let key = self.composer_key().unwrap_or_default();
        let state = self.state.borrow();
        let loading = state.images.pending.contains_key(&key);
        self.image_button.set_sensitive(
            !key.is_empty() && !state.archived && !state.session_mutation && !loading,
        );
        let stamp = (key.clone(), state.images.revision);
        if *self.image_stamp.borrow() == stamp {
            return;
        }
        self.image_stamp.replace(stamp);
        while let Some(child) = self.image_chips.first_child() {
            self.image_chips.remove(&child);
        }
        let images = state.images.images(&key);
        self.image_chips.set_visible(loading || !images.is_empty());
        for image in images {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            row.add_css_class("card");
            row.set_margin_start(4);
            row.set_margin_end(4);
            row.append(&gtk::Image::from_icon_name("image-x-generic-symbolic"));
            let label = gtk::Label::builder()
                .label(&image.name)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .max_width_chars(24)
                .tooltip_text(format!(
                    "{} · {:.1} KiB",
                    image.name,
                    image.size as f64 / 1024.0
                ))
                .build();
            row.append(&label);
            let size = gtk::Label::new(Some(&if image.size >= 1024 * 1024 {
                format!("{:.1} MiB", image.size as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.0} KiB", image.size as f64 / 1024.0)
            }));
            size.add_css_class("caption");
            size.add_css_class("dim-label");
            row.append(&size);
            let remove = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text(format!("Remove {}", image.name))
                .action_name("win.remove-image")
                .action_target(&(key.clone(), image.id.clone()).to_variant())
                .build();
            remove.add_css_class("flat");
            row.append(&remove);
            self.image_chips.insert(&row, -1);
        }
        if loading {
            let spinner = gtk::Spinner::builder()
                .spinning(true)
                .tooltip_text("Preparing images…")
                .margin_start(8)
                .margin_end(8)
                .build();
            self.image_chips.insert(&spinner, -1);
        }
    }
}

#[cfg(debug_assertions)]
async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
    let started = std::time::Instant::now();
    while !ready() {
        if started.elapsed() > Duration::from_secs(6) {
            return Err("Native attachment operation timed out".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}

#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    use base64::Engine;
    until(|| {
        ui.state.borrow().ready
            && !ui.state.borrow().loading
            && !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    let key = ui.composer_key().ok_or("No image test conversation")?;
    let draft = ui.composer_text();
    let mut bytes=base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=").map_err(|error|error.to_string())?;
    // Valid PNG followed by padding: exercise a >1 MiB image transport without
    // generating or decoding a huge raster in the acceptance driver.
    bytes.resize(1100 * 1024, 0);
    let path = directory.join("project/native-image-中文.png");
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    ui.attach_files(&key, vec![path.clone()])?;
    until(|| !ui.state.borrow().images.pending.contains_key(&key)).await?;
    if ui.state.borrow().images.images(&key).len() != 1 {
        return Err("Selected image was not attached".into());
    }

    let clipboard = ui.composer.clipboard();
    let previous = clipboard.content();
    let provider =
        gtk::gdk::ContentProvider::for_bytes("image/png", &glib::Bytes::from_owned(bytes));
    clipboard
        .set_content(Some(&provider))
        .map_err(|error| error.to_string())?;
    // Exercise GTK's local clipboard provider in the same main-loop dispatch.
    // An unattended window may lack Wayland activation/input serials; waiting
    // for a compositor round trip can legitimately reject its exported offer.
    // Cross-application paste still requires focused/manual acceptance.
    if !clipboard.is_local() || !clipboard.formats().contain_mime_type("image/png") {
        return Err("The GTK clipboard did not accept the local image provider".into());
    }
    ui.composer.emit_by_name::<()>("paste-clipboard", &[]);
    // A new copy between paste initiation and the next main-loop poll must not
    // change which image is attached. This makes the former timing race explicit.
    let replacement = gtk::gdk::ContentProvider::for_bytes(
        "text/plain",
        &glib::Bytes::from_static(b"replacement clipboard test"),
    );
    clipboard
        .set_content(Some(&replacement))
        .map_err(|error| error.to_string())?;
    let pasted = until(|| !ui.state.borrow().images.pending.contains_key(&key)).await;
    let formats = clipboard.formats().mime_types();
    if clipboard.content().as_ref() == Some(&replacement) {
        let _ = clipboard.set_content(previous.as_ref());
    }
    pasted?;
    if ui.state.borrow().images.images(&key).len() != 2 {
        return Err(format!(
            "Native clipboard image paste was not attached: status={}, formats={formats:?}",
            ui.status.text()
        ));
    }

    let first = ui
        .image_chips
        .first_child()
        .and_then(|child| child.first_child())
        .ok_or("Image chip was not rendered")?;
    let remove = first
        .last_child()
        .and_downcast::<gtk::Button>()
        .ok_or("Image remove button missing")?;
    remove.emit_clicked();
    if ui.state.borrow().images.images(&key).len() != 1 {
        return Err("Image remove action did not update the composer".into());
    }

    let controllers = ui.composer.observe_controllers();
    let target = (0..controllers.n_items())
        .find_map(|index| controllers.item(index).and_downcast::<gtk::DropTarget>())
        .ok_or("Native file drop target missing")?;
    let list = gtk::gdk::FileList::from_array(&[gio::File::for_path(&path)]);
    if !target.emit_by_name::<bool>(
        "drop",
        &[&glib::BoxedValue(list.to_value()), &0.0f64, &0.0f64],
    ) {
        return Err("Native file drop was rejected".into());
    }
    until(|| !ui.state.borrow().images.pending.contains_key(&key)).await?;
    if ui.state.borrow().images.images(&key).len() != 2 {
        return Err("Dropped file did not reach the image worker".into());
    }
    ui.composer.buffer().set_text("native-image-reject");
    ui.submit();
    until(|| !ui.state.borrow().sending && !ui.state.borrow().loading && ui.send.is_sensitive())
        .await?;
    if ui.composer_text() != "native-image-reject"
        || ui.state.borrow().images.images(&key).len() != 2
    {
        return Err("Rejected image send discarded the draft or attachments".into());
    }

    let original = ui
        .state
        .borrow()
        .images
        .images(&key)
        .iter()
        .map(|image| image.id.clone())
        .collect::<Vec<_>>();
    ui.composer.buffer().set_text("native-image-late");
    ui.submit();
    ui.composer
        .buffer()
        .set_text("Newer attachment draft 中文😀");
    ui.attach_files(&key, vec![path.clone()])?;
    until(|| !ui.state.borrow().images.pending.contains_key(&key)).await?;
    let newer = ui
        .state
        .borrow()
        .images
        .images(&key)
        .into_iter()
        .find(|image| !original.contains(&image.id))
        .ok_or("Newer attachment missing")?
        .id
        .clone();
    let selected = ui.state.borrow().selected.clone().unwrap();
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("Image isolation sibling missing")?;
    ui.select(sibling);
    until(|| !ui.state.borrow().loading && !ui.state.borrow().sending && !ui.state.borrow().pending.values().any(|request| matches!(&request.action,Pending::Send{thread,..} if thread==&selected.id))).await?;
    if !ui
        .state
        .borrow()
        .images
        .images(&ui.composer_key().unwrap())
        .is_empty()
    {
        return Err("Image attachments leaked into a sibling conversation".into());
    }
    if ui
        .state
        .borrow()
        .images
        .images(&key)
        .iter()
        .map(|image| image.id.clone())
        .collect::<Vec<_>>()
        != vec![newer.clone()]
    {
        return Err("Offscreen send acknowledgement cleared newer attachments".into());
    }
    ui.select(selected);
    until(|| {
        !ui.state.borrow().loading
            && ui.send.is_sensitive()
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    if ui.composer_text() != "Newer attachment draft 中文😀" {
        return Err("Offscreen image acknowledgement cleared a newer draft".into());
    }
    glib::timeout_future(Duration::from_millis(50)).await;
    capture("attachments.png");
    if ui.enqueue().is_ok() {
        return Err("Queue accepted an image message without supported persistence".into());
    }
    if ui.state.borrow().images.images(&key).len() != 1 {
        return Err("Queue rejection discarded the image".into());
    }

    ui.composer.buffer().set_text("");
    ui.submit();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_none()
            && ui.state.borrow().images.images(&key).is_empty()
    })
    .await?;
    ui.composer.buffer().set_text(&draft);
    Ok(
        json!({"ok":true,"fileSelection":true,"clipboardPaste":true,"clipboardSnapshot":true,"fileDrop":true,"remove":true,"failedSendRetention":true,"sessionIsolation":true,"newerAttachmentsRetained":true,"imageOnly":true,"queueGuard":true}),
    )
}

#[cfg(debug_assertions)]
pub(super) async fn smoke_opencode(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<(), String> {
    let key = ui.composer_key().ok_or("No OpenCode image session")?;
    ui.attach_files(&key, vec![directory.join("project/native-image-中文.png")])?;
    until(|| !ui.state.borrow().images.pending.contains_key(&key)).await?;
    if ui.state.borrow().images.images(&key).len() != 1 {
        return Err("OpenCode image did not load".into());
    }
    ui.composer.buffer().set_text("");
    ui.submit();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_none()
            && ui.state.borrow().images.images(&key).is_empty()
    })
    .await?;
    let bytes = std::fs::read_to_string(directory.join("opencode-requests.jsonl"))
        .map_err(|error| error.to_string())?;
    if !bytes
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .any(|value| {
            value["path"]
                .as_str()
                .is_some_and(|path| path.contains("/prompt_async"))
                && value.pointer("/body/parts/0/type") == Some(&json!("file"))
        })
    {
        return Err("OpenCode image-only prompt was not sent as a file part".into());
    }
    std::fs::write(directory.join("opencode-attachments.json"),serde_json::to_vec_pretty(&json!({"ok":true,"imageOnly":true,"serverOwnedIdentity":true,"authoritativeIdle":true})).unwrap()).map_err(|error|error.to_string())?;
    Ok(())
}
