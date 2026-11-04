use super::*;
impl Ui {
    pub(super) fn mcp_disconnected(&self, backend: &str, generation: u64) {
        let panel = self.mcp.borrow().clone();
        if let Some(panel) = panel.filter(|panel| {
            panel.context.backend == backend && panel.context.generation == generation
        }) {
            panel
                .error("The backend disconnected; close and reopen MCP servers after reconnecting");
        }
    }
    pub(super) fn open_mcp(self: &Rc<Self>, context: crate::composer_tools_ui::Context) {
        let current = self.mcp.borrow().clone();
        if let Some(panel) = current.filter(|panel| panel.context == context) {
            panel.dialog.present(Some(&self.window));
            return;
        }
        let previous = self.mcp.borrow_mut().take();
        if let Some(previous) = previous {
            previous.dialog.force_close();
        }
        let panel = crate::mcp_ui::Panel::new(context);
        let weak = Rc::downgrade(self);
        let id = panel.id;
        *panel.on_load.borrow_mut() = Some(Box::new(move |cursor| {
            if let Some(ui) = weak.upgrade() {
                ui.request_mcp(id, cursor);
            }
        }));
        let weak = Rc::downgrade(self);
        panel.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui.mcp.borrow().as_ref().is_some_and(|panel| panel.id == id) {
                    ui.mcp.borrow_mut().take();
                }
            }
        });
        self.mcp.replace(Some(panel.clone()));
        panel.dialog.present(Some(&self.window));
        panel.begin();
    }
    fn request_mcp(self: &Rc<Self>, dialog: u64, cursor: Option<String>) {
        let panel = self.mcp.borrow().clone();
        let Some(panel) = panel.filter(|panel| panel.id == dialog) else {
            return;
        };
        if self.command_context().as_ref() != Some(&panel.context) {
            panel.error("The session or connection changed; close and reopen MCP servers");
            return;
        }
        let thread = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.id.clone())
            .unwrap_or_default();
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let revision = panel.revision.get();
        if !self.command(Command::Rpc {
            backend: panel.context.backend.clone(),
            generation: panel.context.generation,
            id,
            method: "mcpServerStatus/list".into(),
            params: json!({"threadId":thread,"cursor":cursor,"limit":100}),
            root: None,
            images: vec![],
        }) {
            panel.error("The backend is busy or disconnected; retry after reconnecting");
            return;
        }
        self.state.borrow_mut().pending.insert(
            id,
            PendingRequest {
                backend: panel.context.backend.clone(),
                generation: panel.context.generation,
                action: Pending::Mcp { dialog, revision },
            },
        );
        panel.pending.set(id);
        let weak = Rc::downgrade(&panel);
        glib::timeout_add_local_once(Duration::from_secs(35), move || {
            if let Some(panel) = weak.upgrade() {
                if panel.revision.get() == revision && panel.pending.get() == id {
                    panel.error("MCP status timed out; reconnect and retry");
                }
            }
        });
    }
    pub(super) fn mcp_response(
        self: &Rc<Self>,
        dialog: u64,
        revision: u64,
        payload: &Value,
        prepared: protocol::Prepared,
    ) {
        let panel = self.mcp.borrow().clone();
        let Some(panel) = panel.filter(|panel| {
            panel.id == dialog && panel.revision.get() == revision && panel.pending.get() != 0
        }) else {
            return;
        };
        if self.command_context().as_ref() != Some(&panel.context) {
            panel.error("The session or connection changed; close and reopen MCP servers");
            return;
        }
        let result = if let Some(error) = payload.get("error") {
            Err(model::text(error, "message").to_owned())
        } else {
            prepared
                .mcp_page
                .unwrap_or_else(|| Err("Invalid MCP inventory response".into()))
        };
        match result.and_then(|page| panel.page(page)) {
            Ok(Some(cursor)) => self.request_mcp(dialog, Some(cursor)),
            Ok(None) => {}
            Err(error) => panel.error(&error),
        }
    }
}

#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: &impl Fn(&adw::Dialog, &str),
) -> Result<(), String> {
    async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !ready() {
            if start.elapsed() > Duration::from_secs(7) {
                return Err("Native MCP operation timed out".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    let draft = ui.composer_text();
    ui.run_command("mcp", None)?;
    let panel = ui.mcp.borrow().clone().ok_or("MCP did not open")?;
    until(|| panel.pending.get() == 0 && panel.smoke_names().len() == 2).await?;
    if panel.smoke_names() != ["Native MCP 中文", "Native MCP second"]
        || ui.composer_text() != draft
    {
        return Err("MCP pagination changed its server inventory or the composer".into());
    }
    glib::timeout_future(Duration::from_millis(40)).await;
    capture(&panel.dialog, "mcp-servers.png");
    let cycle = directory.join("native-mcp-cycle.flag");
    std::fs::write(&cycle, b"cycle").map_err(|error| error.to_string())?;
    panel.begin();
    until(|| panel.pending.get() == 0 && panel.smoke_status().contains("repeated")).await?;
    if panel.smoke_names().len() != 1 {
        return Err("Cyclic MCP cursor duplicated the inventory".into());
    }
    std::fs::remove_file(&cycle).map_err(|error| error.to_string())?;
    let delay = directory.join("native-mcp-delay.flag");
    std::fs::write(&delay, b"delay").map_err(|error| error.to_string())?;
    panel.begin();
    until(|| !delay.exists()).await?;
    let old_id = panel.id;
    panel.dialog.close();
    until(|| ui.mcp.borrow().is_none()).await?;
    ui.run_command("mcp", None)?;
    let replacement = ui.mcp.borrow().clone().unwrap();
    until(|| replacement.pending.get() == 0 && replacement.smoke_names().len() == 2).await?;
    glib::timeout_future(Duration::from_millis(450)).await;
    if replacement.id == old_id || replacement.smoke_names().len() != 2 {
        return Err("An old MCP reply changed its replacement dialog".into());
    }
    let selected = ui.state.borrow().selected.clone().unwrap();
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("No MCP sibling fixture")?;
    std::fs::write(&delay, b"delay").map_err(|error| error.to_string())?;
    replacement.begin();
    until(|| !delay.exists()).await?;
    ui.select(sibling);
    until(|| !ui.state.borrow().loading).await?;
    until(|| {
        replacement
            .smoke_status()
            .contains("session or connection changed")
    })
    .await?;
    replacement.dialog.close();
    until(|| ui.mcp.borrow().is_none()).await?;
    ui.select(selected);
    until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
    if ui.composer_text() != draft {
        return Err("MCP navigation discarded the composer draft".into());
    }
    Ok(())
}
