//! Conversation integration; paging does not mutate the live delivery model.
use super::*;
impl Ui {
    pub(super) fn connect_history_pages(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        *self.history_pages.on_present.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.chat_diagrams.reset();
                ui.state.borrow_mut().render_dirty = true;
                ui.render_items();
                if ui.finder.widget.is_visible() {
                    ui.finder.perform();
                }
            }
        }));
    }
    pub(super) fn sync_history_pages(&self) {
        let (source, enabled) = {
            let state = self.state.borrow();
            let source = state
                .selected
                .as_ref()
                .filter(|_| state.ready && !self.router_selected())
                .map(|thread| crate::conversation_pages::Source {
                    context: crate::session_search::Context {
                        backend: state.backend.clone(),
                        thread: thread.id.clone(),
                        generation: state.socket_generation,
                    },
                    backend_kind: state
                        .backends
                        .iter()
                        .find(|b| b.id == state.backend)
                        .map(|b| b.kind.clone())
                        .unwrap_or_default(),
                    directory: thread.cwd.clone(),
                });
            let enabled = source.is_some()
                && !state.loading
                && !state.session_mutation
                && self.router.borrow().registry.is_some();
            (source, enabled)
        };
        self.history_pages.sync(source, enabled);
        if let Some(action) = self
            .window
            .lookup_action("history")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(enabled);
        }
    }
    pub(super) fn open_history_pages(self: &Rc<Self>) {
        self.sync_history_pages();
        self.history_pages.open();
    }
}
