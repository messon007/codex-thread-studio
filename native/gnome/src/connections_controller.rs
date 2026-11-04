use super::*;
impl Ui {
    pub(super) fn sync_connections(&self) {
        if let Some(panel) = self.connections.borrow().clone() {
            let state = self.state.borrow();
            panel.sync(crate::connections_ui::Runtime {
                ready: state.backend_ready.clone(),
                known: state.backend_generations.keys().cloned().collect(),
                stopped: self.gateway_stopped.get(),
            });
        }
    }
    pub(super) fn open_connections(self: &Rc<Self>) {
        if let Some(panel) = self.connections.borrow().clone() {
            panel.dialog.present(Some(&self.window));
            return;
        }
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let panel = crate::connections_ui::Panel::new(self.sender.clone(), id);
        let weak = Rc::downgrade(self);
        panel.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui.connections.borrow().as_ref().is_some_and(|p| p.id == id) {
                    ui.connections.borrow_mut().take();
                }
            }
        });
        self.connections.replace(Some(panel.clone()));
        self.sync_connections();
        panel.dialog.present(Some(&self.window));
        panel.read();
    }
}
