//! Validated file-manager handoff through GTK/GIO, with native context actions.
use super::*;

fn detach_after_activation(popup: &gtk::PopoverMenu) {
    // GTK closes a menu before activating its item's inherited GAction. Keep
    // the action ancestry and native widgets alive until that dispatch returns.
    let popup = popup.clone();
    glib::idle_add_local_once(move || {
        if popup.parent().is_some() {
            popup.unparent();
        }
    });
}

impl Files {
    pub(super) fn install_actions(self: &Rc<Self>) {
        let group = gio::SimpleActionGroup::new();
        for (name, operation) in [
            ("reveal", FileOperation::Reveal),
            ("copy-path", FileOperation::CopyPath),
        ] {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::new("(ts)").unwrap()));
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, value| {
                if let (Some(files), Some((epoch, path))) =
                    (weak.upgrade(), value.and_then(|v| v.get::<(u64, String)>()))
                {
                    if epoch == files.generation.get() && files.active_scope(&path) {
                        files.item_operation(&path, operation);
                    } else {
                        files.toast_message("File context changed; open the menu again");
                    }
                }
            });
            group.add_action(&action);
        }
        self.widget.insert_action_group("files", Some(&group));
        let reveal = button("system-file-manager-symbolic", "Open project in Files");
        let weak = Rc::downgrade(self);
        reveal.connect_clicked(move |_| {
            if let Some(files) = weak.upgrade() {
                files.item_operation("", FileOperation::Reveal);
            }
        });
        if let Some(bar) = self.widget.first_child().and_downcast::<gtk::Box>() {
            bar.insert_child_after(&reveal, Some(&self.title));
        }
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        let weak = Rc::downgrade(self);
        gesture.connect_pressed(move |gesture, _, x, y| {
            if let Some(files) = weak.upgrade() {
                let mut picked = files.list.pick(x, y, gtk::PickFlags::DEFAULT);
                while let Some(widget) = picked {
                    if let Some(expander) = widget.downcast_ref::<gtk::TreeExpander>() {
                        if let Some(row) = expander.list_row() {
                            if let Data::Entry(e) = &node(&row.item().unwrap()).data {
                                if let Some(selection) =
                                    files.list.model().and_downcast::<gtk::SingleSelection>()
                                {
                                    selection.set_selected(row.position());
                                }
                                files.context_menu(&e.path, x, y);
                                gesture.set_state(gtk::EventSequenceState::Claimed);
                            }
                        }
                        return;
                    }
                    picked = widget.parent();
                }
            }
        });
        self.list.add_controller(gesture);
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, mods| {
            if key == gtk::gdk::Key::Menu
                || (key == gtk::gdk::Key::F10 && mods.contains(gtk::gdk::ModifierType::SHIFT_MASK))
            {
                if let Some(files) = weak.upgrade() {
                    if let Some(row) = files
                        .list
                        .model()
                        .and_downcast::<gtk::SingleSelection>()
                        .and_then(|m| m.selected_item())
                        .and_downcast::<gtk::TreeListRow>()
                    {
                        if let Data::Entry(e) = &node(&row.item().unwrap()).data {
                            files.context_menu(&e.path, 16.0, 16.0);
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }
            glib::Propagation::Proceed
        });
        self.list.add_controller(keys);
    }
    pub(super) fn context_menu(&self, path: &str, x: f64, y: f64) {
        self.close_context();
        let model = gio::Menu::new();
        let directory = self
            .nodes
            .borrow()
            .get(path)
            .and_then(Weak::upgrade)
            .is_some_and(|n| n.folder().is_some());
        for (label, action) in [
            (
                if directory {
                    "Open in Files"
                } else {
                    "Show in Files"
                },
                "files.reveal",
            ),
            ("Copy path", "files.copy-path"),
        ] {
            let item = gio::MenuItem::new(Some(label), None);
            item.set_action_and_target_value(
                Some(action),
                Some(&(self.generation.get(), path).to_variant()),
            );
            model.append_item(&item);
            i18n_ui::menu_label(&model, model.n_items() - 1, label);
        }
        let popup = gtk::PopoverMenu::from_model(Some(&model));
        popup.set_has_arrow(false);
        popup.set_parent(&self.list);
        popup.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popup.connect_closed(detach_after_activation);
        *self.context.borrow_mut() = Some(popup.clone());
        popup.popup();
    }
    pub(super) fn close_context(&self) {
        if let Some(old) = self.context.borrow_mut().take() {
            old.popdown();
            // Explicit scope/pane teardown cancels the menu and must not leave
            // a popup attached if the application event loop is stopping.
            if old.parent().is_some() {
                old.unparent();
            }
        }
    }
    pub(super) fn item_operation(&self, path: &str, operation: FileOperation) {
        if !self.has_root() || !self.active_scope(path) {
            return;
        }
        if self.operations.borrow().len() >= 8 {
            self.toast_message("File actions are busy; try again");
            return;
        }
        self.serial.set(self.serial.get() + 1);
        let serial = self.serial.get();
        let request = Request::Location {
            root: self.root.borrow().clone(),
            path: path.into(),
        };
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 1,
                generation: self.generation.get(),
                serial,
                request,
            })
            .is_ok()
        {
            self.operations.borrow_mut().insert(serial, operation);
        } else {
            self.toast_message("Files workspace is busy; try again");
        }
    }
    pub(super) fn location_event(&self, response: Response) {
        if response.generation != self.generation.get() {
            return;
        }
        let Some(operation) = self.operations.borrow_mut().remove(&response.serial) else {
            return;
        };
        let location = match response.result {
            Ok(Reply::Location(location)) => location,
            Err(error) => {
                self.toast.add_toast(adw::Toast::new(&error));
                return;
            }
            _ => return,
        };
        #[cfg(debug_assertions)]
        {
            *self.last_location.borrow_mut() = Some(location.clone());
        }
        if matches!(operation, FileOperation::CopyPath) {
            self.widget.clipboard().set_text(&location.path);
            return;
        }
        let file = gio::File::for_path(&location.path);
        let launcher = gtk::FileLauncher::new(Some(&file));
        // GTK's portal opens writable launches with O_RDWR. Directories need
        // a read descriptor even when the project itself is writable.
        launcher.set_writable(!location.read_only && !location.directory);
        #[cfg(debug_assertions)]
        {
            *self.last_handoff.borrow_mut() = Some((
                launcher
                    .file()
                    .and_then(|f| f.path())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                launcher.is_writable(),
            ));
        }
        #[cfg(debug_assertions)]
        if let Some(directory) =
            std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from)
        {
            if std::env::var_os("STUDIO_GNOME_SMOKE_REVEAL").is_some()
                && std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
                    == Some(directory.join("fake-codex.py"))
                && std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
                    == Some(directory.join("config"))
            {
                return;
            }
        }
        let toast = self.toast.downgrade();
        #[cfg(debug_assertions)]
        let probe = self.last_handoff_result.clone();
        let completed = move |result: Result<(), glib::Error>| {
            #[cfg(debug_assertions)]
            probe.replace(Some(
                result.as_ref().map(|_| ()).map_err(ToString::to_string),
            ));
            if let Err(error) = result {
                if !error.matches(gtk::DialogError::Dismissed)
                    && !error.matches(gio::IOErrorEnum::Cancelled)
                {
                    if let Some(toast) = toast.upgrade() {
                        let notice = adw::Toast::new("");
                        i18n_ui::bind_format(
                            &notice,
                            "title",
                            "Unable to open Files: {error}",
                            &[("error", &error.to_string())],
                        );
                        toast.add_toast(notice);
                    }
                }
            }
        };
        if location.directory {
            launcher.launch(Some(&self.window), gio::Cancellable::NONE, completed);
        } else {
            launcher.open_containing_folder(Some(&self.window), gio::Cancellable::NONE, completed);
        }
    }
}
