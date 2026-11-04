use super::*;
use crate::{approval_ui::Choices, interaction_ui::Scope};
use std::cell::Cell;

impl Ui {
    pub(super) fn present_approval(
        self: &Rc<Self>,
        scope: Scope,
        id: Value,
        notice: Option<String>,
        origin: &str,
        prepared: Option<&Result<Arc<crate::approval::Request>, String>>,
        details: &str,
    ) {
        let (dialog, choices) = crate::approval_ui::create(&self.window, origin, prepared, details);
        let choices = Rc::new(choices);
        let finished = Rc::new(Cell::new(false));
        let pending = Rc::new(Cell::new(false));
        let serial = Rc::new(Cell::new(0u64));
        let cancelling = Rc::new(Cell::new(false));
        self.approvals.borrow_mut().insert(
            scope.clone(),
            crate::interaction_ui::Approval {
                dialog: dialog.clone(),
                finished: finished.clone(),
                pending: pending.clone(),
                notice: notice.clone(),
            },
        );
        let weak = Rc::downgrade(self);
        // AdwMessageDialog's button handler calls gtk_window_close *before*
        // emitting response. Defer that close; a button's synchronous response
        // replaces this fallback. A real window close invalidates pending work
        // immediately and then takes the non-grant close response on idle.
        let (close_finished, close_pending, close_serial, close_cancelling) = (
            finished.clone(),
            pending.clone(),
            serial.clone(),
            cancelling.clone(),
        );
        dialog.connect_close_request(move |dialog| {
            if close_finished.get() {
                return glib::Propagation::Proceed;
            }
            close_serial.set(close_serial.get().wrapping_add(1));
            close_pending.set(false);
            close_cancelling.set(false);
            let revision = close_serial.get();
            let (finished, serial, dialog) = (
                close_finished.clone(),
                close_serial.clone(),
                dialog.downgrade(),
            );
            glib::idle_add_local_once(move || {
                if !finished.get() && serial.get() == revision {
                    if let Some(dialog) = dialog.upgrade() {
                        dialog.response(&dialog.close_response());
                    }
                }
            });
            glib::Propagation::Stop
        });
        dialog.connect_response(None, move |dialog, response| {
            if finished.get() {
                return;
            }
            if pending.get() && (!Choices::cancellation(response) || cancelling.get()) {
                return;
            }
            cancelling.set(Choices::cancellation(response));
            pending.set(true);
            serial.set(serial.get().wrapping_add(1));
            let revision = serial.get();
            choices.busy(dialog, true);
            let input = choices.capture(response);
            let task = gio::spawn_blocking(move || input.resolve());
            let (weak, finished, pending, serial, choices, scope, id, notice, dialog) = (
                weak.clone(),
                finished.clone(),
                pending.clone(),
                serial.clone(),
                choices.clone(),
                scope.clone(),
                id.clone(),
                notice.clone(),
                dialog.downgrade(),
            );
            glib::spawn_future_local(async move {
                let resolved = task.await;
                if finished.get() || serial.get() != revision {
                    return;
                }
                let (Some(ui), Some(dialog)) = (weak.upgrade(), dialog.upgrade()) else {
                    return;
                };
                let valid = {
                    let state = ui.state.borrow();
                    state.backend_generations.get(&scope.backend) == Some(&scope.generation)
                        && state.backend_ready.contains(&scope.backend)
                };
                if !valid {
                    finished.set(true);
                    pending.set(false);
                    ui.approvals.borrow_mut().remove(&scope);
                    if let Some(notice) = notice {
                        ui.notifications.withdraw(&notice);
                    }
                    dialog.close();
                    return;
                }
                let result = match resolved {
                    Ok(Ok(value)) => value,
                    Ok(Err(error)) => {
                        pending.set(false);
                        choices.busy(&dialog, false);
                        choices.report(&error);
                        return;
                    }
                    Err(_) => {
                        pending.set(false);
                        choices.busy(&dialog, false);
                        choices.report("Permission validation stopped; your selection is retained");
                        return;
                    }
                };
                if !ui.command(Command::Reply {
                    id,
                    result,
                    backend: scope.backend.clone(),
                    generation: scope.generation,
                }) {
                    pending.set(false);
                    choices.busy(&dialog, false);
                    choices.report(
                        "The backend input queue is busy; your selection is retained. Try again.",
                    );
                    return;
                }
                finished.set(true);
                pending.set(false);
                ui.approvals.borrow_mut().remove(&scope);
                if let Some(notice) = notice {
                    ui.notifications.withdraw(&notice);
                }
                dialog.close();
            });
        });
        dialog.present();
    }
}
