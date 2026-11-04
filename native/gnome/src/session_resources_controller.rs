use super::*;
use crate::session_resources_ui::Input;

impl Ui {
    pub(super) fn sync_resources(&self) {
        let input = {
            let state = self.state.borrow();
            state
                .selected
                .as_ref()
                .filter(|_| !state.loading)
                .map(|thread| Input {
                    context: crate::session_search::Context {
                        backend: state.backend.clone(),
                        thread: thread.id.clone(),
                        generation: state.socket_generation,
                    },
                    thread: thread.clone(),
                    items: crate::session_resources::latest(&state.items),
                })
        };
        self.resources.sync(input);
    }
    pub(super) fn open_resources(self: &Rc<Self>) {
        self.sync_resources();
        self.show_tool("resources");
        self.resources.focus_search();
    }
    pub(super) fn connect_resources(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        *self.resources.on_close.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.close_tool("resources");
            }
        }));
        let weak = Rc::downgrade(self);
        *self.resources.on_open.borrow_mut() =
            Some(Box::new(move |input, resource, occurrence| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let input = source_input(input, &occurrence);
                if resource.blocked.is_some() {
                    return;
                }
                if resource.directory {
                    match crate::links::resolve(
                        &resource.address,
                        std::path::Path::new(&input.thread.cwd),
                        None,
                    ) {
                        Ok(crate::links::Target::Document { path, .. }) => {
                            match ui.files.open_folder(&input.thread.cwd, &path) {
                                Ok(()) => ui.present_viewer("files"),
                                Err(error) => ui.error(&error),
                            }
                        }
                        _ => ui.error("Unable to open resource"),
                    }
                } else {
                    ui.activate_reference_from(&resource.address, &input.thread.cwd);
                }
            }));
        let weak = Rc::downgrade(self);
        *self.resources.on_source.borrow_mut() =
            Some(Box::new(move |input, _resource, occurrence| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let mut input = source_input(input, &occurrence);
                let serial = {
                    let mut state = ui.state.borrow_mut();
                    state.sequence += 1;
                    input.context.generation = *state
                        .backend_generations
                        .get(&input.context.backend)
                        .unwrap_or(&input.context.generation);
                    state.sequence
                };
                let origin = occurrence.item.origin.as_ref();
                let snippet = occurrence.excerpt.clone();
                let highlight = 0..snippet.len();
                let hit = crate::session_search::Occurrence {
                    turn_id: origin
                        .map_or_else(|| occurrence.item.turn_id.clone(), |o| o.turn.clone()),
                    item_id: origin.map_or_else(|| occurrence.item.id.clone(), |o| o.item.clone()),
                    kind: if occurrence.item.role == "You" {
                        crate::session_search::Filter::User
                    } else {
                        crate::session_search::Filter::Assistant
                    },
                    snippet,
                    highlight,
                    source: if origin.is_none() {
                        Some(occurrence.source)
                    } else {
                        None
                    },
                    block: None,
                    table_row: None,
                    paragraph_page: None,
                    paragraph_range: None,
                };
                ui.present_history_context(
                    input.context,
                    serial,
                    input.thread.title,
                    input.thread.cwd,
                    hit,
                );
            }));
        let weak = Rc::downgrade(self);
        *self.resources.on_favorite.borrow_mut() =
            Some(Box::new(move |input, resource, occurrence| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let input = source_input(input, &occurrence);
                let origin = occurrence.item.origin.as_ref();
                let turn_id =
                    origin.map_or_else(|| occurrence.item.turn_id.clone(), |o| o.turn.clone());
                let item_id = origin.map_or_else(|| occurrence.item.id.clone(), |o| o.item.clone());
                if turn_id.is_empty() || item_id.is_empty() {
                    ui.error("This response has no persistent source anchor");
                    return;
                }
                let library = ui.favorites_library();
                if library.busy() {
                    ui.notice("Save or discard the current favorite before adding another");
                    return;
                }
                let question = origin
                    .map(|o| o.question.as_ref().clone())
                    .unwrap_or_else(|| {
                        input
                            .items
                            .iter()
                            .find(|i| i.role == "You" && i.turn_id == turn_id)
                            .map(|i| i.body.clone())
                            .unwrap_or_default()
                    });
                let content = if resource.kind == crate::session_resources::Filter::Web {
                    resource.address.clone()
                } else {
                    format!("`{}`", resource.address.replace('`', ""))
                };
                library.draft(crate::favorites::Favorite {
                    id: glib::uuid_string_random().to_string(),
                    scope: "selection".into(),
                    backend: input.context.backend,
                    thread_id: input.thread.id,
                    thread_title: input.thread.title,
                    project_path: input.thread.cwd,
                    turn_id,
                    item_id,
                    title: resource.title.clone(),
                    question,
                    content,
                    note: String::new(),
                    tags: Vec::new(),
                    created_at: glib::DateTime::now_utc()
                        .and_then(|d| d.format_iso8601())
                        .map(|d| d.to_string())
                        .unwrap_or_else(|_| "unknown".into()),
                });
            }));
    }
}
fn source_input(mut input: Input, occurrence: &crate::session_resources::Occurrence) -> Input {
    if let Some(origin) = &occurrence.item.origin {
        input.context.backend = origin.backend.clone();
        input.context.thread = origin.thread.clone();
        input.thread.id = origin.thread.clone();
        input.thread.title = origin.title.clone();
        input.thread.cwd = origin.root.clone();
    }
    input
}
