//! Real protocol/GTK acceptance; only private fake providers may run this.
use super::*;
use std::{cell::Cell, path::Path};

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(format!("Hydration acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn marker(root: &Path, name: &str, text: Option<&str>) -> Result<(), String> {
    let path = root.join(name);
    let text = text.map(str::to_owned);
    gio::spawn_blocking(move || {
        match text {
            Some(text) => std::fs::write(path, text),
            None => std::fs::remove_file(path),
        }
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Fixture worker stopped")?
}
fn body(ui: &Ui, id: &str) -> Option<String> {
    ui.state
        .borrow()
        .items
        .iter()
        .find(|i| i.id == id)
        .map(|i| i.body.clone())
}
async fn ready(ui: &Ui) -> Result<(), String> {
    until("verified history", || {
        let s = ui.state.borrow();
        s.history_verified && !s.loading && s.options_loaded
    })
    .await
}
fn thread(ui: &Ui, id: &str) -> Result<Thread, String> {
    ui.state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or_else(|| format!("Missing fixture session {id}"))
}

pub async fn run(ui: &Rc<Ui>, root: &Path, capture: impl Fn(&str)) -> Result<Value, String> {
    let ticks = Rc::new(Cell::new(0u64));
    let gap = Rc::new(Cell::new(0u64));
    let previous = Cell::new(std::time::Instant::now());
    let t = ticks.clone();
    let g = gap.clone();
    let timer = glib::timeout_add_local(Duration::from_millis(10), move || {
        let now = std::time::Instant::now();
        g.set(
            g.get()
                .max(now.duration_since(previous.replace(now)).as_millis() as u64),
        );
        t.set(t.get() + 1);
        glib::ControlFlow::Continue
    });
    struct Timer(Option<glib::SourceId>);
    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(t) = self.0.take() {
                t.remove();
            }
        }
    }
    let mut timer = Timer(Some(timer));
    let opening = std::time::Instant::now();
    until("stream during initial load", || {
        body(ui, "agent-initial").as_deref() == Some("First 中文 ")
    })
    .await?;
    if !ui.state.borrow().loading || ui.send.is_sensitive() || ui.command_context().is_some() {
        return Err("Unverified session allowed a mutation".into());
    }
    ui.composer
        .buffer()
        .set_text("Unsent hydration draft 中文😀 ");
    marker(root, "hydration-hold-target", None).await?;
    ready(ui).await?;
    if ui.state.borrow().items.len() != crate::conversation_pages::PAGE_ITEMS + 1
        || body(ui, "agent-initial").as_deref() != Some("First 中文 ")
        || ui.state.borrow().active_turn.as_deref() != Some("turn-initial")
    {
        return Err("Initial page lost its live partial or active turn".into());
    }
    until("mapped initial body", || {
        ui.chat_visibility
            .visible_ends(ui.transcript_scroll.upcast_ref())
            .contains(&("agent-initial".to_owned(), "turn-initial".to_owned()))
    })
    .await?;
    let first_open_ms = opening.elapsed().as_millis();
    let heartbeat = json!({"ticks":ticks.get(),"maximumGapMs":gap.get()});
    timer.0.take().unwrap().remove();
    capture("hydration-initial.png");
    marker(root, "hydration-finish", Some("")).await?;
    until("initial complete text", || {
        body(ui, "agent-initial").as_deref() == Some("First 中文 second 😀")
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    // A read can finish while another session is selected. Streamed partials
    // must update its bounded cache, not leak into the selected transcript.
    marker(root, "hydration-hold-target", Some("")).await?;
    marker(root, "hydration-inject", Some("offscreen")).await?;
    ui.read_session("target", false);
    until("offscreen partial began", || {
        body(ui, "agent-offscreen").is_some()
    })
    .await?;
    ui.select(thread(ui, "child")?);
    ready(ui).await?;
    marker(root, "hydration-hold-target", None).await?;
    until("offscreen load committed", || {
        ui.state
            .borrow()
            .cache
            .0
            .peek("codex:target")
            .is_some_and(|v| {
                v.history_verified
                    && v.items
                        .iter()
                        .any(|i| i.id == "agent-offscreen" && i.body == "First 中文 ")
            })
    })
    .await?;
    if body(ui, "agent-offscreen").is_some() {
        return Err("Offscreen text leaked into another session".into());
    }
    ui.select(thread(ui, "target")?);
    ready(ui).await?;
    if body(ui, "agent-offscreen").as_deref() != Some("First 中文 ") {
        return Err("Cached partial was erased by a new persisted page".into());
    }
    let codex = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "codex")
        .cloned()
        .ok_or("Missing Codex backend")?;
    let opencode = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.kind == "opencode")
        .cloned()
        .ok_or("Missing OpenCode backend")?;
    let socket = ui.state.borrow().socket_generation;
    ui.switch_backend(opencode);
    until("OpenCode catalog", || {
        ui.state.borrow().ready
            && ui
                .state
                .borrow()
                .threads
                .iter()
                .any(|t| t.id == "oc-target")
    })
    .await?;
    ui.select(thread(ui, "oc-target")?);
    ready(ui).await?;
    ui.switch_backend(codex);
    ready(ui).await?;
    if ui.state.borrow().socket_generation != socket
        || body(ui, "agent-offscreen").as_deref() != Some("First 中文 ")
    {
        return Err("Reusing a subscribed backend discarded its observed partial reply".into());
    }
    marker(root, "hydration-finish", Some("")).await?;
    until("offscreen completed", || {
        ui.state.borrow().active_turn.is_none()
    })
    .await?;
    // The turn completes after the snapshot was taken but before its response.
    marker(root, "hydration-inject", Some("stale")).await?;
    marker(root, "hydration-stale-state", Some("")).await?;
    ui.read_session("target", false);
    ready(ui).await?;
    if ui.state.borrow().active_turn.is_some()
        || body(ui, "agent-stale").as_deref() != Some("First 中文 second 😀")
    {
        return Err("A stale busy snapshot overwrote terminal events".into());
    }
    if ui.composer_text() != "Unsent hydration draft 中文😀 " {
        return Err("Hydration erased an unsent draft".into());
    }
    // Malformed data fails closed and never falls back to a different snapshot.
    marker(root, "hydration-invalid", Some("")).await?;
    ui.read_session("target", false);
    until("invalid history rejected", || !ui.state.borrow().loading).await?;
    if ui.state.borrow().history_verified
        || ui.send.is_sensitive()
        || ui.command_context().is_some()
        || Ui::history_ready(&ui.state.borrow(), "codex:target")
    {
        return Err("Malformed history enabled conversation mutations".into());
    }
    ui.submit();
    marker(root, "hydration-invalid", None).await?;
    marker(root, "hydration-invalid-state", Some("")).await?;
    ui.read_session("target", false);
    until("invalid lifecycle rejected", || !ui.state.borrow().loading).await?;
    if ui.state.borrow().history_verified || ui.send.is_sensitive() || ui.enqueue().is_ok() {
        return Err("An invalid lifecycle enabled Send or Queue".into());
    }
    marker(root, "hydration-invalid-state", None).await?;
    ui.read_session("target", false);
    ready(ui).await?;
    if ui.composer_text() != "Unsent hydration draft 中文😀 " {
        return Err("Failed verification changed the draft".into());
    }
    marker(root, "hydration-waiting", None).await?;
    marker(root, "hydration-hold-target", Some("")).await?;
    ui.read_session("target", false);
    until("first superseded read reached provider", || {
        root.join("hydration-waiting").exists()
    })
    .await?;
    ui.read_session("target", false);
    let newest = ui.state.borrow().history_generation;
    marker(root, "hydration-hold-target", None).await?;
    ready(ui).await?;
    glib::timeout_future(Duration::from_millis(180)).await;
    if ui.state.borrow().history_generation != newest
        || !ui.state.borrow().history_verified
        || ui.state.borrow().items.len() != crate::conversation_pages::PAGE_ITEMS
    {
        return Err("A late superseded load changed the latest view".into());
    }
    // Capability fallback uses the turn cursor, never an item cursor.
    marker(root, "hydration-no-items", Some("")).await?;
    ui.read_session("target", true);
    ready(ui).await?;
    if ui.state.borrow().items.len() != crate::conversation_pages::PAGE_ITEMS {
        return Err("Turn compatibility read was not bounded".into());
    }
    marker(root, "hydration-no-turns", Some("")).await?;
    ui.read_session("target", false);
    ready(ui).await?;
    if ui.state.borrow().items.len() != crate::conversation_pages::PAGE_ITEMS {
        return Err("Legacy private read was not bounded".into());
    }
    marker(root, "hydration-no-items", None).await?;
    marker(root, "hydration-no-turns", None).await?;
    marker(root, "hydration-old-resume", Some("")).await?;
    ui.read_session("target", true);
    ready(ui).await?;
    if ui.state.borrow().items.len() != crate::model::MAX_VISIBLE_ITEMS {
        return Err("Old resume compatibility failed".into());
    }
    marker(root, "hydration-old-resume", None).await?;
    // The selected cache belongs to the old socket. It must not append deltas
    // across a reconnect gap; persisted final history replaces it safely.
    let backend = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "codex")
        .cloned()
        .ok_or("Missing backend")?;
    let old = ui.state.borrow().socket_generation;
    ui.reconnect();
    until("reconnected metadata", || {
        ui.state.borrow().socket_generation != old && ui.state.borrow().ready
    })
    .await?;
    ready(ui).await?;
    if ui.state.borrow().backend != backend.id
        || ui.state.borrow().items.len() != crate::conversation_pages::PAGE_ITEMS
        || ui.composer_text() != "Unsent hydration draft 中文😀 "
    {
        return Err("Reconnect failed to restore verified bounded history and draft".into());
    }
    capture("hydration-final.png");
    Ok(
        json!({"ok":true,"boundedInitialItems":crate::conversation_pages::PAGE_ITEMS,"liveSnapshotMerge":true,"activeTurnVerified":true,"offscreenPartialRetention":true,"backendReuseKeepsPartial":true,"terminalEventWins":true,"invalidHistoryBlocksMutations":true,"invalidLifecycleBlocksQueue":true,"supersededReadIsolation":true,"draftRetention":true,"turnCursorFallback":true,"legacyReadFallback":true,"oldResumeRetry":true,"reconnectVerification":true,"firstMappedBodyMs":first_open_ms,"initialHeartbeat":heartbeat}),
    )
}
