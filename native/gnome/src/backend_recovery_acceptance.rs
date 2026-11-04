//! Real private WebSocket/SSE disconnects through an owned loopback proxy.
use super::*;
use std::path::Path;
struct Pulse {
    source: Option<glib::SourceId>,
    ticks: Rc<std::cell::Cell<u64>>,
    gap: Rc<std::cell::Cell<u128>>,
    start: std::time::Instant,
}
impl Pulse {
    fn start() -> Self {
        let (ticks, gap) = (
            Rc::new(std::cell::Cell::new(0)),
            Rc::new(std::cell::Cell::new(0)),
        );
        let (t, g) = (ticks.clone(), gap.clone());
        let start = std::time::Instant::now();
        let mut last = start;
        let source = glib::timeout_add_local(Duration::from_millis(10), move || {
            let now = std::time::Instant::now();
            g.set(g.get().max(now.duration_since(last).as_millis()));
            t.set(t.get() + 1);
            last = now;
            glib::ControlFlow::Continue
        });
        Self {
            source: Some(source),
            ticks,
            gap,
            start,
        }
    }
}
impl Drop for Pulse {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
    }
}
async fn until(ui: &Ui, label: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let started = std::time::Instant::now();
    while !test() {
        if started.elapsed() > Duration::from_secs(20) {
            return Err(format!(
                "Backend recovery timed out: {label}; status={}",
                ui.status.text()
            ));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn flag(root: &Path, name: &str, present: bool) -> Result<(), String> {
    let path = root.join(name);
    gio::spawn_blocking(move || {
        if present {
            std::fs::write(path, b"1").map_err(|e| e.to_string())
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        }
    })
    .await
    .map_err(|_| "Recovery fixture writer stopped")?
}
async fn file(root: &Path, name: &str) -> Result<String, String> {
    let path = root.join(name);
    gio::spawn_blocking(move || std::fs::read_to_string(path).map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Recovery fixture reader stopped")?
}
fn ready(ui: &Ui, backend: &str, thread: &str) -> bool {
    let s = ui.state.borrow();
    s.backend == backend
        && s.ready
        && !s.loading
        && s.history_verified
        && s.options_loaded
        && s.selected.as_ref().is_some_and(|t| t.id == thread)
}
fn visible_text(widget: &gtk::Widget, text: &str, exclude: &gtk::Widget) -> bool {
    if widget == exclude {
        return false;
    }
    if widget.is_mapped()
        && widget
            .clone()
            .downcast::<gtk::Label>()
            .is_ok_and(|label| label.text() == text)
    {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if visible_text(&widget, text, exclude) {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}
async fn drop_transport(ui: &Ui, root: &Path, backend: &str) -> Result<u64, String> {
    let generation = ui.state.borrow().backend_generations[backend];
    flag(root, &format!("drop-{backend}"), true).await?;
    until(ui, "real transport closed", || {
        !ui.state.borrow().backend_ready.contains(backend)
    })
    .await?;
    flag(root, &format!("drop-{backend}"), false).await?;
    Ok(generation)
}
async fn wait_file(
    root: &Path,
    name: &str,
    predicate: impl Fn(&str) -> bool,
) -> Result<(), String> {
    let at = std::time::Instant::now();
    loop {
        if file(root, name).await.is_ok_and(|text| predicate(&text)) {
            return Ok(());
        }
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Owned recovery fixture did not publish {name}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
async fn fail_backend(
    ui: &Ui,
    root: &Path,
    backend: &str,
    process: bool,
    gap: bool,
) -> Result<u64, String> {
    if gap {
        let pulse = Pulse::start();
        let generation = ui.state.borrow().backend_generations[backend];
        let before = file(root, "gap-published.json")
            .await
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|v| v["sequence"].as_u64())
            .unwrap_or(0);
        flag(root, "hold-codex", true).await?;
        wait_file(root, "hold-codex-active", |_| true).await?;
        flag(root, "emit-event-gap", true).await?;
        let published = wait_file(root, "gap-published.json", |text| {
            serde_json::from_str::<Value>(text)
                .ok()
                .and_then(|v| v["sequence"].as_u64())
                .is_some_and(|n| n > before)
        })
        .await;
        flag(root, "hold-codex", false).await?;
        published?;
        until(ui, "real gateway event gap invalidated state", || {
            !ui.state.borrow().backend_ready.contains(backend)
        })
        .await?;
        let measured = json!({"elapsedMs":pulse.start.elapsed().as_millis(),"mainLoopTicks":pulse.ticks.get(),"maximumMainLoopGapMs":pulse.gap.get()});
        let path = root.join(format!("gap-responsiveness-{}.json", before + 1));
        gio::spawn_blocking(move || {
            std::fs::write(path, measured.to_string()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Gap measurement writer stopped")??;
        return Ok(generation);
    }
    if !process {
        return drop_transport(ui, root, backend).await;
    }
    let generation = ui.state.borrow().backend_generations[backend];
    flag(root, "exit-codex-process", true).await?;
    until(ui, "real Codex process stopped", || {
        !ui.state.borrow().backend_ready.contains(backend)
    })
    .await?;
    Ok(generation)
}
async fn background(
    ui: &Rc<Ui>,
    root: &Path,
    backend: &str,
    process: bool,
    gap: bool,
) -> Result<(), String> {
    let (selected, thread, generation, items) = {
        let s = ui.state.borrow();
        (
            s.backend.clone(),
            s.selected.as_ref().unwrap().id.clone(),
            s.socket_generation,
            s.items.clone(),
        )
    };
    let draft = ui.composer_text();
    let old = fail_backend(ui, root, backend, process, gap).await?;
    until(ui, "offscreen transport recovery", || {
        ui.state.borrow().backend_ready.contains(backend)
            && ui.state.borrow().backend_generations[backend] > old
    })
    .await?;
    if !ready(ui, &selected, &thread)
        || ui.state.borrow().socket_generation != generation
        || ui.composer_text() != draft
        || ui.state.borrow().items.len() != items.len()
        || !ui
            .state
            .borrow()
            .items
            .iter()
            .zip(&items)
            .all(|(a, b)| Arc::ptr_eq(a, b))
    {
        return Err("Background recovery changed the selected transcript/draft/generation".into());
    }
    Ok(())
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    let startup = std::env::var("STUDIO_GNOME_SMOKE_STARTUP_RECOVERY").as_deref() == Ok("1");
    let process = std::env::var("STUDIO_GNOME_SMOKE_PROCESS_RECOVERY").as_deref() == Ok("1");
    let gap = std::env::var("STUDIO_GNOME_SMOKE_EVENT_GAP").as_deref() == Ok("1");
    let startup_draft = "Typed before the first successful connection 中文😀 ";
    if startup {
        until(ui, "two initial WebSocket failures", || {
            ui.recovery.borrow().pending() == 1
                && ui.status.text().ends_with("(2/8)")
                && !ui.state.borrow().ready
        })
        .await?;
        if ui.state.borrow().selected.is_some() || ui.send.is_sensitive() {
            return Err("Initial transport failure enabled an unverified conversation".into());
        }
        ui.composer.buffer().set_text(startup_draft);
        ui.submit();
        crate::window_capture::capture(&ui.window, &root.join("backend-startup-retry.png")).await?;
        flag(root, "reject-codex", false).await?;
    }
    until(ui, "initial selection", || {
        ready(ui, "codex", "target") && ui.router.borrow().registry.is_some()
    })
    .await?;
    if startup && ui.composer_text() != startup_draft {
        return Err("First successful startup connection lost the pending draft".into());
    }
    until(ui, "durable queued-message inventory", || {
        ui.state
            .borrow()
            .queues
            .entries
            .get("codex:target")
            .is_some_and(|e| e.loaded)
    })
    .await?;
    // One explicit fake-provider Send accepts a turn but never returns its ACK.
    // Recovery must retrieve its authoritative history without resending it.
    flag(root, "hold-ack-target", true).await?;
    let prompt = "Uncertain fake prompt 中文😀";
    ui.composer.buffer().set_text(prompt);
    ui.submit();
    let deadline = std::time::Instant::now();
    loop {
        if file(root, "codex-turns.json")
            .await
            .is_ok_and(|s| s.contains("Native routed target response"))
        {
            break;
        }
        if deadline.elapsed() > Duration::from_secs(10) {
            return Err("Fake provider did not accept the single prompt".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    {
        let mut s = ui.state.borrow_mut();
        let e = s.queues.entries.get_mut("codex:target").unwrap();
        e.paused = false;
        e.active = true;
        e.released = false;
    }
    flag(root, "hold-reconnect-catalog", true).await?;
    let old = fail_backend(ui, root, "codex", process, gap).await?;
    let feedback = ui.status.text();
    if !feedback.starts_with("Disconnected · Reconnecting in ") {
        return Err("Reconnect wait has no visible feedback".into());
    }
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if !ui.status.text().starts_with("连接已断开 · ") {
        return Err("Reconnect feedback did not translate live".into());
    }
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    if ui.status.text() != feedback {
        return Err("Reconnect feedback changed its literal attempt/delay".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("backend-reconnecting.png")).await?;
    until(ui, "catalog gap remains editable", || {
        let s = ui.state.borrow();
        s.socket_generation > old && s.ready && s.catalog_loading && s.selected.is_none()
    })
    .await?;
    if ui.composer_text() != prompt || ui.send.is_sensitive() {
        return Err(
            "Reconnecting cleared the draft or enabled Send before selection verification".into(),
        );
    }
    let newer = "New text typed during reconnect 中文😀 ";
    let buffer = ui.composer.buffer();
    buffer.set_text(newer);
    buffer.select_range(&buffer.iter_at_offset(3), &buffer.iter_at_offset(14));
    ui.submit(); // Loading must also block programmatic submission.
    flag(root, "hold-reconnect-catalog", false).await?;

    until(ui, "automatic selected Codex recovery", || {
        ready(ui, "codex", "target") && ui.state.borrow().socket_generation > old
    })
    .await?;
    if ui.composer_text() != newer
        || buffer
            .selection_bounds()
            .map(|(a, b)| (a.offset(), b.offset()))
            != Some((3, 14))
        || !ui
            .state
            .borrow()
            .items
            .iter()
            .any(|i| i.body.contains("Native routed target response"))
        || !ui.state.borrow().queues.entries["codex:target"].paused
    {
        return Err("Recovery lost the uncertain draft/history or resumed queued messages".into());
    }
    if gap
        && !ui
            .state
            .borrow()
            .items
            .iter()
            .any(|i| i.body.contains("Recovered missed events 1"))
    {
        return Err("Selected event-gap recovery did not revalidate authoritative history".into());
    }
    let stale = ui.state.borrow().socket_generation;
    ui.event(Event::Wire{backend:"codex".into(),generation:old,payload:json!({"method":"turn/started","params":{"threadId":"target","turn":{"id":"stale-reconnect"}}}),prepared:Default::default()});
    ui.event(Event::BackendDisconnected {
        backend: "codex".into(),
        generation: old,
        error: "stale transport".into(),
    });
    if ui.state.borrow().socket_generation != stale || !ready(ui, "codex", "target") {
        return Err("Stale transport events changed the recovered session".into());
    }
    let codex = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "codex")
        .cloned()
        .unwrap();
    let opencode = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "opencode")
        .cloned()
        .unwrap();
    if startup {
        flag(root, "reject-opencode", true).await?;
    }
    ui.switch_backend(opencode.clone());
    if startup {
        until(ui, "two initial SSE failures", || {
            ui.recovery.borrow().pending() == 1
                && ui.status.text().ends_with("(2/8)")
                && !ui.state.borrow().ready
        })
        .await?;
        if ui.state.borrow().selected.is_some() || ui.send.is_sensitive() {
            return Err("Initial SSE failure enabled an unverified conversation".into());
        }
        ui.composer.buffer().set_text(startup_draft);
        ui.submit();
        flag(root, "reject-opencode", false).await?;
    }
    until(ui, "OpenCode catalog", || {
        ui.state
            .borrow()
            .threads
            .iter()
            .any(|t| t.id == "oc-target")
    })
    .await?;
    if startup && ui.composer_text() != startup_draft {
        return Err("First SSE connection cleared text with no selected conversation".into());
    }
    let thread = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "oc-target")
        .cloned()
        .unwrap();
    ui.select(thread);
    until(ui, "OpenCode selection", || {
        ready(ui, "opencode", "oc-target")
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent OpenCode draft 中文😀");
    let old = drop_transport(ui, root, "opencode").await?;
    until(ui, "automatic selected SSE recovery", || {
        ready(ui, "opencode", "oc-target") && ui.state.borrow().socket_generation > old
    })
    .await?;
    if ui.composer_text() != "Unsent OpenCode draft 中文😀" {
        return Err("SSE recovery lost the selected draft".into());
    }
    background(ui, root, "codex", process, gap).await?;
    ui.switch_backend(codex);
    until(ui, "return to Codex", || ready(ui, "codex", "target")).await?;
    if gap
        && !ui
            .state
            .borrow()
            .items
            .iter()
            .any(|i| i.body.contains("Recovered missed events 2"))
    {
        return Err("Returning to a backend after an offscreen gap reused stale history".into());
    }
    background(ui, root, "opencode", false, false).await?;
    let old = drop_transport(ui, root, "codex").await?;
    until(ui, "scheduled recovery before manual override", || {
        ui.recovery.borrow().pending() > 0
    })
    .await?;
    ui.reconnect();
    until(ui, "manual reconnect", || {
        ready(ui, "codex", "target") && ui.state.borrow().socket_generation > old
    })
    .await?;
    let generation = ui.state.borrow().socket_generation;
    // Two startup failures retain their budget, so the canceled timer in that
    // workflow can be eight seconds rather than the ordinary two seconds.
    glib::timeout_future(Duration::from_millis(if startup { 9000 } else { 2500 })).await;
    if ui.state.borrow().socket_generation != generation {
        return Err("A stale retry timer replaced the manual connection".into());
    }
    if process {
        ui.toast.dismiss_all();
    }
    let diagnostic = "Request <b>queue</b> is full & 中文😀";
    ui.event(Event::BackendError {
        backend: "codex".into(),
        generation,
        error: diagnostic.into(),
    });
    glib::timeout_future(Duration::from_millis(650)).await;
    if !ready(ui, "codex", "target")
        || ui.recovery.borrow().pending() != 0
        || ui.state.borrow().socket_generation != generation
    {
        return Err("A non-transport error recycled a healthy connection".into());
    }
    if process {
        // Inspect the rendered toast label rather than the already-literal
        // status label. These diagnostics must never be parsed as Pango markup.
        until(ui, "literal diagnostic toast", || {
            visible_text(ui.toast.upcast_ref(), diagnostic, ui.status.upcast_ref())
        })
        .await?;
        crate::window_capture::capture(&ui.window, &root.join("literal-backend-diagnostic.png"))
            .await?;
    }
    // Leave a real background retry pending; accepted window close must cancel
    // it. The proxy records that no transport opened after this close marker.
    flag(root, "drop-opencode", true).await?;
    until(ui, "pending retry on close", || {
        ui.recovery.borrow().pending() > 0
    })
    .await?;
    flag(root, "closing-recovery-fixture", true).await?;
    let mut result = json!({"ok":true,"selectedCodex":true,"selectedOpenCode":true,"backgroundOpenCode":true,"backgroundCodex":true,"uncertainSendRetained":true,"typingDuringRecovery":true,"draftSelectionPreserved":true,"historyRevalidated":true,"queuePaused":true,"staleGenerationRejected":true,"manualTimerOverride":true,"nonTransportErrorIsolated":true,"closeWithPendingRetry":true,"liveFeedbackLanguage":true});
    if startup {
        result["initialCodex"] = true.into();
        result["initialOpenCode"] = true.into();
        result["startupDraftRetained"] = true.into();
    }
    if process {
        result["selectedProcessRecovery"] = true.into();
        result["backgroundProcessRecovery"] = true.into();
        result["literalDiagnostic"] = true.into();
    }
    if gap {
        result["selectedGapRecovery"] = true.into();
        result["backgroundGapRecovery"] = true.into();
    }
    Ok(result)
}
