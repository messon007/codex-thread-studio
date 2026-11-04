use super::*;
use crate::interaction_ui::{Panel, Scope};
impl Ui {
    pub(super) fn native_input(
        self: &Rc<Self>,
        payload: &Value,
        prepared: Option<&Result<Arc<crate::interaction::Request>, String>>,
        backend: &str,
        generation: u64,
        notice: Option<String>,
    ) {
        let Some(scope) = Scope::new(backend, generation, &payload["id"]) else {
            self.error("Invalid agent input request identity");
            return;
        };
        if self.interactions.borrow().contains_key(&scope) {
            return;
        }
        if self.interactions.borrow().len() + self.approvals.borrow().len() >= 8 {
            self.error("Resolve an existing input request before opening another");
            let _ = self.command(Command::Reply {
                id: payload["id"].clone(),
                result: Err(protocol::ReplyError {
                    code: -32000,
                    message: "Native input dialog capacity reached".into(),
                }),
                backend: backend.into(),
                generation,
            });
            if let Some(notice) = notice {
                self.notifications.withdraw(&notice);
            }
            return;
        }
        let origin = {
            let state = self.state.borrow();
            let name = state
                .backends
                .iter()
                .find(|b| b.id == backend)
                .map(|b| b.name.as_str())
                .unwrap_or(backend);
            let mut origin = format!(
                "{name} · {}",
                payload["params"]["threadId"]
                    .as_str()
                    .unwrap_or("Workspace")
            );
            if let Some(cwd) = payload["params"]["cwd"].as_str() {
                origin.push_str(" · ");
                origin.push_str(cwd);
            }
            origin
        };
        let panel = Panel::new(
            &self.window,
            &origin,
            prepared.cloned().unwrap_or_else(|| {
                Err("Input data is unavailable; reconnect before retrying".into())
            }),
            payload["method"].as_str().unwrap_or(""),
        );
        let id = payload["id"].clone();
        let target = scope.clone();
        let weak = Rc::downgrade(self);
        *panel.on_reply.borrow_mut() = Some(Box::new(move |result| {
            let ui = weak.upgrade().ok_or("Studio closed")?;
            if ui.state.borrow().backend_generations.get(&target.backend)
                != Some(&target.generation)
                || !ui.state.borrow().backend_ready.contains(&target.backend)
            {
                return Err("The backend connection changed; this response was not sent".into());
            }
            if !ui.command(Command::Reply {
                id: id.clone(),
                result,
                backend: target.backend.clone(),
                generation: target.generation,
            }) {
                return Err("The backend input queue is busy; your responses are retained".into());
            }
            Ok(())
        }));
        let target = scope.clone();
        let weak = Rc::downgrade(self);
        *panel.on_closed.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.interactions.borrow_mut().remove(&target);
                if let Some(notice) = &notice {
                    ui.notifications.withdraw(notice);
                }
            }
        }));
        self.interactions.borrow_mut().insert(scope, panel.clone());
        panel.dialog.present(Some(&self.window));
        panel.focus_initial();
    }
    pub(super) fn expire_inputs(
        &self,
        backend: Option<&str>,
        generation: Option<u64>,
        id: Option<&Value>,
    ) {
        let id = id.map(Value::to_string);
        let closing = {
            let mut book = self.interactions.borrow_mut();
            let keys = book
                .keys()
                .filter(|key| key.matches(backend, generation, id.as_deref()))
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| book.remove(&key))
                .collect::<Vec<_>>()
        };
        for panel in closing {
            panel.expire();
        }
        let closing = {
            let mut book = self.approvals.borrow_mut();
            let keys = book
                .keys()
                .filter(|key| key.matches(backend, generation, id.as_deref()))
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| book.remove(&key))
                .collect::<Vec<_>>()
        };
        for approval in closing {
            approval.finished.set(true);
            approval.pending.set(false);
            if let Some(notice) = approval.notice {
                self.notifications.withdraw(&notice);
            }
            approval.dialog.close();
        }
    }
}
