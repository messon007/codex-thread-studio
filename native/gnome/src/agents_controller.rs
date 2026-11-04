use super::*;
use crate::agents;
impl Ui {
    pub(super) fn open_agents(self: &Rc<Self>, context: crate::composer_tools_ui::Context) {
        let current = self.agents.borrow().clone();
        if let Some(panel) = current.filter(|panel| panel.context == context) {
            panel.dialog.present(Some(&self.window));
            return;
        }
        let previous = self.agents.borrow_mut().take();
        if let Some(previous) = previous {
            previous.dialog.force_close();
        }
        let selected = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.id.clone())
            .unwrap_or_default();
        let panel = crate::agents_ui::Panel::new(context.clone(), selected);
        let id = panel.id;
        let weak = Rc::downgrade(self);
        *panel.on_load.borrow_mut() = Some(Box::new(move |step| {
            if let Some(ui) = weak.upgrade() {
                ui.request_agents(id, step);
            }
        }));
        let weak = Rc::downgrade(self);
        let scope = context;
        *panel.on_open.borrow_mut() = Some(Box::new(move |thread| {
            if let Some(ui) = weak.upgrade() {
                if ui.command_context().as_ref() != Some(&scope) {
                    if let Some(panel) = ui.agents.borrow().as_ref() {
                        panel.error("The session or connection changed; reload Agents from the current session");
                    }
                    return;
                }
                let panel = ui.agents.borrow().clone();
                if let Some(panel) = panel.filter(|panel| panel.id == id) {
                    panel.dialog.close();
                }
                {
                    let mut state = ui.state.borrow_mut();
                    if !state
                        .threads
                        .iter()
                        .any(|candidate| candidate.id == thread.id)
                    {
                        state.threads.insert(0, thread.clone());
                    }
                }
                ui.select(thread);
                ui.render_sessions();
            }
        }));
        let weak = Rc::downgrade(self);
        panel.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui
                    .agents
                    .borrow()
                    .as_ref()
                    .is_some_and(|panel| panel.id == id)
                {
                    ui.agents.borrow_mut().take();
                }
            }
        });
        self.agents.replace(Some(panel.clone()));
        panel.dialog.present(Some(&self.window));
        panel.begin();
    }
    fn request_agents(self: &Rc<Self>, dialog: u64, step: agents::Step) {
        let panel = self.agents.borrow().clone();
        let Some(panel) = panel.filter(|panel| panel.id == dialog) else {
            return;
        };
        if self.command_context().as_ref() != Some(&panel.context) {
            panel.error("The session or connection changed; close and reopen Agents");
            return;
        }
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let revision = panel.revision.get();
        let (method, params) = step.rpc();
        if !self.command(Command::Rpc {
            backend: panel.context.backend.clone(),
            generation: panel.context.generation,
            id,
            method: method.into(),
            params,
            root: None,
            images: vec![],
        }) {
            panel.error("The backend is busy or disconnected; reopen Agents after reconnecting");
            return;
        }
        self.state.borrow_mut().pending.insert(
            id,
            PendingRequest {
                backend: panel.context.backend.clone(),
                generation: panel.context.generation,
                action: Pending::Agents {
                    dialog,
                    revision,
                    step,
                },
            },
        );
        panel.pending.set(id);
        let weak = Rc::downgrade(&panel);
        glib::timeout_add_local_once(Duration::from_secs(35), move || {
            if let Some(panel) = weak.upgrade() {
                if panel.pending.get() == id && panel.revision.get() == revision {
                    panel.error("Agent discovery timed out; reconnect and reload Agents");
                }
            }
        });
    }
    pub(super) fn agents_response(
        self: &Rc<Self>,
        dialog: u64,
        revision: u64,
        step: agents::Step,
        payload: &Value,
        prepared: protocol::Prepared,
    ) {
        let panel = self.agents.borrow().clone();
        let Some(panel) = panel.filter(|panel| {
            panel.id == dialog && panel.revision.get() == revision && panel.pending.get() != 0
        }) else {
            return;
        };
        if self.command_context().as_ref() != Some(&panel.context) {
            panel.error("The session or connection changed; close and reopen Agents");
            return;
        }
        if let Some(error) = payload.get("error") {
            panel.error(
                error["message"]
                    .as_str()
                    .unwrap_or("Agent discovery failed"),
            );
            return;
        }
        let next = {
            let mut discovery = panel.discovery.borrow_mut();
            let Some(discovery) = discovery.as_mut() else {
                return;
            };
            match step {
                agents::Step::Read(expected) => prepared
                    .thread_summary
                    .unwrap_or_else(|| Err("No agent summary was returned".into()))
                    .and_then(|agent| discovery.summary(&expected, agent))
                    .map(Some),
                agents::Step::List(_) => prepared
                    .catalog
                    .unwrap_or_else(|| Err("Invalid agent catalog response".into()))
                    .and_then(|page| discovery.page(page)),
            }
        };
        match next {
            Ok(Some(step)) => {
                let count = panel
                    .discovery
                    .borrow()
                    .as_ref()
                    .map_or(0, agents::Discovery::scanned);
                if matches!(&step, agents::Step::List(_)) {
                    panel.progress(count);
                }
                self.request_agents(dialog, step);
            }
            Ok(None) => {
                let discovery = panel.discovery.borrow_mut().take().unwrap();
                panel.computing();
                if !self.command(Command::AgentTree {
                    dialog,
                    revision,
                    discovery,
                }) {
                    panel.error("The worker is busy; reload Agents");
                }
            }
            Err(error) => panel.error(&error),
        }
    }
    pub(super) fn agents_tree(
        &self,
        dialog: u64,
        revision: u64,
        result: Result<agents::Tree, String>,
    ) {
        let panel = self.agents.borrow().clone();
        let Some(panel) =
            panel.filter(|panel| panel.id == dialog && panel.revision.get() == revision)
        else {
            return;
        };
        if self.command_context().as_ref() != Some(&panel.context) {
            panel.error("The session or connection changed; close and reopen Agents");
            return;
        }
        match result {
            Ok(tree) => panel.apply(tree),
            Err(error) => panel.error(&error),
        }
    }
    pub(super) fn agents_disconnected(&self, backend: &str, generation: u64) {
        let panel = self.agents.borrow().clone();
        if let Some(panel) = panel.filter(|panel| {
            panel.context.backend == backend && panel.context.generation == generation
        }) {
            panel.error("The backend disconnected; reopen Agents after reconnecting");
        }
    }
}

#[cfg(debug_assertions)]
async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !ready() {
        if start.elapsed() > Duration::from_secs(8) {
            return Err("Native agent operation timed out".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
#[cfg(debug_assertions)]
async fn loaded(ui: &Rc<Ui>, id: &str) -> Result<(), String> {
    until(|| {
        let state = ui.state.borrow();
        state
            .selected
            .as_ref()
            .is_some_and(|thread| thread.id == id)
            && !state.loading
            && state.options_loaded
    })
    .await
}
#[cfg(debug_assertions)]
async fn opened(ui: &Rc<Ui>) -> Result<Rc<crate::agents_ui::Panel>, String> {
    ui.run_command("subagents", None)?;
    let panel = ui.agents.borrow().clone().ok_or("Agents did not open")?;
    until(|| !panel.smoke_ids().is_empty()).await?;
    glib::timeout_future(Duration::from_millis(50)).await;
    Ok(panel)
}
#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: &impl Fn(&adw::Dialog, &str),
) -> Result<Value, String> {
    let draft = ui.composer_text();
    let root = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("No agent test session")?;
    ui.composer.buffer().set_text("Native root draft 中文😀");
    let panel = opened(ui).await?;
    let expected = [
        "native-smoke-thread",
        "native-agent-before",
        "native-agent-child",
        "native-agent-grandchild",
    ];
    if panel.smoke_ids() != expected {
        return Err(format!(
            "Agent membership/order mismatch: {:?}",
            panel.smoke_ids()
        ));
    }
    capture(&panel.dialog, "agents.png");
    if panel.smoke_open(&root.id).is_ok() {
        return Err("The current agent can be reopened".into());
    }
    panel.smoke_open("native-agent-child")?;
    loaded(ui, "native-agent-child").await?;
    until(|| ui.agents.borrow().is_none()).await?;
    if ui
        .state
        .borrow()
        .selected
        .as_ref()
        .and_then(|thread| thread.parent.as_deref())
        != Some("native-smoke-thread")
    {
        return Err("Selected agent lost its parent metadata".into());
    }
    ui.composer.buffer().set_text("Native child draft 中文😀");
    ui.catalog(false);
    until(|| !ui.state.borrow().catalog_loading).await?;
    if !ui
        .state
        .borrow()
        .threads
        .iter()
        .any(|thread| thread.id == "native-agent-child")
    {
        return Err("Catalog refresh removed the selected subagent".into());
    }
    let child = opened(ui).await?;
    if child.smoke_ids() != expected {
        return Err("Opening Agents from a child lost its family".into());
    }
    capture(&child.dialog, "agents-child.png");
    child.smoke_open("native-agent-grandchild")?;
    loaded(ui, "native-agent-grandchild").await?;
    until(|| ui.agents.borrow().is_none()).await?;
    ui.composer
        .buffer()
        .set_text("Native grandchild draft 中文😀");
    let grandchild = opened(ui).await?;
    grandchild.smoke_open(&root.id)?;
    loaded(ui, &root.id).await?;
    until(|| ui.agents.borrow().is_none()).await?;
    if ui.composer_text() != "Native root draft 中文😀" {
        return Err("Agent navigation discarded the root draft".into());
    }
    let panel = opened(ui).await?;
    panel.smoke_open("native-agent-child")?;
    loaded(ui, "native-agent-child").await?;
    until(|| ui.agents.borrow().is_none()).await?;
    if ui.composer_text() != "Native child draft 中文😀" {
        return Err("Agent navigation discarded the child draft".into());
    }
    let child = opened(ui).await?;
    child.smoke_open(&root.id)?;
    loaded(ui, &root.id).await?;
    until(|| ui.agents.borrow().is_none()).await?;

    let panel = opened(ui).await?;
    let cycle = directory.join("native-agent-cursor-cycle.flag");
    std::fs::write(&cycle, b"cycle").map_err(|e| e.to_string())?;
    panel.begin();
    until(|| {
        panel
            .smoke_status()
            .contains("repeated an agent catalog cursor")
    })
    .await?;
    if !panel.smoke_ids().is_empty() {
        return Err("Cyclic pagination left a misleading agent tree".into());
    }
    std::fs::remove_file(cycle).map_err(|e| e.to_string())?;
    let cycle = directory.join("native-agent-lineage-cycle.flag");
    std::fs::write(&cycle, b"cycle").map_err(|e| e.to_string())?;
    panel.begin();
    until(|| panel.smoke_status().contains("ancestry contains a cycle")).await?;
    std::fs::remove_file(cycle).map_err(|e| e.to_string())?;
    let delay = directory.join("native-agent-delay.flag");
    std::fs::write(&delay, b"delay").map_err(|e| e.to_string())?;
    panel.begin();
    until(|| !delay.exists()).await?;
    let old = (panel.id, panel.revision.get());
    panel.dialog.close();
    until(|| ui.agents.borrow().is_none()).await?;
    let replacement = opened(ui).await?;
    glib::timeout_future(Duration::from_millis(450)).await;
    ui.event(Event::AgentTree {
        dialog: old.0,
        revision: old.1,
        result: Ok(agents::Tree {
            selected: "obsolete".into(),
            rows: vec![],
            partial: None,
        }),
    });
    if replacement.smoke_ids() != expected {
        return Err("Old agent replies changed the reopened dialog".into());
    }
    std::fs::write(&delay, b"delay").map_err(|e| e.to_string())?;
    replacement.begin();
    until(|| !delay.exists()).await?;
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("No agent isolation sibling")?;
    ui.select(sibling);
    loaded(ui, "native-cache-thread").await?;
    until(|| {
        replacement
            .smoke_status()
            .contains("session or connection changed")
    })
    .await?;
    replacement.dialog.close();
    until(|| ui.agents.borrow().is_none()).await?;
    ui.select(root.clone());
    loaded(ui, &root.id).await?;
    if ui.composer_text() != "Native root draft 中文😀" {
        return Err("Stale agent discovery discarded the original draft".into());
    }
    ui.composer.buffer().set_text(&draft);
    Ok(
        json!({"ok":true,"rootMembership":true,"nestedNavigation":true,"currentAgentDisabled":true,"rootAndChildDrafts":true,"selectedAgentSurvivesRefresh":true,"cursorCycle":true,"lineageCycle":true,"recreatedDialogIsolation":true,"staleSessionIsolation":true}),
    )
}

#[cfg(debug_assertions)]
pub(super) async fn restart_smoke(ui: &Rc<Ui>, seed: bool) -> Result<Value, String> {
    until(|| ui.state.borrow().ready && !ui.state.borrow().catalog_loading).await?;
    if seed {
        let backend = ui
            .state
            .borrow()
            .backends
            .iter()
            .find(|backend| backend.kind == "codex")
            .cloned()
            .ok_or("No Codex fixture")?;
        ui.switch_backend(backend);
        until(|| {
            ui.state.borrow().ready
                && !ui.state.borrow().catalog_loading
                && ui
                    .state
                    .borrow()
                    .threads
                    .iter()
                    .any(|thread| thread.id == "native-smoke-thread")
        })
        .await?;
        let root = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-smoke-thread")
            .cloned()
            .unwrap();
        ui.select(root);
        loaded(ui, "native-smoke-thread").await?;
        let panel = opened(ui).await?;
        panel.smoke_open("native-agent-child")?;
        loaded(ui, "native-agent-child").await?;
        until(|| ui.agents.borrow().is_none()).await?;
    } else {
        loaded(ui, "native-agent-child").await?;
        ui.catalog(false);
        until(|| !ui.state.borrow().catalog_loading).await?;
    }
    let state = ui.state.borrow();
    let selected = state.selected.as_ref().ok_or("No restored agent")?;
    if selected.id != "native-agent-child"
        || selected.parent.as_deref() != Some("native-smoke-thread")
        || !state.threads.iter().any(|thread| thread.id == selected.id)
    {
        return Err("Restart lost the selected subagent or its parent".into());
    }
    let isolated = state.backend_ready.len() == 1;
    drop(state);
    if !seed && !isolated {
        return Err("Restoring an agent eagerly started another backend".into());
    }
    glib::timeout_future(Duration::from_millis(120)).await;
    Ok(
        json!({"ok":true,"selectedAgent":true,"parentMetadata":true,"catalogRetention":true,"lazyBackendRestore":seed||isolated}),
    )
}
