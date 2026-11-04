use super::*;
impl Ui {
    pub(super) fn connect_session_map(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.session_map
            .context
            .replace(Some(Box::new(move || weak.upgrade()?.native_map_context())));
        let weak = Rc::downgrade(self);
        self.session_map.capture.replace(Some(Box::new(move || {
            let ui = weak.upgrade()?;
            let context = ui.native_map_context()?;
            let state = ui.state.borrow();
            // Only cheap Arc references cross GTK; text selection/packing happens on a worker.
            let n = state.items.len();
            let source = state
                .items
                .iter()
                .take(32)
                .chain(
                    state
                        .items
                        .iter()
                        .skip(n.saturating_sub(256).max(32.min(n))),
                )
                .cloned()
                .collect();
            Some(crate::session_map_ui::Capture {
                context,
                effort: state.options.effort.clone(),
                source,
            })
        })));
    }
    fn native_map_context(&self) -> Option<crate::translation::Context> {
        let state = self.state.borrow();
        let thread = state.selected.as_ref()?;
        let backend = state
            .backends
            .iter()
            .find(|b| b.id == state.backend)?
            .clone();
        let model = if !state.options.model.is_empty() {
            &state.options.model
        } else if !state.session_model.is_empty() {
            &state.session_model
        } else {
            &state.default_model
        }
        .clone();
        Some(crate::translation::Context {
            backend,
            socket: state.socket_generation,
            thread: thread.id.clone(),
            cwd: thread.cwd.clone(),
            model,
            ready: state.ready,
        })
    }
    pub(super) fn open_session_map(self: &Rc<Self>) {
        self.show_tool("map");
        self.session_map.open();
    }
}
