use super::*;
use crate::router::{Display, History, PageSpec, Receipt, Registry, Reply, Request};
use std::time::Instant;
#[cfg(debug_assertions)]
#[path = "router_notification_acceptance.rs"]
mod notification_acceptance;
#[cfg(debug_assertions)]
#[path = "router_search_acceptance.rs"]
mod search_acceptance;

#[derive(Clone, PartialEq, Eq)]
struct NavigationScope {
    backend: String,
    thread: Option<String>,
    generation: u64,
}
struct Lookup {
    serial: u64,
    scope: NavigationScope,
    item: Arc<Item>,
    origin: Arc<model::Origin>,
    target_generation: Option<u64>,
    kind: String,
}
struct Opening {
    backend: String,
    thread: String,
    generation: u64,
    origin: Arc<model::Origin>,
    started: Instant,
}
enum Navigation {
    Lookup(Lookup),
    Opening(Opening),
}
struct ReturnTo {
    backend: String,
    thread: Thread,
    target_key: String,
}
struct PendingInput {
    annotation: Option<Arc<crate::annotations::Submission>>,
    key: String,
    id: String,
    text: String,
    images: Vec<Arc<crate::attachments::Image>>,
    references: Vec<Arc<crate::composer_refs::Reference>>,
}
#[derive(Default)]
pub(super) struct Book {
    notices: crate::router_notifications::Tracker,
    pub registry: Option<Registry>,
    pub jobs: HashMap<String, Display>,
    pub histories: Vec<History>,
    view: Option<(PageSpec, History)>,
    page: Option<u64>,
    pub scroll: Option<bool>,
    stamps: HashMap<String, String>,
    serial: u64,
    sync: Option<u64>,
    send: Option<u64>,
    stop: Option<u64>,
    seen: Option<u64>,
    next_seen: Option<Instant>,
    dwell: HashMap<(String, String), Instant>,
    pub(super) pending: Option<(Value, String, String, bool)>,
    pub submitting: bool,
    pub stopping: bool,
    pub next_poll: Option<Instant>,
    error: Option<String>,
    navigation: Option<Navigation>,
    return_to: Option<ReturnTo>,
    targets: HashMap<String, crate::router::Target>,
    choice_scope: Option<NavigationScope>,
    picker_request: Option<u64>,
    picker_id: Option<u64>,
    input_pending: Option<PendingInput>,
    queued_input: Option<Arc<crate::message_queue::Message>>,
    #[cfg(debug_assertions)]
    drop_queued_ack_once: bool,
    files_request: Option<(u64, NavigationScope)>,
}
impl Book {
    pub(super) fn pending_retry(&self) -> bool {
        self.pending.is_some() && !self.submitting
    }
    pub(super) fn pending_for(&self, key: &str) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, controller, _, _)| controller == key)
    }
}
impl Ui {
    pub(super) fn router_reference_target(&self, key: &str) -> Option<crate::router::Target> {
        self.router.borrow().targets.get(key).cloned()
    }
    fn router_scope(&self) -> NavigationScope {
        let state = self.state.borrow();
        NavigationScope {
            backend: state.backend.clone(),
            thread: state.selected.as_ref().map(|thread| thread.id.clone()),
            generation: state.socket_generation,
        }
    }
    pub(super) fn router_attach_reference_files(&self, paths: Vec<std::path::PathBuf>) {
        if !self.router_selected() {
            return;
        }
        let scope = self.router_scope();
        let serial = {
            let mut book = self.router.borrow_mut();
            book.serial += 1;
            let serial = book.serial;
            book.files_request = Some((serial, scope));
            serial
        };
        if !self.command(Command::Router {
            serial,
            request: Request::PrepareFiles { paths, tag: serial },
        }) {
            self.router.borrow_mut().files_request = None;
            self.error("Reference file preparation could not be started");
        }
        self.update_composer();
    }
    fn router_choose_reference_files(self: &Rc<Self>) {
        let scope = self.router_scope();
        let dialog = gtk::FileDialog::builder()
            .title("Attach reference files")
            .accept_label("Attach")
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        let parent = self.window.clone();
        glib::spawn_future_local(async move {
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
                        ui.error(&format!("Cannot select reference files: {error}"));
                    }
                    return;
                }
            };
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.router_scope() != scope || !ui.router_selected() {
                ui.error("The conversation changed; select reference files again");
                return;
            }
            if files.n_items() > 32 {
                ui.error("Choose at most 32 reference files");
                return;
            }
            let mut paths = Vec::new();
            for index in 0..files.n_items() {
                let path = files
                    .item(index)
                    .and_downcast::<gio::File>()
                    .and_then(|file| file.path());
                let Some(path) = path else {
                    ui.error("Choose files on the Studio server filesystem");
                    return;
                };
                paths.push(path);
            }
            if !paths.is_empty() {
                ui.router_attach_reference_files(paths);
            }
        });
    }
    pub(super) fn router_inputs_waiting(&self) -> bool {
        self.router.borrow().input_pending.is_some()
    }
    pub(super) fn router_files_preparing(&self) -> bool {
        self.router.borrow().files_request.is_some()
    }
    fn router_finish_inputs(&self, accepted: bool) {
        let pending = self.router.borrow_mut().input_pending.take();
        let Some(pending) = pending else {
            return;
        };
        if let Some(ticket) = &pending.annotation {
            self.annotation_result(ticket, accepted);
        }
        let selected = self.composer_key().as_deref() == Some(pending.key.as_str());
        if accepted && selected && self.composer_text() == pending.text {
            self.composer.buffer().set_text("");
        }
        let mut state = self.state.borrow_mut();
        state.images.acknowledged(
            &pending.key,
            &pending
                .images
                .iter()
                .map(|image| image.id.clone())
                .collect::<Vec<_>>(),
            accepted,
        );
        if accepted {
            state.references.remove(
                &pending.key,
                &pending
                    .references
                    .iter()
                    .map(|reference| reference.id.clone())
                    .collect::<Vec<_>>(),
            );
        }
        if accepted && state.drafts.get(&pending.key) == Some(&pending.text) {
            state.drafts.remove(&pending.key);
        }
    }
    fn router_observe_inputs(&self, rows: &[crate::router::Tracked]) {
        let outcome = {
            let book = self.router.borrow();
            book.input_pending
                .as_ref()
                .and_then(|pending| {
                    rows.iter()
                        .find(|row| row.controller == pending.key && row.id == pending.id)
                })
                .and_then(|row| {
                    if !row.target_turn_id.is_empty() {
                        Some(true)
                    } else if row.status.as_deref().is_some_and(|status| {
                        matches!(status, "completed" | "failed" | "clarify" | "stopped")
                    }) {
                        Some(false)
                    } else {
                        None
                    }
                })
        };
        if let Some(accepted) = outcome {
            self.router_finish_inputs(accepted);
        }
    }
    pub(super) fn router_queue_gate(&self, key: &str) -> Option<String> {
        let book = self.router.borrow();
        book.pending
            .as_ref()
            .filter(|(_, controller, _, _)| controller == key)
            .and_then(|(body, _, _, _)| body["requestId"].as_str().map(str::to_owned))
            .or_else(|| {
                book.histories
                    .iter()
                    .find(|history| history.controller == key)
                    .and_then(|history| history.active_ids.first().cloned())
            })
    }
    pub(super) fn router_queue_ready(&self, key: &str) -> bool {
        let book = self.router.borrow();
        !book.submitting
            && book.input_pending.is_none()
            && book.pending.is_none()
            && book
                .registry
                .as_ref()
                .and_then(|registry| registry.active())
                .as_deref()
                == Some(key)
            && book.histories.iter().any(|history| {
                history.controller == key
                    && !history.controller_busy
                    && history.active_ids.is_empty()
            })
    }
    fn router_queue_tracking(&self) -> Vec<(String, String)> {
        let state = self.state.borrow();
        let mut tracking = Vec::new();
        for (key, entry) in &state.queues.entries {
            if !entry.router {
                continue;
            }
            if let Some(id) = &entry.router_wait {
                tracking.push((key.clone(), id.clone()));
            }
            if let Some(flight) = &entry.flight {
                tracking.push((key.clone(), flight.id.clone()));
            }
        }
        if let Some(input) = &self.router.borrow().input_pending {
            tracking.push((input.key.clone(), input.id.clone()));
        }
        tracking.sort();
        tracking.dedup();
        let book = self.router.borrow();
        book.notices.tracking(&book.histories, &mut tracking);
        tracking
    }
    fn router_queue_observe(&self, rows: &[crate::router::Tracked], confirmed: bool) {
        let unconfirmed = self
            .router
            .borrow()
            .pending
            .as_ref()
            .filter(|(_, _, _, queued)| *queued)
            .and_then(|(body, key, _, _)| {
                body["requestId"]
                    .as_str()
                    .map(|id| (key.clone(), id.to_owned()))
            });
        let mut state = self.state.borrow_mut();
        for row in rows {
            // A status UUID alone does not establish that the queued text and
            // references match the accepted request. Preserve the snapshot until
            // POST acknowledges it; explicit retries use its original UUID.
            if !confirmed
                && unconfirmed
                    .as_ref()
                    .is_some_and(|(key, id)| key == &row.controller && id == &row.id)
            {
                continue;
            }
            if let Some(status) = &row.status {
                if let Some(entry) = state.queues.entries.get_mut(&row.controller) {
                    entry.observe_router(&row.id, status);
                }
            }
        }
        // Jobs remain authoritative even when another conversation/history page
        // is displayed. Never derive target completion from controller SDK events.
        state.queues.dirty = true;
    }
    fn router_queue_gates(&self) {
        let book = self.router.borrow();
        let mut state = self.state.borrow_mut();
        for history in &book.histories {
            if let Some(entry) = state.queues.entries.get_mut(&history.controller) {
                entry.router = true;
                if entry.router_wait.is_none() && entry.flight.is_none() {
                    if let Some(id) = history.active_ids.first() {
                        entry.router_wait = Some(id.clone());
                        entry.active = true;
                        entry.released = false;
                        entry.touch();
                    }
                }
            }
        }
        state.queues.dirty = true;
    }
    fn router_queue_rejected(&self, error: &str) {
        let pending = self.router.borrow().pending.clone();
        if let Some((body, key, _, queued)) = pending {
            let mut state = self.state.borrow_mut();
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                if entry.router_wait.as_deref() == body["requestId"].as_str() {
                    entry.router_wait = None;
                    entry.active = false;
                }
                if queued
                    && entry.flight.as_ref().is_some_and(|flight| {
                        Some(flight.id.as_str()) == body["requestId"].as_str()
                    })
                {
                    entry.flight = None;
                    entry.active = false;
                }
                entry.pause(error);
            }
        }
    }
    pub(super) fn router_dispatch_queue(
        &self,
        context: crate::message_queue::Context,
        message: Arc<crate::message_queue::Message>,
        options: crate::turn_options::Options,
    ) {
        let key = context.key();
        if !self.router_queue_ready(&key) || message.text.len() > 64 * 1024 {
            let mut state = self.state.borrow_mut();
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                entry.flight = None;
                entry.pause("Router queue is unavailable, or the message exceeds 64 KiB. Check the draft and Router settings before resuming.");
            }
            return;
        }
        let (serial, body) = {
            let mut book = self.router.borrow_mut();
            let Some(registry) = &book.registry else {
                return;
            };
            let explicit = book
                .targets
                .get(&key)
                .map(|target| json!({"sessionKey":target.session_key,"cwd":target.cwd}));
            let body = json!({"requestId":message.id,"queuedMessageId":message.id,"controller":key,"expectedRevision":registry.revision,"prompt":message.text,"explicitTarget":explicit,"options":options,"policies":self.state.borrow().policies.router_snapshot()});
            book.pending = Some((body.clone(), key.clone(), message.text.clone(), true));
            book.queued_input = Some(message.clone());
            book.serial += 1;
            let serial = book.serial;
            book.send = Some(serial);
            book.submitting = true;
            (serial, body)
        };
        if !self.command(Command::Router {
            serial,
            request: Request::Start {
                body,
                images: Vec::new(),
                references: Vec::new(),
                queued_references: Some(message.input.clone()),
            },
        }) {
            let mut book = self.router.borrow_mut();
            book.submitting = false;
            book.send = None;
        }
    }
    fn router_picker_current(&self) -> bool {
        let state = self.state.borrow();
        self.router
            .borrow()
            .choice_scope
            .as_ref()
            .is_some_and(|scope| {
                scope.backend == state.backend
                    && scope.thread == state.selected.as_ref().map(|thread| thread.id.clone())
                    && scope.generation == state.socket_generation
            })
    }
    pub(super) fn router_target_control(&self) {
        let current = self.router_selected();
        self.router_file_button.set_visible(current);
        self.router_target.set_visible(current);
        if !self.router_picker_current() {
            let panel = self.router_picker.borrow().clone();
            if let Some(panel) = panel {
                panel.dialog.close();
            }
        }
        let key = self.composer_key().unwrap_or_default();
        let state = self.state.borrow();
        let book = self.router.borrow();
        self.router_file_button.set_sensitive(
            current
                && !state.loading
                && !state.archived
                && !state.session_mutation
                && book.files_request.is_none(),
        );
        let selected = book.targets.get(&key);
        if let Some(target) = selected {
            crate::i18n_ui::literal(
                &self.router_target_label,
                "label",
                &format!("{} · {}", target.title, target.backend),
            );
            crate::i18n_ui::bind_format(
                &self.router_target,
                "tooltip-text",
                "Send directly to {title}\n{key}\n{cwd}",
                &[
                    ("title", &target.title),
                    ("key", &target.session_key),
                    ("cwd", &target.cwd),
                ],
            );
        } else {
            crate::i18n_ui::bind(&self.router_target_label, "label", "Automatic routing");
            crate::i18n_ui::bind(
                &self.router_target,
                "tooltip-text",
                "Let Thread Router choose a suitable session",
            );
        }
        self.router_target.set_sensitive(
            current
                && book.pending.is_none()
                && book.input_pending.is_none()
                && !book.submitting
                && !state.session_mutation
                && !state.archived
                && !state.loading
                && book
                    .registry
                    .as_ref()
                    .and_then(|registry| registry.active())
                    .as_deref()
                    == Some(key.as_str()),
        );
    }
    fn router_choose_target(self: &Rc<Self>) {
        if !self.router_target.is_sensitive() || self.window.visible_dialog().is_some() {
            return;
        }
        let Some(key) = self.composer_key().filter(|key| self.router_managed(key)) else {
            return;
        };
        let (picker_id, selected) = {
            let state = self.state.borrow();
            let mut book = self.router.borrow_mut();
            book.choice_scope = Some(NavigationScope {
                backend: state.backend.clone(),
                thread: state.selected.as_ref().map(|thread| thread.id.clone()),
                generation: state.socket_generation,
            });
            book.serial += 1;
            let picker_id = book.serial;
            book.picker_id = Some(picker_id);
            (picker_id, book.targets.get(&key).cloned())
        };
        let weak = Rc::downgrade(self);
        let pick_weak = weak.clone();
        let panel = crate::router_picker_ui::Picker::new(
            key.clone(),
            selected,
            move |request| {
                if let Some(ui) = weak.upgrade() {
                    ui.router_picker_request(picker_id, request);
                }
            },
            move |target, revision| {
                let Some(ui) = pick_weak.upgrade() else {
                    return false;
                };
                if !ui.router_picker_current() || ui.composer_key().as_deref() != Some(key.as_str())
                {
                    return false;
                }
                {
                    let book = ui.router.borrow();
                    if book.picker_id != Some(picker_id)
                        || book.pending.is_some()
                        || book.submitting
                        || book
                            .registry
                            .as_ref()
                            .and_then(|registry| registry.active())
                            .as_deref()
                            != Some(key.as_str())
                    {
                        return false;
                    }
                    if target.is_some()
                        && book
                            .registry
                            .as_ref()
                            .is_none_or(|registry| Some(&registry.revision) != revision.as_ref())
                    {
                        return false;
                    }
                }
                if let Some(target) = &target {
                    if let Err(error) = ui.connect_reference_backend(&target.backend) {
                        ui.error(&error);
                        return false;
                    }
                }
                {
                    let mut book = ui.router.borrow_mut();
                    if let Some(target) = target {
                        book.targets.insert(key.clone(), target);
                    } else {
                        book.targets.remove(&key);
                    }
                }
                ui.router_target_control();
                ui.composer_tools.close();
                ui.references_changed();
                true
            },
        );
        self.router_picker.replace(Some(panel.clone()));
        let weak = Rc::downgrade(self);
        let panel_weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let (Some(ui), Some(panel)) = (weak.upgrade(), panel_weak.upgrade()) {
                let same = ui
                    .router_picker
                    .borrow()
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &panel));
                if same {
                    ui.router_picker.replace(None);
                    let mut book = ui.router.borrow_mut();
                    book.choice_scope = None;
                    book.picker_request = None;
                    book.picker_id = None;
                }
            }
        });
        panel.dialog.present(Some(&self.window));
        panel.load();
    }
    fn router_picker_request(&self, picker_id: u64, request: Request) {
        if self.router.borrow().picker_id != Some(picker_id) || !self.router_picker_current() {
            return;
        }
        let serial = {
            let mut book = self.router.borrow_mut();
            book.serial += 1;
            let serial = book.serial;
            book.picker_request = Some(serial);
            serial
        };
        if !self.command(Command::Router { serial, request }) {
            self.router.borrow_mut().picker_request = None;
            let panel = self.router_picker.borrow().clone();
            if let Some(panel) = panel {
                panel.event(Err(
                    "Native workers are busy; refresh the target list".into()
                ));
            }
        }
    }
    pub(super) fn router_managed(&self, key: &str) -> bool {
        self.router
            .borrow()
            .registry
            .as_ref()
            .is_some_and(|r| r.contains(key))
    }
    pub(super) fn router_focus(&self, key: &str) {
        self.finder.close();
        self.router_bar.bind(key);
        let mut book = self.router.borrow_mut();
        if book
            .view
            .as_ref()
            .is_some_and(|(spec, _)| spec.controller != key)
        {
            book.view = None;
            book.jobs.clear();
            book.stamps.clear();
        }
        book.page = None;
        book.sync = None;
        book.next_poll = None;
        book.dwell.clear();
    }
    pub(super) fn router_selected(&self) -> bool {
        self.composer_key()
            .is_some_and(|key| self.router_managed(&key))
    }
    pub(super) fn router_tick(self: &Rc<Self>) {
        self.router_navigation_tick();
        self.router_confirm_visible(false);
        if self.state.borrow().backends.is_empty() {
            return;
        }
        let focus = self
            .composer_key()
            .filter(|key| self.router_managed(key))
            .map(|key| {
                self.router
                    .borrow()
                    .view
                    .as_ref()
                    .filter(|(spec, _)| spec.controller == key)
                    .map(|(spec, _)| spec.clone())
                    .unwrap_or_else(|| PageSpec::latest(key))
            });
        let (serial, known) = {
            let mut book = self.router.borrow_mut();
            if book.page.is_some()
                || book.sync.is_some()
                || book.next_poll.is_some_and(|next| next > Instant::now())
            {
                return;
            }
            book.serial += 1;
            let serial = book.serial;
            book.sync = Some(serial);
            (serial, book.stamps.clone())
        };
        if !self.command(Command::Router {
            serial,
            request: Request::Sync {
                known,
                focus,
                tracking: self.router_queue_tracking(),
            },
        }) {
            let mut book = self.router.borrow_mut();
            book.sync = None;
            book.next_poll = Some(Instant::now() + Duration::from_secs(5));
        }
    }
    pub(super) fn router_event(self: &Rc<Self>, serial: u64, result: Result<Reply, String>) {
        let files = self
            .router
            .borrow()
            .files_request
            .clone()
            .filter(|(pending, _)| *pending == serial);
        if let Some((_, scope)) = files {
            self.router.borrow_mut().files_request = None;
            if self.router_scope() == scope && self.router_selected() {
                match result {
                    Ok(Reply::Files(files)) => {
                        let key = self.composer_key().unwrap_or_default();
                        let added = self.state.borrow_mut().references.add_prepared(&key, files);
                        if let Err(error) = added {
                            self.error(&error);
                        }
                    }
                    Err(error) => self.error(&error),
                    _ => self.error("Unexpected reference preparation response"),
                }
            }
            self.update_composer();
            return;
        }
        if self.router.borrow().picker_request == Some(serial) {
            self.router.borrow_mut().picker_request = None;
            if self.router_picker_current() {
                let panel = self.router_picker.borrow().clone();
                if let Some(panel) = panel {
                    panel.event(result);
                }
            }
            return;
        }
        let (sync, send, stop, seen, page, navigation) = {
            let book = self.router.borrow();
            (
                book.sync == Some(serial),
                book.send == Some(serial),
                book.stop == Some(serial),
                book.seen == Some(serial),
                book.page == Some(serial),
                matches!(&book.navigation,Some(Navigation::Lookup(lookup)) if lookup.serial==serial),
            )
        };
        if !sync && !send && !stop && !seen && !page && !navigation {
            return;
        }
        #[cfg(debug_assertions)]
        let result = {
            let drop_ack = {
                let mut book = self.router.borrow_mut();
                send && matches!(&result, Ok(Reply::Started(_)))
                    && book
                        .pending
                        .as_ref()
                        .is_some_and(|(_, _, _, queued)| *queued)
                    && std::mem::take(&mut book.drop_queued_ack_once)
            };
            if drop_ack {
                Err("Private queued HTTP acknowledgement loss".into())
            } else {
                result
            }
        };
        let confirmed = matches!(&result, Ok(Reply::Started(_)));
        let observed = match &result {
            Ok(Reply::Sync { tracking, .. }) => tracking.clone(),
            Ok(Reply::Started(display)) => vec![crate::router::Tracked {
                controller: display.job.controller.clone(),
                id: display.job.id.clone(),
                status: Some(display.job.status.clone()),
                target_turn_id: display.job.target_turn_id.clone(),
            }],
            _ => Vec::new(),
        };
        self.router_queue_observe(&observed, confirmed);
        self.router_observe_inputs(&observed);
        if navigation {
            let same = {
                let book = self.router.borrow();
                let state = self.state.borrow();
                matches!(&book.navigation,Some(Navigation::Lookup(lookup)) if lookup.scope.backend==state.backend && lookup.scope.thread==state.selected.as_ref().map(|t|t.id.clone()) && lookup.scope.generation==state.socket_generation && state.items.iter().any(|item|Arc::ptr_eq(item,&lookup.item)))
            };
            if !same {
                self.router.borrow_mut().navigation = None;
                return;
            }
        }
        let was_router = self.router_selected();
        let registry_was_unknown = self.router.borrow().registry.is_none();
        {
            let mut book = self.router.borrow_mut();
            if sync {
                book.sync = None;
                book.next_poll = Some(Instant::now() + Duration::from_secs(2));
            }
            if send {
                book.send = None;
                book.submitting = false;
            }
            if page {
                book.page = None;
            }
            if seen {
                book.seen = None;
            }
            if stop {
                book.stop = None;
                book.stopping = false;
            }
        }
        let alerts = {
            let mut book = self.router.borrow_mut();
            match &result {
                Ok(Reply::Sync {
                    registry,
                    histories,
                    tracking,
                    ..
                }) => book.notices.sync(registry, histories, tracking),
                Ok(Reply::Started(_)) => observed
                    .iter()
                    .filter_map(|row| book.notices.observe(row, true))
                    .collect(),
                _ => Vec::new(),
            }
        };
        for alert in alerts {
            if let Some((backend, thread)) = alert.controller.split_once(':') {
                let body = self.notification_caption(backend, thread);
                match alert.kind {
                    crate::router_notifications::Kind::Clarify => {
                        self.notifications.approval(backend, thread, &body);
                    }
                    kind => self.notifications.completion(
                        backend,
                        thread,
                        &body,
                        kind == crate::router_notifications::Kind::Failed,
                    ),
                }
            }
        }
        match result {
            Ok(Reply::Choices(_) | Reply::ChoicePage(_) | Reply::Files(_)) => return,
            Ok(Reply::Target { thread, origin }) => {
                self.router_target_ready(serial, thread, origin);
                return;
            }
            Ok(Reply::Sync {
                registry,
                histories,
                changed,
                stamps,
                view,
                tracking: _,
            }) => {
                let mut book = self.router.borrow_mut();
                book.error = None;
                if let Some((body, _, _, _)) = &mut book.pending {
                    body["expectedRevision"] = registry.revision.clone().into();
                }
                book.targets.retain(|key, _| registry.contains(key));
                book.registry = Some(registry);
                book.histories = histories;
                book.stamps = stamps;
                if let Some((spec, history)) = view.filter(|(spec, _)| {
                    self.composer_key().as_deref() == Some(spec.controller.as_str())
                        && self.router_bar.matches(spec)
                }) {
                    if book
                        .view
                        .as_ref()
                        .is_none_or(|(old, _)| old.controller != spec.controller)
                    {
                        book.scroll = Some(true);
                    }
                    book.view = Some((spec, history));
                }
                for display in changed {
                    book.jobs.insert(display.job.id.clone(), display);
                }
                let retained: HashSet<_> = book
                    .view
                    .as_ref()
                    .into_iter()
                    .flat_map(|(_, h)| h.ids.iter().cloned())
                    .collect();
                book.jobs.retain(|id, _| retained.contains(id));
            }
            Ok(Reply::Started(display)) => {
                let mut book = self.router.borrow_mut();
                let id = display.job.id.clone();
                let controller = display.job.controller.clone();
                if let Some(history) = book
                    .histories
                    .iter_mut()
                    .find(|h| h.controller == controller)
                {
                    if !history.ids.contains(&id) {
                        history.ids.insert(0, id.clone());
                        history.ids.truncate(20);
                    }
                } else {
                    book.histories.push(History {
                        statuses: Vec::new(),
                        controller: controller.clone(),
                        ids: vec![id.clone()],
                        next: None,
                        newer: None,
                        unread_count: 0,
                        active_ids: Vec::new(),
                        controller_busy: true,
                    });
                }
                if let Some(history) = book
                    .histories
                    .iter_mut()
                    .find(|h| h.controller == display.job.controller)
                {
                    if !display.job.terminal() && !history.active_ids.contains(&id) {
                        history.active_ids.insert(0, id.clone());
                    }
                    history.controller_busy =
                        matches!(display.job.status.as_str(), "routing" | "dispatching");
                }
                if self.composer_key().as_deref() == Some(controller.as_str()) {
                    if let Some(history) = book
                        .histories
                        .iter()
                        .find(|h| h.controller == controller)
                        .cloned()
                    {
                        book.view = Some((PageSpec::latest(controller.clone()), history));
                    }
                    book.page = None;
                    book.scroll = Some(true);
                }
                book.jobs.insert(id, display);
                book.sync = None;
                book.next_poll = None;
                drop(book);
                if self.composer_key().as_deref() == Some(controller.as_str()) {
                    self.router_bar.current();
                    self.router_bar.reset_filter();
                }
            }
            Ok(Reply::Page {
                spec,
                history,
                changed,
                stamps,
            }) => {
                if self.composer_key().as_deref() == Some(spec.controller.as_str())
                    && self.router_bar.matches(&spec)
                {
                    let mut book = self.router.borrow_mut();
                    book.jobs.clear();
                    for display in changed {
                        book.jobs.insert(display.job.id.clone(), display);
                    }
                    book.stamps = stamps;
                    book.scroll = Some(spec.before.is_none() && spec.after.is_none());
                    book.view = Some((spec, history));
                    book.sync = None;
                    book.next_poll = None;
                }
            }
            Ok(Reply::Rejected(error)) => {
                self.router_queue_rejected(&error);
                self.router_finish_inputs(false);
                let mut book = self.router.borrow_mut();
                book.pending = None;
                book.queued_input = None;
                book.next_poll = None;
                drop(book);
                self.error(&error);
            }
            Ok(Reply::Seen {
                controller,
                acknowledged,
            }) => {
                let mut book = self.router.borrow_mut();
                for ack in acknowledged {
                    if let Some(display) = book
                        .jobs
                        .get_mut(&ack.id)
                        .filter(|d| d.job.controller == controller && d.job.receipt == ack.receipt)
                    {
                        display.job.unread = false;
                    }
                    book.stamps.remove(&ack.id);
                }
                book.sync = None;
                book.next_poll = None;
            }
            Ok(Reply::Stopped) => {
                self.router.borrow_mut().next_poll = None;
            }
            Err(error) => {
                let mut book = self.router.borrow_mut();
                if navigation {
                    book.navigation = None;
                }

                if book.error.as_deref() != Some(&error) {
                    self.error(&error);
                }
                book.error = Some(error);
                if seen {
                    book.next_seen = Some(Instant::now() + Duration::from_secs(5));
                }
                if sync {
                    book.next_poll = Some(Instant::now() + Duration::from_secs(5));
                }
            }
        }
        // Direct sends may reconcile from durable status. Queued snapshots need
        // a matching POST acknowledgement; retries retain their original UUID.
        let accepted = {
            let book = self.router.borrow();
            book.pending.as_ref().is_some_and(|(body, _, _, queued)| {
                if *queued && !confirmed {
                    return false;
                }
                body["requestId"].as_str().is_some_and(|id| {
                    book.jobs.contains_key(id)
                        || observed.iter().any(|row| {
                            row.id == id
                                && Some(row.controller.as_str()) == body["controller"].as_str()
                                && row.status.is_some()
                        })
                        || book.histories.iter().any(|history| {
                            history.controller == body["controller"].as_str().unwrap_or("")
                                && history.ids.iter().any(|saved| saved == id)
                        })
                })
            })
        };
        if accepted {
            let pending = {
                let mut book = self.router.borrow_mut();
                book.queued_input = None;
                book.pending.take()
            };
            if let Some((_, key, text, queued)) = pending {
                let images_waiting = self
                    .router
                    .borrow()
                    .input_pending
                    .as_ref()
                    .is_some_and(|input| input.key == key);
                let selected = self.composer_key().as_deref() == Some(key.as_str());
                if !queued && !images_waiting && selected && self.composer_text() == text {
                    self.composer.buffer().set_text("");
                }
                let mut state = self.state.borrow_mut();
                if !queued && !images_waiting && state.drafts.get(&key) == Some(&text) {
                    state.drafts.remove(&key);
                }
            }
        }
        self.router_attention();
        self.router_queue_gates();
        if self.router_selected() {
            if registry_was_unknown || !was_router {
                let selected = self.state.borrow().selected.clone();
                if let Some(thread) = selected {
                    self.read_session(&thread.id, false);
                }
            }
            self.router_render();
        } else if was_router || (registry_was_unknown && self.router.borrow().registry.is_some()) {
            let selected = self.state.borrow().selected.clone();
            if let Some(thread) = selected {
                let resume = !self.state.borrow().archived;
                self.read_session(&thread.id, resume);
            }
        }
        self.update_composer();
    }
    fn router_attention(&self) {
        let book = self.router.borrow();
        let mut state = self.state.borrow_mut();
        for history in &book.histories {
            let activity = state
                .activity
                .entry(history.controller.clone())
                .or_default();
            activity.active = !history.active_ids.is_empty();
            activity.attention = history.unread_count > 0;
            activity.stale = false;
        }
        drop(state);
        drop(book);
        self.render_sessions();
    }
    pub(super) fn router_render(&self) {
        let Some(key) = self.composer_key() else {
            return;
        };
        let (items, loaded, older, newer, historical, busy, unread, filtered, count, applied) = {
            let book = self.router.borrow();
            let view = book
                .view
                .as_ref()
                .filter(|(spec, _)| spec.controller == key);
            let items = view
                .into_iter()
                .flat_map(|(_, h)| h.ids.iter().rev())
                .filter_map(|id| book.jobs.get(id))
                .flat_map(|display| display.items.iter().cloned())
                .collect::<Vec<_>>();
            let unread = if self.router_bar.browser() {
                0
            } else {
                book.histories
                    .iter()
                    .find(|h| h.controller == key)
                    .map_or(0, |h| h.unread_count)
            };
            (
                items,
                view.is_some(),
                view.is_some_and(|(_, h)| h.next.is_some()),
                view.is_some_and(|(_, h)| h.newer.is_some()),
                view.is_some_and(|(spec, _)| spec.before.is_some() || spec.after.is_some()),
                book.page.is_some(),
                unread,
                view.is_some_and(|(spec, _)| !spec.query.is_empty()),
                view.map_or(0, |(_, h)| h.ids.len()),
                view.is_some_and(|(spec, _)| self.router_bar.matches(spec)),
            )
        };
        let mut state = self.state.borrow_mut();
        state.loading = !loaded;
        state.active_turn = None;
        state.sending = false;
        if state.items != items {
            state.items = items;
            model::trim_items(&mut state.items);
            state.render_dirty = true;
        }
        drop(state);
        self.router_bar.widget.set_visible(true);
        self.router_bar
            .older
            .set_sensitive(loaded && older && !busy && applied);
        self.router_bar
            .newer
            .set_sensitive(loaded && newer && !busy && applied);
        self.router_bar
            .latest
            .set_sensitive((self.router_bar.browser() || newer || historical) && !busy);
        self.router_bar
            .seen
            .set_sensitive(!self.router_bar.browser() && loaded && unread > 0);
        crate::i18n_ui::bind(
            &self.router_bar.latest,
            "tooltip-text",
            if self.router_bar.browser() {
                "Refresh browser history"
            } else {
                "Latest Router requests"
            },
        );
        crate::i18n_ui::bind(
            &self.router_bar.search,
            "tooltip-text",
            if self.router_bar.browser() {
                "Search saved routing metadata. Source replies load when their page opens."
            } else {
                "Literal text in saved requests, replies and target details. Spaces are significant."
            },
        );
        let caption = if busy {
            "Loading history…"
        } else if loaded && !applied {
            "Search not applied"
        } else if filtered {
            "Search results"
        } else if self.router_bar.browser() {
            "Earlier browser requests"
        } else if newer {
            "Earlier requests"
        } else {
            "Latest requests"
        };
        let mut parts = vec![crate::i18n_ui::Part::message(caption, &[])];
        if filtered {
            parts.push(if count == 1 {
                crate::i18n_ui::Part::message("1 request", &[])
            } else {
                crate::i18n_ui::Part::message("{count} requests", &[("count", &count.to_string())])
            });
        }
        parts.push(crate::i18n_ui::Part::message(
            "{count} unread",
            &[("count", &unread.to_string())],
        ));
        crate::i18n_ui::bind_parts(&self.router_bar.label, "label", parts, " · ");
        crate::i18n_ui::bind(
            &self.status,
            "label",
            if self.router_bar.browser() {
                "Read-only browser history"
            } else if loaded {
                "Thread Router"
            } else {
                "Loading Router history…"
            },
        );
        self.sync_turn_unread();
    }
    pub(super) fn sync_turn_unread(&self) {
        let context = {
            let state = self.state.borrow();
            state
                .selected
                .as_ref()
                .map(|thread| crate::session_search::Context {
                    backend: state.backend.clone(),
                    thread: thread.id.clone(),
                    generation: state.socket_generation,
                })
        };
        let Some(context) = context else { return };
        let key = format!("{}:{}", context.backend, context.thread);
        let unread = if self.router_selected() && !self.router_bar.browser() {
            let book = self.router.borrow();
            book.view
                .as_ref()
                .filter(|(spec, _)| spec.controller == key && self.router_bar.matches(spec))
                .into_iter()
                .flat_map(|(_, history)| history.ids.iter())
                .filter_map(|id| book.jobs.get(id))
                .filter(|display| {
                    display.job.controller == key
                        && !display.job.browser_history
                        && display.job.unread
                        && display.job.terminal()
                        && display.job.receipt.len() == 32
                })
                .flat_map(|display| {
                    display
                        .items
                        .iter()
                        .filter(|item| item.role == "You")
                        .map(|item| item.turn_id.clone())
                })
                .collect()
        } else {
            std::collections::HashSet::new()
        };
        self.turn_navigation.set_unread(&context, unread);
    }
    pub(super) fn connect_router_bar(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.router_bar.source.connect_selected_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_search_changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.router_bar
            .search_toggle
            .connect_toggled(move |button| {
                let Some(ui) = weak.upgrade() else { return };
                if ui.router_bar.updating.get() {
                    return;
                }
                ui.router_bar.search_bar.set_search_mode(button.is_active());
                if button.is_active() {
                    ui.router_bar.search.grab_focus();
                } else {
                    ui.router_bar.reset_filter();
                    ui.router_search_changed();
                }
            });
        let weak = Rc::downgrade(self);
        self.router_bar.search.connect_search_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_search_changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.router_bar.search.connect_activate(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_search_changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.router_bar.search.connect_stop_search(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_bar.search_toggle.set_active(false);
            }
        });
        let weak = Rc::downgrade(self);
        self.router_bar.case.connect_toggled(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_search_changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.router_file_button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_choose_reference_files();
            }
        });
        let weak = Rc::downgrade(self);
        self.router_target.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_choose_target();
            }
        });
        let weak = Rc::downgrade(self);
        self.router_back.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_return();
            }
        });
        for (button, direction) in [
            (&self.router_bar.older, -1),
            (&self.router_bar.newer, 1),
            (&self.router_bar.latest, 0),
        ] {
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.router_page(direction);
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.router_bar.seen.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.router_mark_visible_read();
            }
        });
    }
    fn router_page(&self, direction: i32) {
        let Some(controller) = self.composer_key().filter(|key| self.router_managed(key)) else {
            return;
        };
        let (serial, spec) = {
            let (query, case_sensitive) = self.router_bar.filter();
            let mut book = self.router.borrow_mut();
            if book.page.is_some() {
                return;
            }
            let view = book
                .view
                .as_ref()
                .filter(|(spec, _)| spec.controller == controller && self.router_bar.matches(spec))
                .map(|(_, h)| h);
            let (before, after) = match direction {
                -1 => {
                    let Some(cursor) = view.and_then(|h| h.next) else {
                        return;
                    };
                    (Some(cursor), None)
                }
                1 => {
                    let Some(cursor) = view.and_then(|h| h.newer) else {
                        return;
                    };
                    (None, Some(cursor))
                }
                _ => (None, None),
            };
            book.serial += 1;
            let serial = book.serial;
            book.page = Some(serial);
            book.sync = None;
            (
                serial,
                PageSpec {
                    browser: self.router_bar.browser(),
                    controller,
                    before,
                    after,
                    query,
                    case_sensitive,
                },
            )
        };
        if !self.command(Command::Router {
            serial,
            request: Request::Page(spec),
        }) {
            self.router.borrow_mut().page = None;
        }
        self.router_render();
    }
    fn router_search_changed(&self) {
        if self.router_bar.updating.get() {
            return;
        }
        let Some(controller) = self.composer_key().filter(|key| self.router_managed(key)) else {
            return;
        };
        let (query, case_sensitive) = self.router_bar.filter();
        if query.len() > 2048 || query.chars().count() > 512 {
            self.error("Router search supports at most 512 characters");
            return;
        }
        let serial = {
            let mut book = self.router.borrow_mut();
            if book.view.as_ref().is_some_and(|(spec, _)| {
                spec.controller == controller
                    && spec.browser == self.router_bar.browser()
                    && spec.query == query
                    && spec.case_sensitive == case_sensitive
            }) && book.page.is_none()
            {
                return;
            }
            if query.is_empty() && book.view.is_none() && !self.router_bar.browser() {
                return;
            }
            book.serial += 1;
            let serial = book.serial;
            book.page = Some(serial);
            book.sync = None;
            serial
        };
        let spec = PageSpec {
            browser: self.router_bar.browser(),
            controller,
            before: None,
            after: None,
            query,
            case_sensitive,
        };
        if !self.command(Command::Router {
            serial,
            request: Request::Page(spec),
        }) {
            self.router.borrow_mut().page = None;
        }
        self.router_render();
    }
    pub(super) fn router_submit(self: &Rc<Self>) {
        let retry = self.router.borrow().pending.is_some();
        if !retry && self.router_files_preparing() {
            self.error("Wait for reference files to finish preparing before sending");
            return;
        }
        let message = self.composer_text();
        if !retry {
            if let Some(name) = crate::command_tools::slash(&message) {
                if let Err(error) = self.run_command(name, Some(message.clone())) {
                    self.error(&error);
                }
                return;
            }
        }
        let Some(key) = self.composer_key() else {
            return;
        };
        let state = self.state.borrow();
        if !retry && crate::command_tools::shell(&message).is_some() {
            self.error("Open the target conversation to run a shell command");
            return;
        }
        if !state.options_loaded || state.archived || state.session_mutation {
            return;
        }
        let mut book = self.router.borrow_mut();
        if book.submitting || book.input_pending.is_some() && book.pending.is_none() {
            return;
        }
        let Some(registry) = &book.registry else {
            return;
        };
        if registry.active().as_deref() != Some(key.as_str()) && !book.pending_for(&key) {
            self.error("This controller is inactive; select the active Thread Router configured in Router settings");
            return;
        }
        let body = if let Some((body, _, _, _)) = &book.pending {
            body.clone()
        } else {
            if message.len() > 64 * 1024 {
                self.error("Router messages are limited to 64 KiB");
                return;
            }
            let images = state.images.images(&key);
            let references = state.references.get(&key);
            if state.images.pending.contains_key(&key) {
                return;
            }
            if message.trim().is_empty() && images.is_empty() && references.is_empty() {
                return;
            }
            let explicit = book
                .targets
                .get(&key)
                .map(|target| json!({"sessionKey":target.session_key,"cwd":target.cwd}));
            let body = json!({"requestId":glib::uuid_string_random().to_string(),"controller":key,"expectedRevision":registry.revision,"prompt":message,"options":state.options,"policies":state.policies.router_snapshot(),"explicitTarget":explicit});
            let annotation = self.annotation_prepared.borrow_mut().take();
            if !images.is_empty() || !references.is_empty() || annotation.is_some() {
                book.input_pending = Some(PendingInput {
                    annotation,
                    key: key.clone(),
                    id: body["requestId"].as_str().unwrap().into(),
                    text: message.clone(),
                    images,
                    references,
                });
            }
            book.pending = Some((body.clone(), key.clone(), message, false));
            body
        };
        if body["controller"].as_str() != Some(key.as_str()) {
            self.error(
                "Resolve the previous Router delivery in its conversation before starting another",
            );
            return;
        }
        book.serial += 1;
        let serial = book.serial;
        book.send = Some(serial);
        book.submitting = true;
        book.navigation = None;
        let images = book
            .input_pending
            .as_ref()
            .filter(|pending| Some(pending.id.as_str()) == body["requestId"].as_str())
            .map(|pending| pending.images.clone())
            .unwrap_or_default();
        let references = book
            .input_pending
            .as_ref()
            .filter(|pending| Some(pending.id.as_str()) == body["requestId"].as_str())
            .map(|pending| pending.references.clone())
            .unwrap_or_default();
        let queued_references = book
            .queued_input
            .as_ref()
            .filter(|message| Some(message.id.as_str()) == body["queuedMessageId"].as_str())
            .map(|message| message.input.clone());
        drop(book);
        drop(state);
        self.supervision_human(&key);
        if !images.is_empty() {
            self.state.borrow_mut().images.sending(
                &key,
                &images
                    .iter()
                    .map(|image| image.id.clone())
                    .collect::<Vec<_>>(),
            );
        }
        if let Some(id) = body["requestId"].as_str() {
            let mut state = self.state.borrow_mut();
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                entry.router_wait = Some(id.into());
                entry.active = true;
                entry.released = false;
                entry.touch();
            }
        }
        if !self.command(Command::Router {
            serial,
            request: Request::Start {
                body,
                images,
                references,
                queued_references,
            },
        }) {
            let mut book = self.router.borrow_mut();
            book.submitting = false;
            book.send = None;
            let unsent_comments = !retry
                && book
                    .input_pending
                    .as_ref()
                    .is_some_and(|input| input.annotation.is_some());
            drop(book);
            if unsent_comments {
                self.router_queue_rejected(
                    "The Router request was not queued; comments and inputs are retained",
                );
                self.router.borrow_mut().pending = None;
                self.router_finish_inputs(false);
            }
        }
        self.update_composer();
    }
    pub(super) fn router_stop(&self) {
        self.pause_selected_queue(
            "Stopped. Check the Router reply before resuming queued messages.",
        );
        let Some(key) = self.composer_key() else {
            return;
        };
        let mut book = self.router.borrow_mut();
        if book.stopping {
            return;
        }
        let id = book
            .histories
            .iter()
            .find(|h| h.controller == key)
            .and_then(|h| h.active_ids.first())
            .cloned();
        let Some(id) = id else {
            return;
        };
        book.serial += 1;
        let serial = book.serial;
        book.stop = Some(serial);
        book.stopping = true;
        drop(book);
        if !self.command(Command::Router {
            serial,
            request: Request::Stop(id),
        }) {
            let mut book = self.router.borrow_mut();
            book.stop = None;
            book.stopping = false;
        }
        self.update_composer();
    }
    pub(super) fn router_mark_visible_read(self: &Rc<Self>) {
        self.router_confirm_visible(true);
    }
    fn router_confirm_visible(&self, explicit: bool) {
        if !self.router_selected()
            || self.state.borrow().render_dirty
            || !self.conversation.is_visible()
            || !self.transcript_view.is_mapped()
            || self.window.visible_dialog().is_some()
            || (!explicit && !self.window.is_active())
        {
            self.router.borrow_mut().dwell.clear();
            return;
        }
        let now = Instant::now();
        {
            let book = self.router.borrow();
            if book.seen.is_some() || (!explicit && book.next_seen.is_some_and(|next| next > now)) {
                return;
            }
        }
        let Some(controller) = self.composer_key() else {
            return;
        };
        let ends = self
            .chat_visibility
            .visible_ends(self.transcript_view.upcast_ref());
        let (serial, seen) = {
            let mut book = self.router.borrow_mut();
            book.next_seen = Some(now + Duration::from_millis(250));
            let candidates = book
                .jobs
                .values()
                .filter(|display| {
                    display.job.controller == controller
                        && !display.job.browser_history
                        && display.job.unread
                        && display.job.terminal()
                        && display.job.receipt.len() == 32
                        && display.items.last().is_some_and(|item| {
                            ends.contains(&(item.id.clone(), item.turn_id.clone()))
                        })
                })
                .map(|display| Receipt {
                    id: display.job.id.clone(),
                    receipt: display.job.receipt.clone(),
                })
                .collect::<Vec<_>>();
            book.dwell.retain(|key, _| {
                candidates
                    .iter()
                    .any(|c| c.id == key.0 && c.receipt == key.1)
            });
            let seen = candidates
                .into_iter()
                .filter(|receipt| {
                    let since = book
                        .dwell
                        .entry((receipt.id.clone(), receipt.receipt.clone()))
                        .or_insert(now);
                    explicit || now.duration_since(*since) >= Duration::from_millis(500)
                })
                .take(20)
                .collect::<Vec<_>>();
            if seen.is_empty() {
                return;
            }
            book.serial += 1;
            let serial = book.serial;
            book.seen = Some(serial);
            (serial, seen)
        };
        if !self.command(Command::Router {
            serial,
            request: Request::Seen { controller, seen },
        }) {
            self.router.borrow_mut().seen = None;
        }
    }
    fn navigation_scope(&self) -> NavigationScope {
        let state = self.state.borrow();
        NavigationScope {
            backend: state.backend.clone(),
            thread: state.selected.as_ref().map(|t| t.id.clone()),
            generation: state.socket_generation,
        }
    }
    pub(super) fn router_open_target(self: &Rc<Self>, item: &Arc<Item>) {
        let Some(origin) = &item.origin else {
            return;
        };
        let (kind, generation) = {
            let state = self.state.borrow();
            if state.loading
                || state.render_dirty
                || state.session_mutation
                || !state.items.iter().any(|visible| Arc::ptr_eq(visible, item))
            {
                self.error("Wait for the current reply view to finish loading");
                return;
            }
            let Some(backend) = state.backends.iter().find(|b| {
                b.id == origin.backend && matches!(b.kind.as_str(), "codex" | "opencode")
            }) else {
                self.error("The target backend is no longer configured");
                return;
            };
            (
                backend.kind.clone(),
                state.backend_generations.get(&origin.backend).copied(),
            )
        };
        let scope = self.navigation_scope();
        let serial = {
            let mut book = self.router.borrow_mut();
            book.serial += 1;
            let serial = book.serial;
            book.navigation = Some(Navigation::Lookup(Lookup {
                serial,
                scope,
                item: item.clone(),
                origin: origin.clone(),
                target_generation: generation,
                kind: kind.clone(),
            }));
            serial
        };
        if !self.command(Command::Router {
            serial,
            request: Request::Target {
                kind,
                origin: origin.clone(),
            },
        }) {
            self.router.borrow_mut().navigation = None;
        }
    }
    fn router_target_ready(
        self: &Rc<Self>,
        serial: u64,
        thread: Thread,
        origin: Arc<model::Origin>,
    ) {
        let pending = self.router.borrow_mut().navigation.take();
        let Some(Navigation::Lookup(lookup)) = pending else {
            return;
        };
        if lookup.serial != serial || lookup.scope != self.navigation_scope() {
            return;
        }
        let valid = {
            let state = self.state.borrow();
            !state.session_mutation
                && state
                    .items
                    .iter()
                    .any(|visible| Arc::ptr_eq(visible, &lookup.item))
                && state.backend_generations.get(&origin.backend).copied()
                    == lookup.target_generation
                && state
                    .backends
                    .iter()
                    .any(|b| b.id == origin.backend && b.kind == lookup.kind)
        };
        if !valid || lookup.origin != origin || self.window.visible_dialog().is_some() {
            self.error("The session or connection changed; open the target again from its reply");
            return;
        }
        let back = {
            let state = self.state.borrow();
            state.selected.clone().map(|source| ReturnTo {
                backend: state.backend.clone(),
                thread: source,
                target_key: format!("{}:{}", origin.backend, origin.thread),
            })
        };
        self.router.borrow_mut().return_to = back;
        if !self.router_select_reference(&origin.backend, thread) {
            return;
        }
        let generation = self.state.borrow().socket_generation;
        self.router.borrow_mut().navigation = Some(Navigation::Opening(Opening {
            backend: origin.backend.clone(),
            thread: origin.thread.clone(),
            generation,
            origin,
            started: Instant::now(),
        }));
        self.update_composer();
    }
    fn router_select_reference(self: &Rc<Self>, backend_id: &str, thread: Thread) -> bool {
        let target = {
            let state = self.state.borrow();
            if state.session_mutation {
                return false;
            }
            state
                .backends
                .iter()
                .enumerate()
                .find(|(_, b)| b.id == backend_id)
                .map(|(index, b)| (index, b.clone()))
        };
        let Some((index, backend)) = target else {
            self.error("The destination backend is no longer configured");
            return false;
        };
        self.scope.set_selected(0);
        self.search.set_text("");
        self.state.borrow_mut().reference_selection = Some(format!("{backend_id}:{}", thread.id));
        let same_backend = self.state.borrow().backend == backend.id;
        if same_backend {
            {
                let mut state = self.state.borrow_mut();
                if !state.threads.iter().any(|t| t.id == thread.id) {
                    state.threads.insert(0, thread.clone());
                }
            }
            self.select(thread);
            self.render_sessions();
        } else {
            self.connect_backend_to(backend, false, Some(thread));
            self.backend.set_selected(index as u32);
        }
        true
    }
    pub(super) fn router_back_visibility(&self) {
        let book = self.router.borrow();
        let state = self.state.borrow();
        let visible = book.return_to.as_ref().is_some_and(|back| {
            back.target_key
                .split_once(':')
                .is_some_and(|(backend, id)| {
                    state.backend == backend
                        && state.selected.as_ref().is_none_or(|thread| thread.id == id)
                })
        });
        self.router_back.set_visible(visible);
        self.router_back.set_sensitive(!state.session_mutation);
    }
    fn router_return(self: &Rc<Self>) {
        if self.state.borrow().session_mutation {
            return;
        }
        let back = {
            let mut book = self.router.borrow_mut();
            book.navigation = None;
            book.return_to.take()
        };
        if let Some(back) = back {
            self.router_select_reference(&back.backend, back.thread);
        }
        self.update_composer();
    }
    pub(super) fn router_navigation_failed(&self, id: &str) {
        let mut book = self.router.borrow_mut();
        if matches!(&book.navigation,Some(Navigation::Opening(opening)) if opening.thread==id && opening.backend==self.state.borrow().backend)
        {
            book.navigation = None;
        }
    }
    fn router_navigation_tick(self: &Rc<Self>) {
        let action = {
            let mut book = self.router.borrow_mut();
            let Some(Navigation::Opening(opening)) = &book.navigation else {
                return;
            };
            let state = self.state.borrow();
            let changed = state.backend != opening.backend
                || state.socket_generation != opening.generation
                || state
                    .selected
                    .as_ref()
                    .is_some_and(|thread| thread.id != opening.thread);
            if changed {
                book.navigation = None;
                return;
            }
            if opening.started.elapsed() > Duration::from_secs(40) {
                book.navigation = None;
                drop(state);
                drop(book);
                self.error("Opening the target timed out; reconnect or return to Router");
                return;
            }
            if !state.ready
                || state.loading
                || state.render_dirty
                || !state.options_loaded
                || state.selected.is_none()
            {
                return;
            }
            if state
                .selected
                .as_ref()
                .is_some_and(|thread| thread.cwd != opening.origin.root)
            {
                book.navigation = None;
                drop(state);
                drop(book);
                self.error("The target project directory changed while opening");
                return;
            }
            let origin = opening.origin.clone();
            book.navigation = None;
            origin
        };
        let item_id = if action.item.is_empty() {
            self.state
                .borrow()
                .items
                .iter()
                .rev()
                .find(|item| item.turn_id == action.turn && item.role == "Assistant")
                .map(|item| item.id.clone())
                .unwrap_or_default()
        } else {
            action.item.clone()
        };
        if item_id.is_empty() {
            self.composer.grab_focus();
            return;
        }
        let hit = crate::session_search::Occurrence {
            turn_id: action.turn.clone(),
            item_id,
            kind: crate::session_search::Filter::Assistant,
            snippet: String::new(),
            highlight: 0..0,
            source: None,
            block: None,
            table_row: None,
            paragraph_page: None,
            paragraph_range: None,
        };
        if let Some(row) = crate::chat_ui::locate(&self.transcript, &hit) {
            self.transcript_navigation
                .jump(&self.transcript_view, row, gtk::ListScrollFlags::NONE);
            self.composer.grab_focus();
        } else {
            let (context, serial) = {
                let mut state = self.state.borrow_mut();
                state.sequence += 1;
                (
                    crate::session_search::Context {
                        backend: action.backend.clone(),
                        thread: action.thread.clone(),
                        generation: state.socket_generation,
                    },
                    state.sequence,
                )
            };
            self.present_history_context(
                context,
                serial,
                action.title.clone(),
                action.root.clone(),
                hit,
            );
        }
    }
    pub(super) fn router_source(self: &Rc<Self>, item: &Arc<Item>) {
        let Some(origin) = &item.origin else {
            return;
        };
        if origin.turn.is_empty() || origin.item.is_empty() {
            self.error("This older routed excerpt has no original message anchor; open the target session from the sidebar");
            return;
        }
        let (context, serial) = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            (
                crate::session_search::Context {
                    backend: origin.backend.clone(),
                    thread: origin.thread.clone(),
                    generation: state
                        .backend_generations
                        .get(&origin.backend)
                        .copied()
                        .unwrap_or(0),
                },
                state.sequence,
            )
        };
        self.present_history_context(
            context,
            serial,
            origin.title.clone(),
            origin.root.clone(),
            crate::session_search::Occurrence {
                turn_id: origin.turn.clone(),
                item_id: origin.item.clone(),
                kind: crate::session_search::Filter::Assistant,
                snippet: String::new(),
                highlight: 0..0,
                source: None,
                block: None,
                table_row: None,
                paragraph_page: None,
                paragraph_range: None,
            },
        );
    }
}

#[cfg(debug_assertions)]
pub(super) async fn smoke(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<Value, String> {
    async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
        let start = Instant::now();
        while !ready() {
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Native Router UI test timed out".into());
            }
            glib::timeout_future(Duration::from_millis(25)).await;
        }
        Ok(())
    }
    fn button(widget: &gtk::Widget, tooltip: &str) -> Option<gtk::Button> {
        if let Some(button) = widget
            .downcast_ref::<gtk::Button>()
            .filter(|b| b.tooltip_text().as_deref() == Some(tooltip))
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(button) = button(&widget, tooltip) {
                return Some(button);
            }
        }
        None
    }
    fn visible(widget: &gtk::Widget, view: &gtk::Widget, text: &str) -> bool {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
            if label.is_mapped()
                && label.is_visible()
                && label.text().contains(text)
                && label
                    .compute_bounds(view)
                    .is_some_and(|b| b.y() >= 0.0 && b.y() < view.height() as f32)
            {
                return true;
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if visible(&widget, view, text) {
                return true;
            }
        }
        false
    }
    async fn reveal(ui: &Rc<Ui>, text: &str) -> Result<(), String> {
        ui.render_items();
        if let Some(last) = ui.transcript.n_items().checked_sub(1) {
            ui.transcript_view
                .scroll_to(last, gtk::ListScrollFlags::NONE, None);
        }
        until(|| {
            visible(
                ui.transcript_view.upcast_ref(),
                ui.transcript_view.upcast_ref(),
                text,
            )
        })
        .await
    }
    async fn capture_command(
        ui: &Rc<Ui>,
        text: &str,
        path: &std::path::Path,
    ) -> Result<(), String> {
        // Wait for dialog/row allocation animations, then verify against the
        // viewport rather than the ListView's potentially much taller bounds.
        glib::timeout_future(Duration::from_millis(350)).await;
        let adjustment = ui.transcript_scroll.vadjustment();
        adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
        glib::timeout_future(Duration::from_millis(250)).await;
        until(|| {
            visible(
                ui.transcript_view.upcast_ref(),
                ui.transcript_scroll.upcast_ref(),
                text,
            )
        })
        .await?;
        super::smoke::capture(&ui.window, path);
        Ok(())
    }
    async fn source_and_favorite(
        ui: &Rc<Ui>,
        directory: &std::path::Path,
        name: &str,
        expected_backend: &str,
    ) -> Result<(), String> {
        let item = ui
            .state
            .borrow()
            .items
            .iter()
            .rev()
            .find(|item| item.origin.is_some())
            .cloned()
            .ok_or("No routed response source")?;
        let origin = item.origin.as_ref().unwrap();
        if origin.backend != expected_backend || origin.thread == "router" || origin.item.is_empty()
        {
            return Err("Routed response has the wrong source".into());
        }
        reveal(ui, &item.body).await?;
        super::smoke::capture(&ui.window, &directory.join(format!("{name}.png")));
        button(
            ui.transcript_view.upcast_ref(),
            &format!("Open response in {} ({})", origin.title, origin.backend),
        )
        .ok_or("No source button in actual GTK rows")?
        .emit_clicked();
        until(|| {
            ui.history_context
                .borrow()
                .as_ref()
                .is_some_and(|view| view.contains(&origin.item))
        })
        .await?;
        let context = ui.history_context.borrow().clone().unwrap();
        context.dialog.close();
        button(
            ui.transcript_view.upcast_ref(),
            &format!(
                "Save response from {} ({}) to favorites",
                origin.title, origin.backend
            ),
        )
        .ok_or("No favorite button in actual GTK rows")?
        .emit_clicked();
        let library = ui
            .favorites
            .borrow()
            .clone()
            .ok_or("Routed favorite dialog did not open")?;
        library.smoke_router_save(origin).await?;
        Ok(())
    }
    until(|| {
        ui.router_selected()
            && ui.state.borrow().options_loaded
            && !ui.state.borrow().loading
            && ui.send.is_sensitive()
    })
    .await?;
    async fn target_language(
        ui: &Rc<Ui>,
        directory: &std::path::Path,
        picker: Option<&Rc<crate::router_picker_ui::Picker>>,
        name: &str,
    ) -> Result<(), String> {
        let key = ui.composer_key().ok_or("No Router locale scope")?;
        let selected = ui.router.borrow().targets.get(&key).cloned();
        let draft = ui.composer_text();
        let options = ui.state.borrow().options.clone();
        let query = picker.map(|p| p.search.text());
        let status = picker.map(|p| p.smoke_status());
        let rows = picker.map(|p| {
            let mut rows = Vec::new();
            let mut child = p.list.first_child();
            while let Some(row) = child {
                child = row.next_sibling();
                rows.push(row);
            }
            rows
        });
        for chinese in [true, false] {
            crate::language::set(if chinese { "zh-CN" } else { "en-US" }, "en-US")?;
            crate::i18n_ui::apply();
            if let Some(changed) = ui.language.on_changed.borrow().as_ref() {
                changed();
            }
            crate::window_capture::next_frame(&ui.window).await?;
            if ui.composer.tooltip_text().as_deref()
                != Some(if chinese {
                    "输入消息。Ctrl+Enter 发送；Enter 换行。"
                } else {
                    "Write a message. Ctrl+Enter sends; Enter starts a new line."
                })
            {
                return Err("Composer instruction remained in the old language".into());
            }
            let expected = selected
                .as_ref()
                .map(|t| format!("{} · {}", t.title, t.backend))
                .unwrap_or_else(|| {
                    if chinese {
                        "自动路由"
                    } else {
                        "Automatic routing"
                    }
                    .into()
                });
            if ui.router_target_label.text().as_str() != expected {
                return Err("Router target language changed its literal identity".into());
            }
            if let Some(t) = &selected {
                let expected = format!(
                    "{}{}\n{}\n{}",
                    if chinese {
                        "直接发送到 "
                    } else {
                        "Send directly to "
                    },
                    t.title,
                    t.session_key,
                    t.cwd
                );
                if ui.router_target.tooltip_text().as_deref() != Some(&expected) {
                    return Err("Router tooltip changed literal target metadata".into());
                }
            }
            if let Some(p) = picker {
                if p.dialog.title()
                    != if chinese {
                        "Router 目标"
                    } else {
                        "Router target"
                    }
                    || p.automatic.label().as_deref()
                        != Some(if chinese {
                            "自动路由"
                        } else {
                            "Automatic routing"
                        })
                    || p.search.text() != *query.as_ref().unwrap()
                    || p.search.placeholder_text().as_deref()
                        != Some(if chinese {
                            "搜索会话、后端或项目路径"
                        } else {
                            "Search sessions, backends or project paths"
                        })
                {
                    return Err("Picker labels/query did not survive language change".into());
                }
                let mut current = Vec::new();
                let mut child = p.list.first_child();
                while let Some(row) = child {
                    child = row.next_sibling();
                    current.push(row);
                }
                if current != *rows.as_ref().unwrap() {
                    return Err("Language change rebuilt the target rows".into());
                }
                if chinese {
                    if !p.smoke_status().contains("个会话") {
                        return Err("Picker count remained English".into());
                    }
                    crate::window_capture::capture_dialog(
                        &ui.window,
                        &p.dialog,
                        &directory.join(format!("router-language-{name}.png")),
                    )
                    .await?;
                } else if p.smoke_status() != *status.as_ref().unwrap() {
                    return Err("Picker count did not survive language roundtrip".into());
                }
            }
            if ui.composer_text() != draft
                || ui.state.borrow().options != options
                || ui.composer_key().as_deref() != Some(&key)
                || ui.router.borrow().targets.get(&key) != selected.as_ref()
            {
                return Err(
                    "Router language change altered draft, options, scope or chosen target".into(),
                );
            }
        }
        Ok(())
    }
    async fn choose_explicit_target(
        ui: &Rc<Ui>,
        directory: &std::path::Path,
    ) -> Result<(), String> {
        let source = ui
            .state
            .borrow()
            .selected
            .clone()
            .ok_or("No picker source")?;
        let draft = ui.composer_text();
        let options = ui.state.borrow().options.clone();
        // A dialog result must not cross a session change, even while its
        // authenticated catalog read is still outstanding.
        until(|| ui.router_target.is_sensitive()).await?;
        ui.router_target.emit_clicked();
        let hidden = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|t| t.id == "hidden")
            .cloned()
            .ok_or("No alternate picker scope")?;
        ui.select(hidden);
        until(|| ui.router_picker.borrow().is_none() && !ui.state.borrow().loading).await?;
        ui.router_select_reference("codex", source);
        until(|| {
            !ui.state.borrow().loading
                && ui.state.borrow().options_loaded
                && ui.router_target.is_sensitive()
        })
        .await?;
        if ui.composer_text() != draft {
            return Err("Closing a stale picker changed the Router draft".into());
        }
        std::fs::write(directory.join("picker-pages"), b"pages").map_err(|e| e.to_string())?;
        ui.router_target.emit_clicked();
        let panel = ui
            .router_picker
            .borrow()
            .clone()
            .ok_or("No native target picker")?;
        until(|| panel.smoke_ready("", 62, 0)).await?;
        if panel.list.row_at_index(49).is_none() || panel.list.row_at_index(50).is_some() {
            return Err("Target picker constructed an unbounded page".into());
        }
        panel.search.set_text("Paged document");
        until(|| panel.smoke_ready("Paged document", 60, 0)).await?;
        panel.next.emit_clicked();
        until(|| panel.smoke_ready("Paged document", 60, 50)).await?;
        if panel.list.row_at_index(9).is_none()
            || panel.list.row_at_index(10).is_some()
            || panel.next.is_sensitive()
        {
            return Err("Target picker final page is incorrect".into());
        }
        target_language(ui, directory, Some(&panel), "paged").await?;
        panel.previous.emit_clicked();
        until(|| panel.smoke_ready("Paged document", 60, 0)).await?;
        panel.search.set_text("does-not-exist");
        until(|| panel.smoke_ready("does-not-exist", 0, 0)).await?;
        if panel.list.first_child().is_some() {
            return Err("Empty target search retained selectable stale rows".into());
        }
        // Let an earlier query complete after the entry already changed.
        panel.search.set_text("Native target");
        panel.search.set_text("OPENCODE:oc-target");
        until(|| panel.smoke_ready("OPENCODE:oc-target", 1, 0)).await?;
        let row = panel
            .list
            .row_at_index(0)
            .ok_or("Missing actual cross-backend row")?;
        crate::window_capture::capture_dialog(
            &ui.window,
            &panel.dialog,
            &directory.join("router-picker-opencode.png"),
        )
        .await?;
        until(|| {
            visible(
                row.upcast_ref(),
                panel.list.upcast_ref(),
                "opencode:oc-target",
            )
        })
        .await?;
        crate::window_capture::capture_dialog(
            &ui.window,
            &panel.dialog,
            &directory.join("router-picker-opencode.png"),
        )
        .await?;
        panel.list.emit_by_name::<()>("row-activated", &[&row]);
        until(|| ui.router_picker.borrow().is_none())
            .await
            .map_err(|error| format!("After cross-backend target selection: {error}"))?;
        let key = ui.composer_key().ok_or("Router lost picker scope")?;
        let closed_picker = panel.clone();
        if ui.router.borrow().targets.get(&key).is_none_or(|target| {
            target.session_key != "opencode:oc-target"
                || target.cwd != directory.join("project-oc").to_string_lossy()
        }) {
            return Err("Cross-backend picker changed target identity or directory".into());
        }
        if ui.composer_text() != draft || ui.state.borrow().options != options {
            return Err("Choosing a target replaced the draft or model options".into());
        }
        target_language(ui, directory, None, "explicit").await?;
        ui.router_target.emit_clicked();
        let panel = ui
            .router_picker
            .borrow()
            .clone()
            .ok_or("No clear-target picker")?;
        crate::window_capture::capture_dialog(
            &ui.window,
            &panel.dialog,
            &directory.join("router-picker-automatic.png"),
        )
        .await?;
        until(|| panel.automatic.is_mapped()).await?;
        panel.automatic.emit_clicked();
        until(|| ui.router_picker.borrow().is_none())
            .await
            .map_err(|error| format!("After automatic-routing selection: {error}"))?;
        if ui.router.borrow().targets.contains_key(&key) {
            return Err("Automatic routing kept an explicit target".into());
        }
        target_language(ui, directory, None, "automatic").await?;
        ui.router_target.emit_clicked();
        let panel = ui
            .router_picker
            .borrow()
            .clone()
            .ok_or("No explicit Codex picker")?;
        // A delayed signal from a closed instance in the *same* session must
        // not replace the new instance's pending catalog request.
        closed_picker
            .search
            .emit_by_name::<()>("search-changed", &[]);
        until(|| panel.smoke_ready("", 62, 0))
            .await
            .map_err(|error| format!("Reopening Codex picker: {error}"))?;
        panel.search.set_text("Native target");
        until(|| panel.smoke_ready("Native target", 1, 0)).await?;
        let row = panel.list.row_at_index(0).ok_or("No Codex target row")?;
        crate::window_capture::capture_dialog(
            &ui.window,
            &panel.dialog,
            &directory.join("router-picker-codex.png"),
        )
        .await?;
        until(|| row.is_mapped()).await?;
        panel.list.emit_by_name::<()>("row-activated", &[&row]);
        until(|| ui.router_picker.borrow().is_none())
            .await
            .map_err(|error| format!("After explicit Codex selection: {error}"))?;
        std::fs::remove_file(directory.join("picker-pages")).map_err(|e| e.to_string())?;
        if ui
            .router
            .borrow()
            .targets
            .get(&key)
            .is_none_or(|target| target.session_key != "codex:target")
        {
            return Err("Explicit Codex target was not selected".into());
        }
        Ok(())
    }
    async fn target_navigation(
        ui: &Rc<Ui>,
        directory: &std::path::Path,
        backend: &str,
    ) -> Result<(), String> {
        let source = ui
            .state
            .borrow()
            .selected
            .clone()
            .ok_or("No Router source")?;
        let source_draft = ui.composer_text();
        let source_options = ui.state.borrow().options.clone();
        let item = ui
            .state
            .borrow()
            .items
            .iter()
            .rev()
            .find(|item| item.origin.as_ref().is_some_and(|o| o.backend == backend))
            .cloned()
            .ok_or("No target reply to navigate")?;
        let origin = item.origin.as_ref().unwrap().clone();
        let tooltip = format!("Open session {} ({})", origin.title, origin.backend);
        reveal(ui, &item.body).await?;
        if backend == "codex" {
            std::fs::write(directory.join("delay-target-summary"), b"delay")
                .map_err(|e| e.to_string())?;
            button(ui.transcript_view.upcast_ref(), &tooltip)
                .ok_or("No live session button")?
                .emit_clicked();
            let other = ui
                .state
                .borrow()
                .threads
                .iter()
                .find(|t| t.id == "hidden")
                .cloned()
                .ok_or("No alternate navigation scope")?;
            ui.select(other);
            until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
            ui.composer
                .buffer()
                .set_text("Do not replace this session draft");
            glib::timeout_future(Duration::from_millis(1000)).await;
            if ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_none_or(|t| t.id != "hidden")
                || ui.composer_text() != "Do not replace this session draft"
                || ui.router_back.is_visible()
            {
                return Err("Late target lookup changed a newer selection or draft".into());
            }
            std::fs::remove_file(directory.join("delay-target-summary"))
                .map_err(|e| e.to_string())?;
            ui.router_select_reference("codex", source.clone());
            until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
            reveal(ui, &item.body).await?;
        }
        std::fs::write(directory.join("omit-target-catalog"), b"omit")
            .map_err(|e| e.to_string())?;
        button(ui.transcript_view.upcast_ref(), &tooltip)
            .ok_or("Live session button disappeared")?
            .emit_clicked();
        until(|| {
            ui.state.borrow().backend == backend
                && ui
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == origin.thread)
                && !ui.state.borrow().loading
                && ui.state.borrow().options_loaded
                && ui.router.borrow().navigation.is_none()
        })
        .await
        .map_err(|error| format!("{error}: opening {backend}, {}", ui.status.text()))?;
        if !ui.router_back.is_visible()
            || ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_none_or(|t| t.cwd != origin.root)
            || !ui
                .state
                .borrow()
                .items
                .iter()
                .any(|i| i.id == origin.item && i.turn_id == origin.turn)
        {
            super::smoke::capture(&ui.window, &directory.join("router-target-incomplete.png"));
            return Err(format!("Live target lost its history, source anchor or return control: backend={backend}, back={}, selected={:?}, origin={}/{}/{}, items={:?}",ui.router_back.is_visible(),ui.state.borrow().selected.as_ref().map(|t|(&t.id,&t.cwd)),origin.thread,origin.turn,origin.item,ui.state.borrow().items.iter().map(|i|(&i.id,&i.turn_id)).collect::<Vec<_>>()));
        }
        if !ui
            .state
            .borrow()
            .items
            .iter()
            .any(|i| i.role == "You" && i.turn_id == origin.turn && i.body == *origin.question)
        {
            return Err("Target live history omitted the original forwarded question".into());
        }
        ui.catalog(false);
        until(|| !ui.state.borrow().catalog_loading).await?;
        if !ui
            .state
            .borrow()
            .threads
            .iter()
            .any(|thread| thread.id == origin.thread)
        {
            return Err("Catalog refresh removed the explicitly opened target".into());
        }
        std::fs::remove_file(directory.join("omit-target-catalog")).map_err(|e| e.to_string())?;
        let expected_model = if backend == "codex" {
            "target-model"
        } else {
            "provider/oc-target-model"
        };
        if ui.state.borrow().options.model != expected_model {
            return Err("Opening a target carried over controller model options".into());
        }
        if ui.window.visible_dialog().is_some() {
            return Err("A reply in the live tail unexpectedly opened historical context".into());
        }
        super::smoke::capture(
            &ui.window,
            &directory.join(format!("router-live-{backend}.png")),
        );
        until(|| {
            visible(
                ui.transcript_view.upcast_ref(),
                ui.transcript_view.upcast_ref(),
                &origin.question,
            )
        })
        .await?;
        ui.composer
            .buffer()
            .set_text(&format!("Target draft {backend} 中文😀"));
        ui.router_back.emit_clicked();
        until(|| {
            ui.state.borrow().backend == "codex"
                && ui
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == source.id)
                && !ui.state.borrow().loading
                && ui.state.borrow().options_loaded
        })
        .await?;
        if ui.composer_text() != source_draft
            || ui.state.borrow().options != source_options
            || ui.router_back.is_visible()
        {
            return Err("Returning from a target replaced the Router draft or options".into());
        }
        let target_key = format!("{}:{}", origin.backend, origin.thread);
        if ui
            .state
            .borrow()
            .drafts
            .get(&target_key)
            .map(String::as_str)
            != Some(format!("Target draft {backend} 中文😀").as_str())
        {
            return Err("Returning discarded the target's draft".into());
        }
        if backend == "codex" {
            std::fs::write(directory.join("trim-target-live-tail"), b"trim")
                .map_err(|e| e.to_string())?;
            reveal(ui, &item.body).await?;
            button(ui.transcript_view.upcast_ref(), &tooltip)
                .ok_or("Missing historical target navigation action")?
                .emit_clicked();
            until(|| {
                ui.state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == origin.thread)
                    && !ui.state.borrow().loading
                    && ui
                        .history_context
                        .borrow()
                        .as_ref()
                        .is_some_and(|v| v.contains(&origin.item))
            })
            .await?;
            if ui.state.borrow().items.iter().any(|i| i.id == origin.item)
                || ui.composer_text() != format!("Target draft {backend} 中文😀")
            {
                return Err("Historical fallback changed the live tail or target draft".into());
            }
            let context = ui.history_context.borrow().clone().unwrap();
            crate::window_capture::capture_dialog(
                &ui.window,
                &context.dialog,
                &directory.join("router-target-historical.png"),
            )
            .await?;
            until(|| {
                visible(
                    context.dialog.upcast_ref(),
                    context.dialog.upcast_ref(),
                    &item.body,
                )
            })
            .await?;
            crate::window_capture::capture_dialog(
                &ui.window,
                &context.dialog,
                &directory.join("router-target-historical.png"),
            )
            .await?;
            context.dialog.close();
            until(|| ui.window.visible_dialog().is_none()).await?;
            ui.router_back.emit_clicked();
            until(|| {
                ui.state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == source.id)
                    && !ui.state.borrow().loading
                    && ui.state.borrow().options_loaded
            })
            .await?;
            if ui.composer_text() != source_draft {
                return Err("Historical target fallback lost the Router draft".into());
            }
            std::fs::remove_file(directory.join("trim-target-live-tail"))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    async fn mark_visible(ui: &Rc<Ui>, expected: u64) -> Result<(), String> {
        until(|| ui.window.visible_dialog().is_none()).await?;
        ui.router_bar.seen.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .histories
                .iter()
                .find(|h| h.controller == "codex:router")
                .is_some_and(|h| h.unread_count == expected)
        })
        .await
    }
    if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_REFS").is_some() {
        async fn choose_reference_target(ui: &Rc<Ui>, key: &str) -> Result<(), String> {
            ui.router_target.emit_clicked();
            let panel = ui
                .router_picker
                .borrow()
                .clone()
                .ok_or("No reference target picker")?;
            until(|| panel.smoke_ready("", 2, 0)).await?;
            panel.search.set_text(key);
            until(|| panel.smoke_ready(key, 1, 0)).await?;
            let row = panel
                .list
                .row_at_index(0)
                .ok_or("No matching reference target")?;
            panel.list.emit_by_name::<()>("row-activated", &[&row]);
            until(|| ui.router_picker.borrow().is_none() && ui.window.visible_dialog().is_none())
                .await
        }
        let key = "codex:router";
        super::refs::pick(ui, "@ref", "ref-note.md").await?;
        if ui.state.borrow().references.get(key).len() != 1 {
            return Err(
                "Router Codex file completion lost its structured project reference".into(),
            );
        }
        button(ui.composer_tools.widget.upcast_ref(), "Remove reference")
            .ok_or("No actual reference removal button")?
            .emit_clicked();
        if !ui.state.borrow().references.get(key).is_empty() {
            return Err("Reference chip removal failed".into());
        }
        let source = ui.state.borrow().selected.clone().unwrap();
        ui.router_attach_reference_files(vec![directory.join("project/参考 large.md")]);
        let other = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "hidden")
            .cloned()
            .ok_or("No alternate reference scope")?;
        ui.select(other);
        until(|| ui.router.borrow().files_request.is_none() && !ui.state.borrow().loading).await?;
        if !ui.state.borrow().references.get(key).is_empty()
            || !ui.state.borrow().references.get("codex:hidden").is_empty()
        {
            return Err("Late file preparation crossed a session boundary".into());
        }
        ui.router_select_reference("codex", source);
        until(|| ui.send.is_sensitive() && !ui.state.borrow().loading).await?;
        ui.router_attach_reference_files(vec![directory.join("project/参考 large.md")]);
        until(|| {
            ui.state.borrow().references.get(key).len() == 1
                && ui.router.borrow().files_request.is_none()
        })
        .await?;
        super::refs::pick(ui, "$router", "router-build").await?;
        super::refs::pick(ui, "$router", "router-plugin").await?;
        if ui.state.borrow().references.get(key).len() != 3 {
            return Err(
                "Router skill/plugin selection dropped the file or another reference".into(),
            );
        }
        std::fs::write(directory.join("hold-ack-controller"), b"hold")
            .map_err(|error| error.to_string())?;
        ui.composer.buffer().set_text("Route scoped references");
        ui.send.emit_clicked();
        until(|| ui.router.borrow().jobs.len() == 1 && ui.router_inputs_waiting()).await?;
        ui.composer
            .buffer()
            .set_text("Keep my newer reference draft 中文😀");
        ui.router_attach_reference_files(vec![directory.join("project/newer ref.md")]);
        until(|| {
            ui.state.borrow().references.get(key).len() == 4
                && ui.router.borrow().files_request.is_none()
        })
        .await?;
        super::smoke::capture(&ui.window, &directory.join("router-refs-pending.png"));
        std::fs::remove_file(directory.join("hold-ack-controller"))
            .map_err(|error| error.to_string())?;
        until(|| {
            !ui.router_inputs_waiting()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .any(|display| display.job.status == "completed")
        })
        .await?;
        if ui.composer_text() != "Keep my newer reference draft 中文😀"
            || ui.state.borrow().references.get(key).len() != 1
            || !ui.state.borrow().references.get(key)[0]
                .label
                .ends_with("newer ref.md")
        {
            return Err("Reference acknowledgement consumed newer text or files".into());
        }
        ui.composer.buffer().set_text("clarify references");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 2
                && !ui.router_inputs_waiting()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .any(|display| display.job.status == "clarify")
        })
        .await?;
        if ui.state.borrow().references.get(key).len() != 1
            || ui.composer_text() != "clarify references"
        {
            return Err("Clarification discarded reference inputs".into());
        }
        ui.composer.buffer().set_text("Choose oc-target file");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 3
                && !ui.router_inputs_waiting()
                && ui.state.borrow().references.get(key).is_empty()
        })
        .await?;
        super::refs::pick(ui, "$router", "router-build").await?;
        std::fs::write(directory.join("disable-router-skill"), b"disabled")
            .map_err(|error| error.to_string())?;
        ui.composer.buffer().set_text("Use disabled target skill");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 4
                && !ui.router_inputs_waiting()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .any(|display| display.job.status == "failed")
        })
        .await?;
        if ui.state.borrow().references.get(key).len() != 1
            || ui.composer_text() != "Use disabled target skill"
        {
            return Err("Unavailable target skill discarded the draft".into());
        }
        std::fs::remove_file(directory.join("disable-router-skill"))
            .map_err(|error| error.to_string())?;
        ui.composer.buffer().set_text("");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 5
                && !ui.router_inputs_waiting()
                && ui.state.borrow().references.get(key).is_empty()
        })
        .await?;
        super::refs::pick(ui, "$router", "router-build").await?;
        ui.composer.buffer().set_text("Choose oc-target skill");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 6
                && !ui.router_inputs_waiting()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .filter(|display| display.job.status == "failed")
                    .count()
                    == 2
        })
        .await?;
        if ui.state.borrow().references.get(key).len() != 1
            || ui.composer_text() != "Choose oc-target skill"
        {
            return Err(
                "Incompatible Codex skill lost its reference draft on an OpenCode target".into(),
            );
        }
        reveal(ui, "Choose an OpenCode skill").await?;
        super::smoke::capture(&ui.window, &directory.join("router-refs-retained.png"));
        let ids = ui
            .state
            .borrow()
            .references
            .get(key)
            .iter()
            .map(|reference| reference.id.clone())
            .collect::<Vec<_>>();
        ui.state.borrow_mut().references.remove(key, &ids);
        choose_reference_target(ui, "opencode:oc-target").await?;
        super::refs::pick(ui, "@oc", "oc-reference.md").await?;
        super::refs::pick(ui, "$oc", "oc-build").await?;
        let cross = ui.state.borrow().references.get(key);
        if cross.len() != 2
            || cross[0].input["root"].as_str()
                != Some(directory.join("project-oc").to_string_lossy().as_ref())
        {
            return Err(
                "Codex Router completion did not read the selected OpenCode backend/project".into(),
            );
        }
        super::smoke::capture(&ui.window, &directory.join("router-refs-cross-backend.png"));
        ui.composer.buffer().set_text("@oc");
        until(|| ui.composer_tools.smoke_has("oc-reference.md")).await?;
        let old_query = ui
            .composer_tools
            .query
            .borrow()
            .clone()
            .ok_or("No old target query")?;
        let old_choice = ui
            .composer_tools
            .sources
            .borrow()
            .iter()
            .flat_map(|source| source.iter())
            .find(|choice| choice.name == "oc-reference.md")
            .cloned()
            .ok_or("No old target result")?;
        choose_reference_target(ui, "codex:target").await?;
        if ui.insert_reference(old_query, old_choice).is_ok()
            || ui.state.borrow().references.get(key).len() != 2
        {
            return Err("A stale target completion changed the retained references".into());
        }
        choose_reference_target(ui, "opencode:oc-target").await?;
        std::fs::write(directory.join("hold-oc-command"), b"hold").map_err(|e| e.to_string())?;
        ui.composer
            .buffer()
            .set_text("Run native OpenCode skill 中文😀 ");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 7
                && ui.router.borrow().jobs.values().any(|display| {
                    display.job.status == "dispatching"
                        && display.job.dispatch_notice.contains("will not guess")
                })
        })
        .await?;
        reveal(ui, "message identity yet").await?;
        capture_command(
            ui,
            "message identity yet",
            &directory.join("router-command-pending.png"),
        )
        .await?;
        ui.composer.buffer().set_text("Newer unsent command draft");
        if !ui.stop.is_sensitive() {
            return Err("Pending Router command hid its Stop action".into());
        }
        ui.stop.emit_clicked();
        until(|| ui.router.borrow().stop.is_none() && !ui.router.borrow().stopping).await?;
        if !ui
            .router
            .borrow()
            .jobs
            .values()
            .any(|display| display.job.status == "dispatching")
        {
            return Err(
                "Pending command Stop invented an outcome before server acknowledgement".into(),
            );
        }
        std::fs::write(directory.join("release-oc-command"), b"release")
            .map_err(|e| e.to_string())?;
        until(|| {
            !ui.router_inputs_waiting()
                && ui.state.borrow().references.get(key).is_empty()
                && ui.router.borrow().jobs.values().any(|display| {
                    display.job.target_turn_id.contains("command-user")
                        && display.job.status == "completed"
                })
        })
        .await?;
        if ui.composer_text() != "Newer unsent command draft" {
            return Err("Command acknowledgement consumed newer unsent text".into());
        }
        reveal(ui, "Native OpenCode skill response").await?;
        ui.toast.dismiss_all();
        capture_command(
            ui,
            "Native OpenCode skill response",
            &directory.join("router-command-completed.png"),
        )
        .await?;
        for name in ["hold-oc-command", "release-oc-command"] {
            std::fs::remove_file(directory.join(name)).map_err(|e| e.to_string())?;
        }
        ui.state.borrow_mut().references.remove(
            key,
            &cross
                .iter()
                .map(|reference| reference.id.clone())
                .collect::<Vec<_>>(),
        );
        ui.composer.buffer().set_text("");
        let settings = directory.join("config/codex-thread-studio/settings.json");
        let mut preferences: Value =
            serde_json::from_slice(&std::fs::read(&settings).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        preferences["router"]["controllerBackend"] = "opencode".into();
        std::fs::write(&settings, serde_json::to_vec(&preferences).unwrap())
            .map_err(|error| error.to_string())?;
        {
            let mut book = ui.router.borrow_mut();
            book.sync = None;
            book.next_poll = None;
        }
        ui.router_tick();
        until(|| {
            ui.router
                .borrow()
                .registry
                .as_ref()
                .is_some_and(|registry| registry.controller_backend == "opencode")
        })
        .await?;
        ui.router_select_reference("opencode",Thread::from_value(&json!({"id":"oc-router","name":"OpenCode Router","cwd":directory.join("data/codex-thread-studio/router").to_string_lossy()})).ok_or("No OpenCode controller metadata")?);
        until(|| {
            ui.router_target.is_sensitive()
                && ui.state.borrow().options_loaded
                && !ui.state.borrow().loading
        })
        .await?;
        ui.router_target.emit_clicked();
        let panel = ui
            .router_picker
            .borrow()
            .clone()
            .ok_or("No OpenCode target picker")?;
        until(|| panel.smoke_ready("", 2, 0)).await?;
        panel.search.set_text("opencode:oc-target");
        until(|| panel.smoke_ready("opencode:oc-target", 1, 0)).await?;
        crate::window_capture::capture_dialog(
            &ui.window,
            &panel.dialog,
            &directory.join("router-refs-opencode-target.png"),
        )
        .await?;
        let row = panel
            .list
            .row_at_index(0)
            .ok_or("Missing OpenCode target row")?;
        panel.list.emit_by_name::<()>("row-activated", &[&row]);
        until(|| ui.router_picker.borrow().is_none() && ui.window.visible_dialog().is_none())
            .await?;
        super::refs::pick(ui, "@oc", "oc-reference.md").await?;
        super::refs::pick(ui, "$oc", "oc-build").await?;
        let refs = ui.state.borrow().references.get("opencode:oc-router");
        if refs.len() != 2
            || refs[0].input["root"].as_str()
                != Some(directory.join("project-oc").to_string_lossy().as_ref())
        {
            return Err(
                "OpenCode completion did not use the explicit target's project directory".into(),
            );
        }
        super::smoke::capture(
            &ui.window,
            &directory.join("router-refs-opencode-scope.png"),
        );
        ui.state.borrow_mut().references.remove(
            "opencode:oc-router",
            &refs
                .iter()
                .map(|reference| reference.id.clone())
                .collect::<Vec<_>>(),
        );
        choose_reference_target(ui, "codex:target").await?;
        super::refs::pick(ui, "@ref", "ref-note.md").await?;
        super::refs::pick(ui, "$router", "router-plugin").await?;
        let reverse = ui.state.borrow().references.get("opencode:oc-router");
        if reverse.len() != 2
            || reverse[0].input["root"].as_str()
                != Some(directory.join("project").to_string_lossy().as_ref())
            || reverse[1].input["type"] != "mention"
        {
            return Err(
                "OpenCode Router completion did not read the selected Codex backend/project".into(),
            );
        }
        super::smoke::capture(
            &ui.window,
            &directory.join("router-refs-reverse-backend.png"),
        );
        return Ok(
            json!({"ok":true,"routerReferences":true,"metadataOnlyLargeFile":true,"newerDraftRetention":true,"lateScopeGuard":true,"codexFileCompletion":true,"targetInventoryGuard":true,"crossBackendFile":true,"openCodeSkillRetention":true,"crossBackendCompletion":true,"staleTargetCompletion":true,"openCodeCommand":true,"pendingCommandStop":true}),
        );
    }
    if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_IMAGES").is_some() {
        let key = "codex:router";
        ui.attach_files(key, vec![directory.join("router-image.png")])?;
        until(|| {
            ui.state.borrow().images.images(key).len() == 1
                && !ui.state.borrow().images.pending.contains_key(key)
        })
        .await?;
        let settings = directory.join("config/codex-thread-studio/settings.json");
        let original = std::fs::read(&settings).map_err(|error| error.to_string())?;
        let revision = ui
            .router
            .borrow()
            .registry
            .as_ref()
            .unwrap()
            .revision
            .clone();
        let mut concurrent: Value =
            serde_json::from_slice(&original).map_err(|error| error.to_string())?;
        concurrent["router"]["fallbacks"][0]["condition"] =
            "Image fixture concurrent revision".into();
        std::fs::write(&settings, serde_json::to_vec(&concurrent).unwrap())
            .map_err(|error| error.to_string())?;
        ui.composer.buffer().set_text("Keep rejected image text");
        ui.send.emit_clicked();
        until(|| {
            !ui.router.borrow().submitting
                && ui.router.borrow().pending.is_none()
                && !ui.router_inputs_waiting()
        })
        .await?;
        if ui.composer_text() != "Keep rejected image text"
            || ui.state.borrow().images.images(key).len() != 1
            || !ui.router.borrow().jobs.is_empty()
        {
            return Err("Rejected image routing discarded its draft or started a model".into());
        }
        std::fs::write(&settings, original).map_err(|error| error.to_string())?;
        {
            let mut book = ui.router.borrow_mut();
            book.sync = None;
            book.next_poll = None;
        }
        ui.router_tick();
        until(|| {
            ui.router
                .borrow()
                .registry
                .as_ref()
                .is_some_and(|registry| registry.revision == revision)
                && ui.send.is_sensitive()
        })
        .await?;
        std::fs::write(directory.join("hold-ack-controller"), b"hold")
            .map_err(|error| error.to_string())?;
        ui.composer.buffer().set_text("");
        ui.send.emit_clicked();
        until(|| ui.router.borrow().jobs.len() == 1 && ui.router_inputs_waiting()).await?;
        ui.composer
            .buffer()
            .set_text("Keep my newer image draft 中文😀");
        ui.attach_files(key, vec![directory.join("router-image-new.png")])?;
        until(|| {
            ui.state.borrow().images.images(key).len() == 2
                && !ui.state.borrow().images.pending.contains_key(key)
        })
        .await?;
        if ui.send.is_sensitive() {
            return Err("Pending image delivery allowed an accidental second send".into());
        }
        super::smoke::capture(&ui.window, &directory.join("router-image-pending.png"));
        std::fs::remove_file(directory.join("hold-ack-controller"))
            .map_err(|error| error.to_string())?;
        until(|| {
            !ui.router_inputs_waiting()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .any(|display| display.job.status == "completed")
        })
        .await?;
        if ui.composer_text() != "Keep my newer image draft 中文😀"
            || ui.state.borrow().images.images(key).len() != 1
            || ui.state.borrow().images.images(key)[0].name != "router-image-new.png"
        {
            return Err("Target acknowledgement removed a newer image or draft".into());
        }
        ui.composer.buffer().set_text("clarify image routing");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 2
                && !ui.router_inputs_waiting()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .any(|display| display.job.status == "clarify")
        })
        .await?;
        if ui.composer_text() != "clarify image routing"
            || ui.state.borrow().images.images(key).len() != 1
        {
            return Err("Clarification consumed an image before target dispatch".into());
        }
        ui.composer.buffer().set_text("Choose oc-target image");
        ui.send.emit_clicked();
        until(|| {
            ui.router.borrow().jobs.len() == 3
                && !ui.router_inputs_waiting()
                && ui.state.borrow().images.images(key).is_empty()
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .filter(|display| display.job.status == "completed")
                    .count()
                    == 2
        })
        .await?;
        if !ui.composer_text().is_empty() {
            return Err("Accepted routed image retained the submitted text".into());
        }
        reveal(ui, "[1 attached image]").await?;
        super::smoke::capture(&ui.window, &directory.join("router-image-delivered.png"));
        return Ok(
            json!({"ok":true,"routerImages":true,"imageOnly":true,"largeBody":true,"crossBackend":true,"rejectedDraftRetention":true,"clarificationRetention":true,"newerDraftAndImage":true}),
        );
    }
    if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_QUEUE").is_some() {
        let key = "codex:router";
        let reference_mode = std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_QUEUE_REFS").is_some();
        let reference_path = directory.join("project/queued 参考.md");
        until(|| {
            ui.send.is_sensitive() && ui.state.borrow().options_loaded && !ui.state.borrow().loading
        })
        .await?;
        until(|| {
            ui.state
                .borrow()
                .queues
                .entries
                .get(key)
                .is_some_and(|entry| entry.loaded)
        })
        .await?;
        if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_RESTORE").is_some() {
            until(|| ui.router.borrow().jobs.len() == 3).await?;
            let count = std::fs::read_to_string(directory.join("requests.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter(|line| line.contains("\"turn/start\""))
                .count();
            glib::timeout_future(Duration::from_millis(1300)).await;
            if std::fs::read_to_string(directory.join("requests.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter(|line| line.contains("\"turn/start\""))
                .count()
                != count
            {
                return Err("Restored Router queue replayed without Resume".into());
            }
            if !ui
                .state
                .borrow()
                .queues
                .entries
                .get(key)
                .is_some_and(|entry| entry.paused && entry.messages.len() == 1)
            {
                return Err("Router queue was not restored paused".into());
            }
            super::smoke::capture(&ui.window, &directory.join("router-queue-restored.png"));
            if reference_mode {
                let refs = ui.state.borrow().queues.entries[key].messages[0]
                    .input
                    .clone();
                if refs.len() != 3
                    || !ui.state.borrow().queues.entries[key].messages[0]
                        .text
                        .is_empty()
                {
                    return Err("Reference-only Router queue did not survive restart".into());
                }
                let moved = reference_path.with_extension("temporarily-missing");
                std::fs::rename(&reference_path, &moved).map_err(|error| error.to_string())?;
                ui.queue.smoke_toggle();
                until(|| {
                    ui.state
                        .borrow()
                        .queues
                        .entries
                        .get(key)
                        .is_some_and(|entry| {
                            entry.paused
                                && entry.flight.is_none()
                                && entry
                                    .error
                                    .as_deref()
                                    .is_some_and(|error| error.contains("no longer exists"))
                        })
                })
                .await?;
                if ui.router.borrow().jobs.len() != 3
                    || ui.state.borrow().queues.entries[key].messages[0].input != refs
                {
                    return Err(
                        "A missing reference consumed or modified the saved queue head".into(),
                    );
                }
                std::fs::rename(&moved, &reference_path).map_err(|error| error.to_string())?;
                super::smoke::capture(
                    &ui.window,
                    &directory.join("router-queue-reference-retained.png"),
                );
            }
            ui.queue.smoke_toggle();
            until(|| {
                ui.router.borrow().jobs.len() == 4
                    && ui
                        .state
                        .borrow()
                        .queues
                        .entries
                        .get(key)
                        .is_some_and(|entry| entry.messages.is_empty() && entry.flight.is_none())
            })
            .await?;
            return Ok(
                json!({"ok":true,"routerQueue":true,"restoredPaused":true,"explicitResume":true,"queuedReferences":reference_mode,"missingFileRetained":reference_mode}),
            );
        }
        std::fs::write(directory.join("hold-target"), b"hold").map_err(|e| e.to_string())?;
        ui.composer.buffer().set_text("Native Router queue seed");
        ui.send.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .jobs
                .values()
                .any(|display| display.job.status == "running")
                && ui.queue_button.is_sensitive()
        })
        .await?;
        if reference_mode {
            ui.router_attach_reference_files(vec![reference_path.clone()]);
            if ui.enqueue().is_ok() {
                return Err(
                    "Router queued a message before its reference metadata was ready".into(),
                );
            }
            until(|| {
                ui.state.borrow().references.get(key).len() == 1 && !ui.router_files_preparing()
            })
            .await?;
        }
        ui.composer
            .buffer()
            .set_text("Choose oc-target queued follow-up");
        ui.queue_button.emit_clicked();
        if reference_mode {
            ui.router_attach_reference_files(vec![directory.join("project/newer queue ref.md")]);
        }
        until(|| {
            ui.state
                .borrow()
                .queues
                .entries
                .get(key)
                .is_some_and(|entry| entry.messages.len() == 1 && entry.saving.is_none())
        })
        .await?;
        ui.composer
            .buffer()
            .set_text("Keep my newer Router queue draft 中文😀");
        glib::timeout_future(Duration::from_millis(1200)).await;
        if ui.router.borrow().jobs.len() != 1 {
            return Err("Router queue advanced when only its controller completed".into());
        }
        super::smoke::capture(&ui.window, &directory.join("router-queue-waiting.png"));
        if reference_mode {
            ui.router.borrow_mut().drop_queued_ack_once = true;
        }
        std::fs::remove_file(directory.join("hold-target")).map_err(|e| e.to_string())?;
        std::fs::write(directory.join("release-target"), b"release").map_err(|e| e.to_string())?;
        if reference_mode {
            until(|| {
                ui.router.borrow().pending_retry()
                    && ui.send.label().as_deref() == Some("Retry delivery")
            })
            .await?;
            let id = ui.router.borrow().pending.as_ref().unwrap().0["requestId"]
                .as_str()
                .unwrap()
                .to_owned();
            until(|| {
                ui.router
                    .borrow()
                    .jobs
                    .get(&id)
                    .is_some_and(|display| display.job.terminal())
            })
            .await?;
            if !ui.router.borrow().pending_retry()
                || ui.state.borrow().queues.entries[key].messages.len() != 1
            {
                return Err("Status polling consumed a queued reference snapshot without a matching POST acknowledgement".into());
            }
            super::smoke::capture(
                &ui.window,
                &directory.join("router-queue-uncertain-ack.png"),
            );
            let newer_draft = ui.composer_text();
            ui.composer.buffer().set_text("/help");
            ui.send.emit_clicked();
            until(|| {
                ui.router.borrow().pending.is_none()
                    && ui.state.borrow().queues.entries[key].messages.is_empty()
                    && ui.state.borrow().queues.entries[key].flight.is_none()
            })
            .await?;
            if ui.composer_text() != "/help" || ui.window.visible_dialog().is_some() {
                return Err("Retry interpreted or replaced the newer command draft instead of checking its frozen delivery".into());
            }
            ui.composer.buffer().set_text(&newer_draft);
        }
        until(|| {
            ui.router.borrow().jobs.len() == 2
                && ui
                    .router
                    .borrow()
                    .jobs
                    .values()
                    .all(|display| display.job.status == "completed")
                && ui
                    .state
                    .borrow()
                    .queues
                    .entries
                    .get(key)
                    .is_some_and(|entry| entry.messages.is_empty() && entry.flight.is_none())
        })
        .await?;
        if ui.composer_text() != "Keep my newer Router queue draft 中文😀" {
            return Err("Queued Router acceptance replaced a newer draft".into());
        }
        if reference_mode {
            until(|| !ui.router_files_preparing()).await?;
            let newer = ui.state.borrow().references.get(key);
            if newer.len() != 1 || !newer[0].label.ends_with("newer queue ref.md") {
                return Err("Queued Router acknowledgement cleared a newer reference draft".into());
            }
            button(ui.composer_tools.widget.upcast_ref(), "Remove reference")
                .ok_or("No newer reference removal button")?
                .emit_clicked();
        }
        if !ui.router.borrow().jobs.values().any(|display| {
            display
                .job
                .target
                .as_ref()
                .is_some_and(|target| target.backend == "opencode")
        }) {
            return Err("Queued Router follow-up lost cross-backend routing".into());
        }
        std::fs::remove_file(directory.join("release-target")).map_err(|e| e.to_string())?;
        std::fs::write(directory.join("hold-target"), b"hold").map_err(|e| e.to_string())?;
        ui.composer
            .buffer()
            .set_text("Native Router queue stop seed");
        ui.send.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .jobs
                .values()
                .any(|display| display.job.status == "running")
                && ui.queue_button.is_sensitive()
        })
        .await?;
        if reference_mode {
            ui.router_attach_reference_files(vec![reference_path.clone()]);
            until(|| {
                ui.state.borrow().references.get(key).len() == 1 && !ui.router_files_preparing()
            })
            .await?;
            super::refs::pick(ui, "$router", "router-build").await?;
            super::refs::pick(ui, "$router", "router-plugin").await?;
        }
        ui.composer.buffer().set_text(if reference_mode {
            ""
        } else {
            "Restored Router queued later"
        });
        ui.queue_button.emit_clicked();
        until(|| {
            ui.state
                .borrow()
                .queues
                .entries
                .get(key)
                .is_some_and(|entry| entry.messages.len() == 1 && entry.saving.is_none())
        })
        .await?;
        ui.stop.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .jobs
                .values()
                .any(|display| display.job.status == "stopped")
                && ui
                    .state
                    .borrow()
                    .queues
                    .entries
                    .get(key)
                    .is_some_and(|entry| entry.paused && entry.messages.len() == 1)
        })
        .await?;
        std::fs::remove_file(directory.join("hold-target")).map_err(|e| e.to_string())?;
        glib::timeout_future(Duration::from_millis(600)).await;
        if ui.router.borrow().jobs.len() != 3 {
            return Err("Stop advanced the Router queue".into());
        }
        super::smoke::capture(&ui.window, &directory.join("router-queue-paused.png"));
        return Ok(
            json!({"ok":true,"routerQueue":true,"fullDeliveryGate":true,"crossBackend":true,"newerDraft":true,"stopPauses":true,"lostAckSnapshot":reference_mode}),
        );
    }
    if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_HISTORY").is_some() {
        let seeded: Value = serde_json::from_slice(
            &std::fs::read(directory.join("history-seed.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let oldest = seeded[0]
            .as_str()
            .ok_or("Missing historical seed")?
            .to_owned();
        let middle = seeded[25]
            .as_str()
            .ok_or("Missing historical seed")?
            .to_owned();
        until(|| {
            ui.router
                .borrow()
                .view
                .as_ref()
                .is_some_and(|(_, h)| h.ids.len() == 20)
                && ui.router_bar.older.is_sensitive()
        })
        .await?;
        if ui
            .router
            .borrow()
            .histories
            .iter()
            .find(|h| h.controller == "codex:router")
            .unwrap()
            .unread_count
            != 2
        {
            return Err("Older unread records were lost outside the latest page".into());
        }
        ui.router_bar.older.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .view
                .as_ref()
                .is_some_and(|(spec, h)| spec.before.is_some() && h.ids.contains(&middle))
                && ui.router.borrow().page.is_none()
        })
        .await?;
        glib::timeout_future(Duration::from_millis(700)).await;
        ui.router_bar.seen.emit_clicked();
        glib::timeout_future(Duration::from_millis(300)).await;
        if !ui.router.borrow().jobs.get(&middle).unwrap().job.unread {
            return Err("Offscreen reply was marked read by model presence".into());
        }
        async fn reveal_job(ui: &Rc<Ui>, id: &str) -> Result<(), String> {
            ui.render_items();
            glib::timeout_future(Duration::from_millis(60)).await;
            let item_id = format!("router:{id}:status");
            let index = (0..ui.transcript.n_items())
                .rev()
                .find(|index| {
                    ui.transcript
                        .item(*index)
                        .and_downcast::<glib::BoxedAnyObject>()
                        .is_some_and(|object| {
                            object.borrow::<crate::chat_ui::Row>().item.id == item_id
                        })
                })
                .ok_or("Historical row missing")?;
            ui.transcript_view
                .scroll_to(index, gtk::ListScrollFlags::NONE, None);
            // Covered Wayland windows may suspend frame callbacks. The existing
            // own-window snapshot helper settles GTK allocation before testing
            // the actual bound row geometry; it does not change read eligibility.
            let directory =
                std::path::PathBuf::from(std::env::var_os("STUDIO_GNOME_SMOKE_DIR").unwrap());
            super::smoke::capture(&ui.window, &directory.join("router-history-position.png"));
            until(|| {
                ui.chat_visibility.visible_ends(ui.transcript_view.upcast_ref()).contains(&(item_id.clone(), id.into()))
            }).await.map_err(|error|{
                super::smoke::capture(&ui.window,&std::path::PathBuf::from(std::env::var_os("STUDIO_GNOME_SMOKE_DIR").unwrap()).join("history-position-failed.png"));
                let adjustment=ui.transcript_scroll.vadjustment();
                format!("{error}: historical row {id}, index {index}, scroll={}/{}/{}, viewport={}, visible={:?}",adjustment.value(),adjustment.upper(),adjustment.page_size(),ui.transcript_view.height(),ui.chat_visibility.visible_ends(ui.transcript_view.upcast_ref()))
            })
        }
        reveal_job(ui, &middle).await?;
        mark_visible(ui, 1).await?;
        if !ui
            .state
            .borrow()
            .activity
            .get("codex:router")
            .is_some_and(|a| a.attention)
        {
            return Err("Reading one page cleared another page's attention".into());
        }
        if !ui
            .router
            .borrow()
            .view
            .as_ref()
            .is_some_and(|(spec, h)| spec.before.is_some() && h.ids.contains(&middle))
        {
            return Err("Live polling replaced the historical page".into());
        }
        super::smoke::capture(&ui.window, &directory.join("router-history-middle.png"));
        ui.router_bar.older.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .view
                .as_ref()
                .is_some_and(|(_, h)| h.ids.contains(&oldest) && h.next.is_none())
                && ui.router.borrow().page.is_none()
        })
        .await?;
        reveal_job(ui, &oldest).await?;
        mark_visible(ui, 0).await?;
        if ui
            .state
            .borrow()
            .activity
            .get("codex:router")
            .is_some_and(|a| a.attention)
        {
            return Err("Read Router records returned to Attention".into());
        }
        ui.router_bar.newer.emit_clicked();
        until(|| {
            ui.router
                .borrow()
                .view
                .as_ref()
                .is_some_and(|(spec, h)| spec.after.is_some() && h.ids.contains(&middle))
                && ui.router.borrow().page.is_none()
        })
        .await?;
        if ui.router.borrow().jobs.get(&middle).unwrap().job.unread {
            return Err("Paging backward resurrected unread state".into());
        }
        ui.router_bar.latest.emit_clicked();
        until(|| {
            ui.router.borrow().view.as_ref().is_some_and(|(spec, h)| {
                spec.before.is_none() && spec.after.is_none() && h.ids.len() == 20
            }) && ui.router.borrow().page.is_none()
        })
        .await?;
        if ui.router.borrow().jobs.len() != 20 || ui.router.borrow().jobs.contains_key(&oldest) {
            return Err("Historical rich trees remained retained after paging".into());
        }
        let search = search_acceptance::run(ui, directory, &oldest).await?;
        super::smoke::capture(&ui.window, &directory.join("router-history-latest.png"));
        return Ok(
            json!({"ok":true,"older":true,"newer":true,"latest":true,"offscreenReadGuard":true,"sharedUnread":true,"pageRetention":true,"boundedView":true,"search":search}),
        );
    }
    if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_RESTORE").is_some() {
        until(|| ui.router.borrow().jobs.len() == 3).await?;
        if ui
            .router
            .borrow()
            .histories
            .iter()
            .any(|h| h.unread_count != 0)
        {
            return Err("Read Router replies reappeared as unread after reopening".into());
        }
        if ui.router.borrow().jobs.values().any(|d| !d.job.terminal()) {
            return Err("Router restart invented an active job".into());
        }
        if ui
            .state
            .borrow()
            .items
            .iter()
            .filter(|item| item.origin.is_some())
            .count()
            != 2
        {
            return Err("Router restart lost original reply anchors".into());
        }
        super::smoke::capture(&ui.window, &directory.join("router-restored.png"));
        return Ok(json!({"ok":true,"restored":true,"jobs":3}));
    }
    until(|| {
        ui.title.title() == "Router"
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| !t.cwd.is_empty())
    })
    .await?;
    // Deliberately invalidate the snapshot without sending anything to a model.
    let settings = directory.join("config/codex-thread-studio/settings.json");
    let original = std::fs::read(&settings).map_err(|e| e.to_string())?;
    let original_revision = ui
        .router
        .borrow()
        .registry
        .as_ref()
        .unwrap()
        .revision
        .clone();
    let mut external: Value = serde_json::from_slice(&original).map_err(|e| e.to_string())?;
    external["router"]["fallbacks"][0]["condition"] = "Concurrent external change".into();
    std::fs::write(&settings, serde_json::to_vec(&external).unwrap()).map_err(|e| e.to_string())?;
    ui.composer
        .buffer()
        .set_text("Native rejected Router request");
    ui.send.emit_clicked();
    until(|| !ui.router.borrow().submitting && ui.router.borrow().pending.is_none()).await?;
    let rejected_retained = ui.composer_text() == "Native rejected Router request"
        && ui.router.borrow().jobs.is_empty();
    std::fs::write(&settings, original).map_err(|e| e.to_string())?;
    {
        let mut book = ui.router.borrow_mut();
        book.sync = None;
        book.next_poll = None;
    }
    ui.router_tick();
    until(|| {
        let book = ui.router.borrow();
        book.sync.is_none()
            && book
                .registry
                .as_ref()
                .is_some_and(|r| r.revision == original_revision)
    })
    .await?;
    if !rejected_retained {
        return Err("Definitively rejected Router request trapped or discarded the draft".into());
    }
    ui.composer
        .buffer()
        .set_text("Native Router UI codex request");
    ui.send.emit_clicked();
    ui.conversation.set_visible(false);
    ui.composer.buffer().set_text("Keep my newer draft 中文😀");
    until(|| {
        ui.router
            .borrow()
            .jobs
            .values()
            .any(|d| d.job.status == "completed")
    })
    .await?;
    if ui.composer_text() != "Keep my newer draft 中文😀" {
        return Err("Router acceptance discarded a newer draft".into());
    }
    ui.window
        .lookup_action("router-mark-visible-read")
        .ok_or("Missing read action")?
        .activate(None);
    glib::timeout_future(Duration::from_millis(700)).await;
    if ui
        .router
        .borrow()
        .jobs
        .values()
        .filter(|d| d.job.status == "completed")
        .any(|d| !d.job.unread)
    {
        return Err("Hidden conversation was marked read".into());
    }
    ui.conversation.set_visible(true);
    source_and_favorite(ui, directory, "router-codex-reply", "codex").await?;
    mark_visible(ui, 0).await?;
    target_navigation(ui, directory, "codex").await?;
    ui.composer
        .buffer()
        .set_text("Choose oc-target from native Router UI");
    ui.send.emit_clicked();
    until(|| {
        ui.router
            .borrow()
            .jobs
            .values()
            .filter(|d| d.job.status == "completed")
            .count()
            == 2
    })
    .await?;
    source_and_favorite(ui, directory, "router-opencode-reply", "opencode").await?;
    mark_visible(ui, 0).await?;
    target_navigation(ui, directory, "opencode").await?;
    choose_explicit_target(ui, directory).await?;
    std::fs::write(directory.join("hold-target"), b"hold").map_err(|e| e.to_string())?;
    ui.composer
        .buffer()
        .set_text("Native Router UI stop request");
    ui.send.emit_clicked();
    until(|| {
        ui.router
            .borrow()
            .jobs
            .values()
            .any(|d| d.job.status == "running")
            && ui.stop.is_sensitive()
    })
    .await?;
    ui.stop.emit_clicked();
    until(|| {
        ui.router
            .borrow()
            .jobs
            .values()
            .any(|d| d.job.status == "stopped")
            && !ui.stop.is_sensitive()
    })
    .await?;
    if ui
        .state
        .borrow()
        .items
        .iter()
        .any(|item| item.body.contains("targetSessionKey"))
    {
        return Err("Raw Router decisions leaked into the native transcript".into());
    }
    std::fs::remove_file(directory.join("hold-target")).map_err(|e| e.to_string())?;
    super::smoke::capture(&ui.window, &directory.join("router-stopped.png"));
    Ok(
        json!({"ok":true,"routerTargetLanguage":true,"composerInstructionLanguage":true,"localePreservesTargetAndDraft":true,"send":true,"stop":true,"crossBackend":true,"newerDraft":true,"actualSourceButtons":true,"actualFavoriteButtons":true,"sourceAnchors":true,"visibleReplies":true,"rejectedDraftRetention":true,"restoredControllerMetadata":true,"hiddenReadGuard":true,"visibleReadReceipts":true,"targetNavigation":true,"navigationDrafts":true,"lateNavigationGuard":true,"historicalTargetNavigation":true,"uncataloguedTargetNavigation":true}),
    )
}
