//! Shared folders remain children of the current project context, never new projects.
use super::*;
impl Files {
    /// Reveal within the project model. Never promote a conversation path to
    /// an authorized root; ordinary folder reads still enforce symlink policy.
    pub fn open_folder(&self, root: &str, path: &std::path::Path) -> Result<(), String> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "The path is outside the current session root")?;
        if root.is_empty()
            || relative.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err("The path is outside the current session root".into());
        }
        self.open(root);
        self.search.set_text("");
        *self.reveal_folder.borrow_mut() = Some(relative.to_string_lossy().into_owned());
        self.schedule_sync();
        Ok(())
    }
    pub(super) fn reveal_pending_folder(&self) {
        if !self.widget.is_mapped() {
            return;
        }
        let Some(target) = self.reveal_folder.borrow().clone() else {
            return;
        };
        let target_path = std::path::Path::new(&target);
        let mut i = 0;
        while i < self.tree.n_items() {
            if let Some(row) = self.tree.row(i) {
                if let Some(object) = row.item() {
                    let current = node(&object);
                    if let Some(folder) = current.folder() {
                        if folder.is_empty() || target_path.starts_with(folder) {
                            row.set_expanded(true);
                            if folder == target && current.loaded.get() {
                                self.reveal_folder.borrow_mut().take();
                                self.list.scroll_to(i, gtk::ListScrollFlags::NONE, None);
                                return;
                            }
                        }
                    }
                }
            }
            i += 1;
        }
    }
    pub fn request_roots(&self) {
        if !self.has_root() {
            return;
        }
        self.serial.set(self.serial.get() + 1);
        let serial = self.serial.get();
        self.roots_serial.set(serial);
        let request = Request::Roots {
            root: self.root.borrow().clone(),
        };
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 1,
                generation: self.root_epoch.get(),
                serial,
                request,
            })
            .is_err()
        {
            i18n_ui::bind(
                &self.status,
                "label",
                "Files workspace is busy; use Refresh",
            );
        }
    }
    pub(super) fn active_scope(&self, path: &str) -> bool {
        if !std::path::Path::new(path).is_absolute() {
            return true;
        }
        (0..self.roots.n_items()).filter_map(|i|self.roots.item(i)).any(|object|{
            matches!(&node(&object).data,Data::Entry(e)if !e.path.is_empty()&&std::path::Path::new(path).starts_with(&e.path))
        })
    }
    pub(super) fn roots_event(&self, response: Response) {
        if response.generation != self.root_epoch.get()
            || response.serial != self.roots_serial.get()
        {
            return;
        }
        let entries = match response.result {
            Ok(Reply::Roots(entries)) => entries,
            Err(error) => {
                i18n_ui::literal(&self.status, "label", &error);
                return;
            }
            _ => return,
        };
        let old = (0..self.roots.n_items())
            .filter_map(|i| self.roots.item(i).and_downcast::<glib::BoxedAnyObject>())
            .collect::<Vec<_>>();
        let mut objects = Vec::new();
        for entry in entries {
            if let Some(object) = old
                .iter()
                .find(|object| matches!(&node(object.upcast_ref()).data,Data::Entry(e)if *e==entry))
            {
                objects.push(object.clone());
            } else {
                let path = entry.path.clone();
                let current = Node::new(Data::Entry(entry));
                self.nodes
                    .borrow_mut()
                    .insert(path, Rc::downgrade(&current));
                objects.push(glib::BoxedAnyObject::new(current));
            }
        }
        let changed = old != objects;
        if changed {
            self.invalidate_reads(false);
            let expanded = self
                .expanded()
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let prefix = old.iter().zip(&objects).take_while(|(a, b)| a == b).count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(objects[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            self.roots.splice(
                prefix as u32,
                (old.len() - prefix - suffix) as u32,
                &objects[prefix..objects.len() - suffix],
            );
            let mut i = 0;
            while i < self.tree.n_items() {
                if let Some(row) = self.tree.row(i) {
                    if let Some(path) = node(&row.item().unwrap()).folder() {
                        if expanded.contains(path) {
                            row.set_expanded(true);
                        }
                    }
                }
                i += 1;
            }
            self.nodes
                .borrow_mut()
                .retain(|path, weak| weak.strong_count() > 0 && self.active_scope(path));
            if self.widget.is_mapped() {
                self.refresh_all();
                self.schedule_sync();
            }
        }
    }
}
