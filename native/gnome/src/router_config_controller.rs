use super::*;
impl Ui {
    pub(super) fn open_router_settings(self: &Rc<Self>) {
        let panel = self.router_settings.borrow().clone();
        if let Some(panel) = panel {
            panel.dialog.present(Some(&self.window));
            return;
        }
        let (backends, title) = {
            let state = self.state.borrow();
            (
                state.backends.clone(),
                state
                    .selected
                    .as_ref()
                    .map(|thread| thread.title.clone())
                    .unwrap_or_default(),
            )
        };
        if backends.is_empty() {
            self.error("Wait for the backend registry to load");
            return;
        }
        let context = self
            .command_context()
            .filter(|_| self.state.borrow().selected.is_some());
        let panel = crate::router_config_ui::Panel::new(
            &self.window,
            self.sender.clone(),
            backends,
            context.clone(),
            title,
        );
        let id = panel.id;
        let weak = Rc::downgrade(self);
        *panel.role_valid.borrow_mut() = Some(Box::new(move || {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let Some(context) = &context else {
                return false;
            };
            let state = ui.state.borrow();
            state.backend == context.backend
                && state.backend_generations.get(&context.backend) == Some(&context.generation)
                && state.selected.as_ref().is_some_and(|thread| {
                    format!("{}:{}", state.backend, thread.id) == context.key
                        && thread.cwd == context.root
                })
                && !state.session_mutation
        }));
        let weak = Rc::downgrade(self);
        panel.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui
                    .router_settings
                    .borrow()
                    .as_ref()
                    .is_some_and(|panel| panel.id == id)
                {
                    ui.router_settings.borrow_mut().take();
                }
            }
        });
        self.router_settings.replace(Some(panel.clone()));
        panel.dialog.present(Some(&self.window));
        panel.begin();
    }
}

#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&adw::Dialog, &str),
) -> Result<Value, String> {
    async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !ready() {
            if start.elapsed() > Duration::from_secs(8) {
                return Err("Router settings scope test timed out".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    let original = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("No responsibility test session")?;
    let draft = ui.composer_text();
    ui.open_router_settings();
    let panel = ui
        .router_settings
        .borrow()
        .clone()
        .ok_or("Router settings did not open")?;
    let result = match panel.smoke_test(directory, capture).await {
        Ok(value) => value,
        Err(error) => {
            let details = panel.smoke_diagnostics();
            panel.smoke_cleanup();
            return Err(format!("{error}; {details}"));
        }
    };
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("No responsibility scope sibling")?;
    ui.select(sibling);
    until(|| !ui.state.borrow().loading).await?;
    panel.smoke_scope_failure().await?;
    until(|| ui.router_settings.borrow().is_none()).await?;
    ui.select(original);
    until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
    if ui.composer_text() != draft {
        return Err("Router settings navigation discarded the composer draft".into());
    }
    Ok(result)
}
