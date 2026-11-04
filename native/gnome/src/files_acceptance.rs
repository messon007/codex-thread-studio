//! Source-specific GTK tree and real notify events; private fixture files only.
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
async fn fixture_work(
    work: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    gio::spawn_blocking(work)
        .await
        .map_err(|_| "Files fixture worker stopped")?
}
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(12) {
            return Err(format!("Native Files acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
struct Timer(Option<glib::SourceId>);
impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(id) = self.0.take() {
            id.remove();
        }
    }
}
fn contains_label(widget: &gtk::Widget, label: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|l| l.text() == label)
    {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        if contains_label(&w, label) {
            return true;
        }
    }
    false
}
fn copy_control(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.css_name() == "modelbutton" && contains_label(widget, "Copy path") {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        if let Some(found) = copy_control(&w) {
            return Some(found);
        }
    }
    None
}

impl Files {
    pub fn smoke_shared_root(&self, path: &str) -> bool {
        self.smoke_row(path).is_some()
    }
    pub async fn smoke_shared(
        &self,
        root: &Path,
        shared: &Path,
        capture: impl Fn(),
    ) -> Result<serde_json::Value, String> {
        let folder = shared.to_string_lossy().into_owned();
        let path = shared.join("notes.md").to_string_lossy().into_owned();
        let owned_input = std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some();
        let fixture = root.parent().ok_or("Private fixture parent missing")?;
        if owned_input {
            self.window.fullscreen();
            until("fullscreen Files allocation", || {
                self.window.is_fullscreen()
                    && self.window.width() == 1600
                    && self.window.height() == 1000
                    && self.window.scale_factor() == 1
            })
            .await?;
            crate::smoke_input::request(fixture, serde_json::json!({"action":"focus"})).await?;
            until("focused Files window", || self.window.is_active()).await?;
        }
        self.smoke_until("shared root inventory", || {
            self.smoke_row(&folder).is_some()
        })
        .await?;
        if self.root.borrow().as_str() != root.to_string_lossy() {
            return Err("Shared folder replaced the project root".into());
        }
        if let Some(project) = self.tree.row(0) {
            project.set_expanded(false);
        }
        self.smoke_row(&folder)
            .ok_or("Shared group missing")?
            .set_expanded(true);
        self.smoke_until("shared contents", || {
            self.smoke_contains(&path)
                && self.watch_status.borrow().as_str() == "Live updates"
                && self.pending.borrow().is_empty()
        })
        .await?;
        let row = self.smoke_row(&path).ok_or("Shared file missing")?;
        if !matches!(&node(&row.item().unwrap()).data,Data::Entry(e)if e.read_only) {
            return Err("Shared file lost its read-only badge".into());
        }
        let private = shared.to_owned();
        fixture_work(move || {
            std::fs::write(private.join("live-shared.md"), "# live shared\n")
                .map_err(|e| e.to_string())
        })
        .await?;
        let live = shared.join("live-shared.md").to_string_lossy().into_owned();
        self.smoke_until("live shared changes", || self.smoke_contains(&live))
            .await?;
        if owned_input {
            let observed = Rc::new(RefCell::new(Vec::new()));
            let events = observed.clone();
            let probe = gtk::EventControllerKey::new();
            probe.set_propagation_phase(gtk::PropagationPhase::Capture);
            probe.connect_key_pressed(move |_, key, _, modifiers| {
                events.borrow_mut().push(format!("{key:?}/{modifiers:?}"));
                glib::Propagation::Proceed
            });
            self.window.add_controller(probe.clone());
            let position = self
                .smoke_row(&path)
                .ok_or("Shared row disappeared")?
                .position();
            self.list
                .model()
                .and_downcast::<gtk::SingleSelection>()
                .ok_or("Files selection model missing")?
                .set_selected(position);
            if !self.list.grab_focus() {
                return Err("Files tree could not receive keyboard focus".into());
            }
            crate::smoke_input::request(fixture, serde_json::json!({"action":"key","key":"menu"}))
                .await?;
            let mapped = until("keyboard context menu mapped", || {
                self.context
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.is_mapped())
            })
            .await;
            self.window.remove_controller(&probe);
            if let Err(error) = mapped {
                let focus = gtk::prelude::GtkWindowExt::focus(&self.window)
                    .map(|w| (w.type_().name(), w.css_name().to_string()));
                return Err(format!(
                    "{error}: keys={:?}; focus={focus:?}; listMapped={}; context={:?}; selected={:?}",
                    observed.borrow(),
                    self.list.is_mapped(),
                    self.context.borrow().as_ref().map(|p| (p.is_visible(), p.is_mapped(), p.width(), p.height())),
                    self.list.model().and_downcast::<gtk::SingleSelection>().map(|m| m.selected())
                ));
            }
        } else {
            self.context_menu(&path, 32.0, 80.0);
        }
        glib::timeout_future(Duration::from_millis(60)).await;
        let popup = self
            .context
            .borrow()
            .clone()
            .ok_or("File context menu missing")?;
        let menu_mapped = popup.is_mapped();
        if menu_mapped {
            if let (Some(child), Some(renderer)) = (popup.first_child(), popup.renderer()) {
                let snapshot = gtk::Snapshot::new();
                popup.snapshot_child(&child, &snapshot);
                if let Some(node) = snapshot.to_node() {
                    renderer
                        .render_texture(&node, None)
                        .save_to_png(root.parent().unwrap().join("files-context-popup.png"))
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        capture();
        self.last_location.borrow_mut().take();
        if owned_input {
            let copy =
                copy_control(popup.upcast_ref()).ok_or("Mapped Copy path menu control missing")?;
            if !copy.grab_focus() {
                return Err("Copy path menu control could not receive focus".into());
            }
            crate::smoke_input::request(fixture, serde_json::json!({"action":"key","key":"enter"}))
                .await?;
        } else {
            self.close_context();
            self.widget
                .activate_action(
                    "files.copy-path",
                    Some(&(self.generation.get(), path.as_str()).to_variant()),
                )
                .map_err(|e| e.to_string())?;
        }
        self.smoke_until("copied shared location", || {
            self.last_location
                .borrow()
                .as_ref()
                .is_some_and(|l| l.path == path && l.read_only)
        })
        .await?;
        let copied = self.widget.clipboard().read_text_future().await;
        let clipboard_error = match copied {
            Ok(value) if value.as_deref() == Some(path.as_str()) => None,
            Ok(_) => Some("Clipboard did not retain the validated path".to_string()),
            Err(error) => Some(error.to_string()),
        };
        let independent_clipboard = if owned_input {
            let copied =
                crate::smoke_input::request(fixture, serde_json::json!({"action":"read"})).await?;
            if copied["text"].as_str() != Some(path.as_str()) {
                return Err(
                    "Independent Wayland clipboard did not receive the canonical shared path"
                        .into(),
                );
            }
            true
        } else {
            false
        };
        if owned_input {
            // Exercise the pointer controller too, on a different shared file.
            self.close_context();
            fn find_row(widget: &gtk::Widget, path: &str) -> Option<gtk::Widget> {
                if !widget.is_mapped() {
                    return None;
                }
                if widget
                    .downcast_ref::<gtk::TreeExpander>()
                    .and_then(|e| e.list_row())
                    .and_then(|r| r.item())
                    .is_some_and(|o| matches!(&node(&o).data, Data::Entry(e) if e.path == path))
                {
                    return Some(widget.clone());
                }
                let mut child = widget.first_child();
                while let Some(w) = child {
                    child = w.next_sibling();
                    if let Some(found) = find_row(&w, path) {
                        return Some(found);
                    }
                }
                None
            }
            let target =
                find_row(self.list.upcast_ref(), &live).ok_or("Pointer target row missing")?;
            let rect = target
                .compute_bounds(&self.window)
                .ok_or("Pointer target unmapped")?;
            crate::smoke_input::request(
                fixture,
                serde_json::json!({"action":"pointer","button":273,
                "x":rect.x()+rect.width()/2.0,"y":rect.y()+rect.height()/2.0}),
            )
            .await?;
            until("right-click menu mapped", || {
                self.context
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.is_mapped())
            })
            .await?;
            let popup = self
                .context
                .borrow()
                .clone()
                .ok_or("Pointer menu disappeared")?;
            let control = copy_control(popup.upcast_ref()).ok_or("Pointer Copy path missing")?;
            if !control.grab_focus() {
                return Err("Pointer menu Copy path cannot receive focus".into());
            }
            self.last_location.borrow_mut().take();
            crate::smoke_input::request(fixture, serde_json::json!({"action":"key","key":"enter"}))
                .await?;
            self.smoke_until("pointer action's validated path", || {
                self.last_location
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.path == live && p.read_only)
            })
            .await?;
            let copied =
                crate::smoke_input::request(fixture, serde_json::json!({"action":"read"})).await?;
            if copied["text"].as_str() != Some(live.as_str()) {
                return Err("Right-click action copied a different file".into());
            }
        }
        self.widget
            .activate_action(
                "files.reveal",
                Some(&(self.generation.get(), path.as_str()).to_variant()),
            )
            .map_err(|e| e.to_string())?;
        self.smoke_until("read-only GTK file launcher", || {
            self.last_handoff
                .borrow()
                .as_ref()
                .is_some_and(|(p, w)| p == &path && !*w)
        })
        .await?;
        let position = (0..self.tree.n_items())
            .find(|i| {
                self.tree.row(*i).is_some_and(
                    |r| matches!(&node(&r.item().unwrap()).data,Data::Entry(e)if e.path==path),
                )
            })
            .ok_or("Shared file row missing")?;
        self.list.emit_by_name::<()>("activate", &[&position]);
        Ok(
            serde_json::json!({"ok":clipboard_error.is_none()&&menu_mapped,"sharedTree":true,"readOnlyBadge":true,"liveSharedChanges":true,"contextMenuMapped":menu_mapped,"validatedCopyAction":true,"clipboardRoundTrip":clipboard_error.is_none(),"clipboardError":clipboard_error,"keyboardMenuAndCopy":owned_input,"pointerMenuAndCopy":owned_input,"independentWaylandClipboard":independent_clipboard,"gtkLauncherConfiguredReadOnly":true,"nativeReaderActivation":true,"portalLaunch":"not invoked by private fixture"}),
        )
    }
    async fn smoke_until(&self, step: &str, test: impl Fn() -> bool) -> Result<(), String> {
        until(step, test).await.map_err(|e| {
            format!(
                "{e}: {}; watch={}; pending={} queued={}",
                self.status.text(),
                self.watch_status.borrow(),
                self.pending.borrow().len(),
                self.queue.borrow().len()
            )
        })
    }
    fn smoke_contains(&self, path: &str) -> bool {
        self.smoke_row(path).is_some()
    }
    fn smoke_row(&self, path: &str) -> Option<gtk::TreeListRow> {
        (0..self.tree.n_items())
            .filter_map(|i| self.tree.row(i))
            .find(|row| matches!(&node(&row.item().unwrap()).data,Data::Entry(e)if e.path==path))
    }
    fn smoke_children(&self, path: &str) -> usize {
        self.nodes
            .borrow()
            .get(path)
            .and_then(Weak::upgrade)
            .map_or(0, |n| {
                (0..n.children.n_items())
                    .filter(|i| matches!(&node(&n.children.item(*i).unwrap()).data, Data::Entry(_)))
                    .count()
            })
    }
    pub async fn smoke_test(
        &self,
        root: &Path,
        capture: impl Fn(),
    ) -> Result<serde_json::Value, String> {
        let opened = Instant::now();
        self.open(&root.to_string_lossy());
        self.smoke_until("initial directory", || {
            self.smoke_contains("nested") && self.pending.borrow().is_empty()
        })
        .await?;
        let open_ms = opened.elapsed().as_millis();
        if self.smoke_contains("outside-link.txt") {
            return Err("Files exposed an outside-project symlink".into());
        }
        let nested = self.smoke_row("nested").ok_or("Nested folder missing")?;
        nested.set_expanded(true);
        self.smoke_until("nested contents", || self.smoke_contains("nested/note.md"))
            .await?;
        capture();
        let ticks = Rc::new(Cell::new(0u64));
        let gap = Rc::new(Cell::new(0u128));
        let last = Rc::new(Cell::new(Instant::now()));
        let (count, maximum, previous) = (ticks.clone(), gap.clone(), last.clone());
        let _timer = Timer(Some(glib::timeout_add_local(
            Duration::from_millis(16),
            move || {
                count.set(count.get() + 1);
                maximum.set(maximum.get().max(previous.get().elapsed().as_millis()));
                previous.set(Instant::now());
                glib::ControlFlow::Continue
            },
        )));
        self.smoke_until("watch ready", || {
            self.watch_status.borrow().as_str() == "Live updates"
                && self.watched.borrow().iter().any(|p| p == "nested")
        })
        .await?;
        let private = root.to_owned();
        fixture_work(move || {
            std::fs::write(private.join("native-watch.md"), "# live root\n")
                .map_err(|e| e.to_string())?;
            std::fs::write(private.join("nested/native-watch.md"), "# live child\n")
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await?;
        self.smoke_until("live creation", || {
            self.smoke_contains("native-watch.md") && self.smoke_contains("nested/native-watch.md")
        })
        .await?;
        if !self.smoke_row("nested").is_some_and(|r| r.is_expanded()) {
            return Err("A live refresh collapsed the nested folder".into());
        }
        let private = root.to_owned();
        fixture_work(move || {
            std::fs::rename(
                private.join("native-watch.md"),
                private.join("native-renamed.md"),
            )
            .map_err(|e| e.to_string())?;
            std::fs::remove_file(private.join("nested/native-watch.md"))
                .map_err(|e| e.to_string())?;
            std::fs::write(private.join(".native-hidden"), "hidden").map_err(|e| e.to_string())?;
            std::fs::write(private.join("native-ignored.tmp"), "ignored")
                .map_err(|e| e.to_string())?;
            std::fs::write(private.join(".ignore"), "native-ignored.tmp\n")
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await?;
        self.smoke_until("live rename and deletion", || {
            self.smoke_contains("native-renamed.md")
                && !self.smoke_contains("native-watch.md")
                && !self.smoke_contains("nested/native-watch.md")
                && self.pending.borrow().is_empty()
        })
        .await?;
        if self.smoke_contains(".native-hidden") || self.smoke_contains("native-ignored.tmp") {
            return Err("Visibility defaults did not respect hidden/ignore rules".into());
        }
        self.hidden.set_active(true);
        self.ignored.set_active(true);
        self.smoke_until("show hidden/ignored", || {
            self.smoke_contains(".native-hidden") && self.smoke_contains("native-ignored.tmp")
        })
        .await?;
        self.hidden.set_active(false);
        self.ignored.set_active(false);
        self.smoke_until("hide hidden/ignored", || {
            !self.smoke_contains(".native-hidden")
                && !self.smoke_contains("native-ignored.tmp")
                && self.pending.borrow().is_empty()
        })
        .await?;
        let large = std::env::var_os("STUDIO_GNOME_SMOKE_FILES_LARGE").is_some();
        let count = if large { 10_020 } else { 240 };
        let private = root.to_owned();
        fixture_work(move || {
            std::fs::create_dir(private.join("native-many")).map_err(|e| e.to_string())?;
            for n in 0..count {
                std::fs::write(
                    private.join(format!("native-many/item-{n:03}.md")),
                    "# page\n",
                )
                .map_err(|e| e.to_string())?;
            }
            std::fs::write(private.join("native-many/中文😀.md"), "# unicode\n")
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await?;
        self.smoke_until("new paged folder", || self.smoke_contains("native-many"))
            .await?;
        let folder_opened = Instant::now();
        self.smoke_row("native-many")
            .ok_or("Paged folder missing")?
            .set_expanded(true);
        self.smoke_until("first directory page", || {
            self.smoke_children("native-many") == 200
        })
        .await?;
        let folder_open_ms = folder_opened.elapsed().as_millis();
        let more=(0..self.tree.n_items()).find(|i|self.tree.row(*i).is_some_and(|r|matches!(&node(&r.item().unwrap()).data,Data::More{parent,..}if parent=="native-many"))).ok_or("Load more action missing")?;
        let parent = self
            .nodes
            .borrow()
            .get("native-many")
            .and_then(Weak::upgrade)
            .ok_or("Paged directory missing")?;
        self.enqueue(&parent, None);
        self.list.emit_by_name::<()>("activate", &[&more]);
        self.smoke_until("second directory page", || {
            self.smoke_children("native-many") == if large { 400 } else { 241 }
        })
        .await?;
        if !large && !self.smoke_contains("native-many/中文😀.md") {
            return Err("Paged tree lost its Unicode path".into());
        }
        if large {
            self.search.set_text("中文😀");
            self.smoke_until("Unicode file in a large folder", || {
                self.smoke_contains("native-many/中文😀.md")
                    && self.smoke_children("native-many") == 1
            })
            .await?;
        }
        self.search.set_text("item-239.md ");
        self.smoke_until("literal trailing whitespace", || {
            self.query.borrow().as_str() == "item-239.md "
                && self.pending.borrow().is_empty()
                && self.smoke_children("native-many") == 0
        })
        .await?;
        self.search.set_text("item-239.md");
        self.smoke_until("file filter match", || {
            self.smoke_contains("native-many/item-239.md")
                && self.smoke_children("native-many") == 1
        })
        .await?;
        self.search.set_text("");
        self.smoke_until("clear file filter", || {
            self.query.borrow().is_empty()
                && self.smoke_children("native-many") == 200
                && self.pending.borrow().is_empty()
        })
        .await?;
        self.smoke_language(
            root.parent()
                .ok_or("Private Files fixture parent missing")?,
        )
        .await?;
        let status = self.status.text();
        self.watch_event(
            self.watch_generation.get().saturating_sub(1),
            crate::files_watch::Update::Error {
                message: "stale".into(),
            },
        );
        if self.status.text() != status {
            return Err("An obsolete Files watch changed the UI".into());
        }
        capture();
        last.set(Instant::now());
        Ok(
            serde_json::json!({"ok":true,"nativeList":true,"folderNavigation":true,"symlinkConfinement":true,"lazyTree":true,"liveCreateRenameDelete":true,"expandedStateRetained":true,"ignoreAndHiddenToggles":true,"pagedDirectory":true,"paginationDuringRefresh":true,"literalWhitespaceFilter":true,"staleWatchIsolation":true,"liveLanguage":true,"literalRowBindings":true,"menuTargetPreserved":true,"sourceFolderEntries":count+1,"initialOpenMs":open_ms,"largeFolderFirstPageMs":folder_open_ms,"mainLoopTicks":ticks.get(),"maximumWorkflowGapExcludingSnapshotsMs":gap.get()}),
        )
    }
}

impl Files {
    pub async fn smoke_portal_launch(
        &self,
        root: &Path,
        shared: &Path,
    ) -> Result<serde_json::Value, String> {
        let trace = crate::smoke_portal::read(root).await?;
        let calls = trace["calls"].as_array().map_or(0, Vec::len);
        self.smoke_until("portal shared root", || {
            self.smoke_shared_root(&shared.to_string_lossy())
        })
        .await?;
        for (path, writable, read_only) in [
            (
                shared.join("notes.md").to_string_lossy().into_owned(),
                false,
                true,
            ),
            (String::new(), false, false),
        ] {
            self.last_handoff_result.borrow_mut().take();
            self.widget
                .activate_action(
                    "files.reveal",
                    Some(&(self.generation.get(), path.as_str()).to_variant()),
                )
                .map_err(|e| e.to_string())?;
            until("actual GTK file handoff completed", || {
                self.last_handoff_result.borrow().is_some()
            })
            .await?;
            if let Some(Err(error)) = self.last_handoff_result.borrow().as_ref() {
                return Err(format!("GTK file launcher failed: {error}"));
            }
            if self
                .last_handoff
                .borrow()
                .as_ref()
                .is_none_or(|(_, w)| *w != writable)
            {
                return Err("File launcher lost its access class".into());
            }
            if self
                .last_location
                .borrow()
                .as_ref()
                .is_none_or(|l| l.read_only != read_only)
            {
                return Err("File handoff changed the workspace access class".into());
            }
        }
        crate::smoke_portal::wait(root, "calls", calls + 2).await?;
        Ok(
            serde_json::json!({"sharedFileDirectoryHandoff":true,"projectDirectoryHandoff":true,"realFdTransport":true,"accessClassPreserved":true,"readableDirectoryDescriptor":true,"gtkCompletion":true}),
        )
    }
}
