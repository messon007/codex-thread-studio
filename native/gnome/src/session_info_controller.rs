use super::*;
use crate::{
    conversation_pages::Source,
    session_info_ui::{Input, Ticket},
};
impl Ui {
    fn information_valid(&self, source: &Source) -> bool {
        let state = self.state.borrow();
        state.backend == source.context.backend
            && state.socket_generation == source.context.generation
            && state
                .selected
                .as_ref()
                .is_some_and(|t| t.id == source.context.thread && t.cwd == source.directory)
            && !state.session_mutation
    }
    fn information_generation_allowed(&self, source: &Source) -> bool {
        if !self.information_valid(source) || self.router_selected() {
            return false;
        }
        let state = self.state.borrow();
        state.ready
            && !state.loading
            && state.history_verified
            && state.options_loaded
            && !state.archived
            && !state.sending
            && state.active_turn.is_none()
            && !self.queue_busy(&state)
            && state.selected.as_ref().is_some_and(|t| t.parent.is_none())
            && !state.shells.contains(&format!(
                "{}:{}",
                source.context.backend, source.context.thread
            ))
            && self.router.borrow().registry.is_some()
    }
    pub(super) fn sync_session_info(&self) {
        let panel = self.session_info.borrow().clone();
        if let Some(panel) = panel {
            let active = self.state.borrow().active_turn.clone();
            panel.sync(
                self.information_valid(&panel.input.source),
                self.information_generation_allowed(&panel.input.source),
                active.as_deref(),
            );
        }
    }
    pub(super) fn open_session_info(self: &Rc<Self>) {
        if let Some(panel) = self.session_info.borrow().clone() {
            panel.dialog.present(Some(&self.window));
            return;
        }
        let input = {
            let state = self.state.borrow();
            let Some(thread) = state.selected.clone() else {
                drop(state);
                self.notice("Select a session to view its information");
                return;
            };
            let Some(backend) = state.backends.iter().find(|b| b.id == state.backend) else {
                return;
            };
            Input {
                source: Source {
                    context: crate::session_search::Context {
                        backend: state.backend.clone(),
                        thread: thread.id.clone(),
                        generation: state.socket_generation,
                    },
                    backend_kind: backend.kind.clone(),
                    directory: thread.cwd.clone(),
                },
                title: thread.title.clone(),
                backend_name: backend.name.clone(),
                routable: thread.parent.is_none() && !self.router_selected(),
                capture: !self.router_selected(),
            }
        };
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let panel =
            crate::session_info_ui::Panel::new(&self.window, self.sender.clone(), input, id);
        let weak = Rc::downgrade(self);
        let source = panel.input.source.clone();
        *panel.on_generate.borrow_mut() = Some(Box::new(move |ticket, opening| {
            let ui = weak.upgrade().ok_or("Session is closed")?;
            ui.generate_information(&source, ticket, opening)
        }));
        let weak = Rc::downgrade(self);
        let source = panel.input.source.clone();
        *panel.on_stop.borrow_mut() = Some(Box::new(move |turn| {
            let ui = weak.upgrade().ok_or("Session is closed")?;
            if !ui.information_valid(&source)
                || !crate::session_info::can_stop(
                    &source,
                    turn,
                    ui.state.borrow().active_turn.as_deref(),
                )
            {
                return Err(
                    "The active turn changed; inspect the conversation before stopping".into(),
                );
            }
            if source.backend_kind == "opencode" {
                ui.pause_selected_queue("Stopped by user");
                ui.rpc(Pending::Stop(source.context.thread.clone()), "turn/interrupt",
                    json!({"threadId":source.context.thread,"turnId":turn,"expectedUserMessageId":turn}));
            } else {
                ui.interrupt();
            }
            Ok(())
        }));
        let weak = Rc::downgrade(self);
        panel.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                let current = ui
                    .session_info
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.id == id);
                if current {
                    ui.session_info.borrow_mut().take();
                }
            }
        });
        self.session_info.replace(Some(panel.clone()));
        self.sync_session_info();
        panel.dialog.present(Some(&self.window));
        panel.begin();
    }
    fn generate_information(
        &self,
        source: &Source,
        ticket: Ticket,
        opening: String,
    ) -> Result<(), String> {
        if !self.information_generation_allowed(source) {
            return Err("Wait for the current turn to finish and the session to be ready before generating a responsibility".into());
        }
        let prompt = crate::session_info::generation_prompt(&opening);
        let mut params =
            json!({"threadId":source.context.thread,"input":[{"type":"text","text":prompt}]});
        {
            let state = self.state.borrow();
            let codex = source.backend_kind == "codex";
            state.options.apply(&mut params, codex);
            state.policies.apply(
                &crate::session_info::key(source),
                &source.directory,
                codex,
                &mut params,
            )?;
        }
        // This intentionally starts a normal session turn, like the existing UI.
        // It neither consumes nor rewrites the composer, attachments or references.
        if self.history_pages.widget.is_visible() {
            self.history_pages.close();
        }
        self.state.borrow_mut().sending = true;
        self.queue_manual_start(&source.context.thread);
        self.update_composer();
        self.rpc(
            Pending::Send {
                information: Some(ticket),
                annotation: None,
                thread: source.context.thread.clone(),
                text: prompt,
                images: Vec::new(),
                references: Vec::new(),
            },
            "turn/start",
            params,
        );
        Ok(())
    }
    pub(super) fn information_ack(&self, ticket: Ticket, result: Result<String, String>) {
        let panel = self.session_info.borrow().clone();
        if let Some(panel) = panel.filter(|p| p.id == ticket.dialog) {
            panel.acknowledged(ticket, result);
        }
    }
}
