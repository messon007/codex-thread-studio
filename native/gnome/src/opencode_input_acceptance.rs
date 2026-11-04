//! Project identity and resolved-request races over private HTTP/SSE providers.
use super::*;
use crate::{
    interaction::Input,
    interaction_ui::{Panel, Scope},
};
use std::path::Path;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(format!("OpenCode input timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn file(path: std::path::PathBuf, bytes: Vec<u8>) -> Result<(), String> {
    gio::spawn_blocking(move || std::fs::write(path, bytes).map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Input fixture worker stopped")?
}
async fn emit(root: &Path, values: Value) -> Result<(), String> {
    let old = std::fs::read_to_string(root.join("oc-input-ack"))
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let temp = root.join("oc-input-events.tmp");
    file(temp.clone(), serde_json::to_vec(&values).unwrap()).await?;
    std::fs::rename(temp, root.join("oc-input-events.json")).map_err(|e| e.to_string())?;
    until("provider event acknowledgement", || {
        std::fs::read_to_string(root.join("oc-input-ack"))
            .is_ok_and(|s| s.parse::<u64>().is_ok_and(|v| v > old))
    })
    .await
}
fn event(root: &Path, project: &str, kind: &str, id: &str, session: &str) -> Value {
    let mut props = json!({"sessionID":session,"permission":"bash","patterns":["echo private input"],"always":[]});
    props[if kind.ends_with(".asked") {
        "id"
    } else {
        "requestID"
    }] = id.into();
    json!({"directory":root.join(format!("project-{project}")),"payload":{"type":kind,"properties":props}})
}
fn question(root: &Path, id: &str) -> Value {
    json!({"directory":root.join("project-a"),"payload":{"type":"question.asked","properties":{"id":id,"sessionID":"s-a","questions":[{"header":"Features","question":"Select features and add a custom response","multiple":true,"options":[{"label":"A","description":"First"},{"label":"B","description":"Second"}]},{"header":"Required choice","question":"Use native controls?","custom":false,"options":[{"label":"GTK4","description":"Native GNOME"}]}]}}})
}
fn key(ui: &Ui, backend: &str, id: &str) -> Scope {
    Scope::new(
        backend,
        *ui.state
            .borrow()
            .backend_generations
            .get(backend)
            .unwrap_or(&0),
        &json!(id),
    )
    .unwrap()
}
fn approval(ui: &Ui, backend: &str, id: &str) -> Option<adw::MessageDialog> {
    ui.approvals
        .borrow()
        .get(&key(ui, backend, id))
        .map(|a| a.dialog.clone())
}
fn panel(ui: &Ui, id: &str) -> Option<Rc<Panel>> {
    ui.interactions
        .borrow()
        .get(&key(ui, "opencode", id))
        .cloned()
}
async fn opened_approval(ui: &Ui, backend: &str, id: &str) -> Result<adw::MessageDialog, String> {
    until("approval opens", || {
        approval(ui, backend, id).is_some_and(|d| d.is_mapped())
    })
    .await?;
    Ok(approval(ui, backend, id).unwrap())
}
async fn opened_panel(ui: &Ui, id: &str) -> Result<Rc<Panel>, String> {
    until("question opens", || {
        panel(ui, id).is_some_and(|p| p.dialog.is_mapped())
    })
    .await?;
    Ok(panel(ui, id).unwrap())
}
fn click(dialog: &adw::MessageDialog, name: &str) -> Result<(), String> {
    fn find(w: &gtk::Widget, name: &str) -> Option<gtk::Button> {
        if let Some(b) = w
            .downcast_ref::<gtk::Button>()
            .filter(|b| b.label().as_deref() == Some(name))
        {
            return Some(b.clone());
        }
        let mut c = w.first_child();
        while let Some(w) = c {
            c = w.next_sibling();
            if let Some(b) = find(&w, name) {
                return Some(b);
            }
        }
        None
    }
    find(dialog.upcast_ref(), name)
        .ok_or("Approval action missing")?
        .emit_clicked();
    Ok(())
}
fn posts(root: &Path, id: &str) -> Vec<Value> {
    std::fs::read_to_string(root.join("opencode-requests.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| {
            v["method"] == "POST"
                && v["path"].as_str().is_some_and(|p| {
                    percent_encoding::percent_decode_str(p)
                        .decode_utf8_lossy()
                        .contains(id)
                })
        })
        .collect()
}
async fn posted(root: &Path, id: &str, n: usize) -> Result<Vec<Value>, String> {
    until("provider POST", || posts(root, id).len() >= n).await?;
    Ok(posts(root, id))
}
async fn choose(ui: &Rc<Ui>, id: &str) -> Result<(), String> {
    let thread = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or("No scoped input session")?;
    ui.select(thread);
    until("session load", || {
        !ui.state.borrow().loading && ui.state.borrow().options_loaded
    })
    .await
}
async fn witness(ui: &Ui, root: &Path, value: &str) -> Result<(), String> {
    emit(root,json!([{"directory":root.join("project-b"),"payload":{"type":"session.diff","properties":{"sessionID":"s-b","diff":value}}}])).await?;
    until("SSE witness", || {
        ui.state
            .borrow_mut()
            .command_data
            .get("opencode:s-b")
            .diff
            .is_some_and(|d| {
                d.is_ok_and(|t| {
                    t.document
                        .window(1, None, false)
                        .is_ok_and(|w| w.text == value)
                })
            })
    })
    .await
}
async fn reconnect(ui: &Rc<Ui>) -> Result<(), String> {
    let generation = ui.state.borrow().socket_generation;
    ui.reconnect();
    until("backend reconnect", || {
        ui.state.borrow().socket_generation != generation
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.state.borrow().options_loaded
    })
    .await
}
async fn capture(panel: &Panel, path: &Path) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..30 {
        match super::smoke::capture_mapped_dialog(&panel.dialog, path) {
            Ok(()) => return Ok(()),
            Err(e) => last = e,
        }
        glib::timeout_future(Duration::from_millis(40)).await;
    }
    Err(last)
}
fn wire(ui: &Rc<Ui>, backend: &str, generation: u64, mut payload: Value) {
    let prepared = protocol::prepare_presentation(&mut payload);
    ui.event(Event::Wire {
        backend: backend.into(),
        generation,
        payload,
        prepared,
    });
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    let mut pulse = super::conversation_pages_acceptance::Probe::new();
    until("initial project", || {
        ui.state.borrow().backend == "opencode"
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.state.borrow().options_loaded
    })
    .await?;
    ui.composer.buffer().set_text("Draft A 中文😀 ");
    let recovered = opened_approval(ui, "opencode", "permission:p-initial").await?;
    click(&recovered, "Allow once")?;
    posted(root, "p-initial", 1).await?;
    let origin = "p/origin 中文";
    emit(
        root,
        json!([event(root, "a", "permission.asked", origin, "s-a")]),
    )
    .await?;
    let dialog = opened_approval(ui, "opencode", &format!("permission:{origin}")).await?;
    choose(ui, "s-b").await?;
    ui.composer.buffer().set_text("Draft B retained");
    click(&dialog, "Allow for session")?;
    if posted(root, origin, 1).await?[0]["body"] != json!({"reply":"always"}) {
        return Err("Session permission changed its response".into());
    }
    emit(root, json!([question(root, "q-multiple")])).await?;
    let p = opened_panel(ui, "question:q-multiple").await?;
    p.smoke_set(
        0,
        Input::Selected {
            choices: vec![0, 1],
            other: "  extra 中文😀  ".into(),
        },
    )?;
    p.smoke_set(1, Input::Choices(vec![0]))?;
    if p.smoke_set(1, Input::Text("forbidden custom".into()))
        .is_ok()
    {
        return Err("Non-custom question exposed an entry".into());
    }
    capture(&p, &root.join("opencode-question.png")).await?;
    p.smoke_click("Submit")?;
    if posted(root, "q-multiple", 1).await?[0]["body"]
        != json!({"answers":[["A","B","  extra 中文😀  "],["GTK4"]]})
    {
        return Err("OpenCode choices/custom text changed".into());
    }
    if ui.state.borrow().selected.as_ref().map(|t| t.id.as_str()) != Some("s-b")
        || ui.composer_text() != "Draft B retained"
    {
        return Err("Offscreen input changed the displayed conversation or draft".into());
    }
    emit(root, json!([question(root, "q-reject")])).await?;
    opened_panel(ui, "question:q-reject")
        .await?
        .smoke_click("Cancel")?;
    posted(root, "q-reject", 1).await?;
    emit(
        root,
        json!([event(root, "a", "permission.asked", "p-external", "s-a")]),
    )
    .await?;
    let old = opened_approval(ui, "opencode", "permission:p-external").await?;
    emit(
        root,
        json!([event(root, "b", "permission.replied", "p-external", "s-a")]),
    )
    .await?;
    witness(ui, root, "wrong project processed").await?;
    if approval(ui, "opencode", "permission:p-external").is_none() {
        return Err("Foreign project resolved an approval".into());
    }
    emit(
        root,
        json!([event(root, "a", "permission.replied", "p-external", "s-a")]),
    )
    .await?;
    until("external permission resolves", || {
        approval(ui, "opencode", "permission:p-external").is_none()
    })
    .await?;
    old.response("accept");
    emit(root, json!([question(root, "q-external")])).await?;
    let old = opened_panel(ui, "question:q-external").await?;
    old.smoke_set(0, Input::Choices(vec![0]))?;
    old.smoke_set(1, Input::Choices(vec![0]))?;
    emit(
        root,
        json!([event(root, "a", "question.rejected", "q-external", "s-a")]),
    )
    .await?;
    until("external question resolves", || {
        panel(ui, "question:q-external").is_none()
    })
    .await?;
    old.smoke_click("Submit")?;
    // Hold a stale inventory while an external client resolves its request.
    // A second recovered question fences completion of that exact snapshot.
    file(root.join("hold-snapshot"), b"hold".to_vec()).await?;
    let mut silent = event(root, "a", "permission.asked", "p-race", "s-a");
    silent["emit"] = false.into();
    emit(root, json!([silent])).await?;
    let start = std::time::Instant::now();
    choose(ui, "s-a").await?;
    let load_ms = start.elapsed().as_millis();
    until("snapshot held", || root.join("snapshot-waiting").exists()).await?;
    if ui.state.borrow().loading {
        return Err("Pending inputs blocked history loading".into());
    }
    let mut fence = question(root, "q-fence");
    fence["emit"] = false.into();
    emit(
        root,
        json!([
            event(root, "a", "permission.replied", "p-race", "s-a"),
            fence
        ]),
    )
    .await?;
    witness(ui, root, "resolved before snapshot returned").await?;
    std::fs::remove_file(root.join("hold-snapshot")).map_err(|e| e.to_string())?;
    let fence = opened_panel(ui, "question:q-fence").await?;
    if approval(ui, "opencode", "permission:p-race").is_some() {
        return Err("A stale inventory resurrected a resolved request".into());
    }
    fence.smoke_click("Cancel")?;
    posted(root, "q-fence", 1).await?;
    ui.composer.buffer().set_text("Draft A before reconnect");
    emit(
        root,
        json!([event(root, "a", "permission.asked", "p-lost", "s-a")]),
    )
    .await?;
    let p = opened_approval(ui, "opencode", "permission:p-lost").await?;
    file(root.join("lose-ack"), b"p-lost".to_vec()).await?;
    click(&p, "Allow once")?;
    until("lost acknowledgement disconnect", || {
        !ui.state.borrow().backend_ready.contains("opencode")
    })
    .await?;
    reconnect(ui).await?;
    emit(
        root,
        json!([event(root, "a", "permission.asked", "p-uncertain", "s-a")]),
    )
    .await?;
    let old = opened_approval(ui, "opencode", "permission:p-uncertain").await?;
    file(root.join("fail-before-accept"), b"p-uncertain".to_vec()).await?;
    click(&old, "Allow once")?;
    until("unaccepted disconnect", || {
        !ui.state.borrow().backend_ready.contains("opencode")
    })
    .await?;
    reconnect(ui).await?;
    let recovered = opened_approval(ui, "opencode", "permission:p-uncertain").await?;
    if posts(root, "p-uncertain").len() != 1 {
        return Err("An uncertain response replayed without user action".into());
    }
    click(&recovered, "Decline")?;
    old.response("accept");
    posted(root, "p-uncertain", 2).await?;
    if ui.composer_text() != "Draft A before reconnect" {
        return Err("Request recovery lost the conversation draft".into());
    }
    if !posts(root, "p-external").is_empty()
        || !posts(root, "q-external").is_empty()
        || !posts(root, "p-race").is_empty()
    {
        return Err("An externally resolved dialog sent a stale response".into());
    }
    emit(
        root,
        json!([event(root, "a", "permission.asked", "p-backend", "s-a")]),
    )
    .await?;
    let pending = opened_approval(ui, "opencode", "permission:p-backend").await?;
    let codex = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "codex")
        .cloned()
        .ok_or("No second backend")?;
    let thread = Thread::from_value(
        &json!({"id":"target","name":"Codex target","cwd":root.join("project")}),
    )
    .ok_or("No Codex target metadata")?;
    ui.connect_backend_to(codex, false, Some(thread));
    until("other backend is selected", || {
        ui.state.borrow().backend == "codex"
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.state.borrow().options_loaded
    })
    .await?;
    ui.composer.buffer().set_text("Codex draft retained");
    click(&pending, "Allow once")?;
    posted(root, "p-backend", 1).await?;
    let generation = ui.state.borrow().socket_generation;
    let request = |id: &str| json!({"id":id,"method":"item/commandExecution/requestApproval","params":{"threadId":"target","command":"private legacy scope test"}});
    wire(ui, "codex", generation, request("permission:p-cross"));
    let old = opened_approval(ui, "codex", "permission:p-cross").await?;
    emit(
        root,
        json!([event(root, "a", "permission.replied", "p-cross", "s-a")]),
    )
    .await?;
    witness(ui, root, "foreign backend resolution processed").await?;
    if approval(ui, "codex", "permission:p-cross").is_none() {
        return Err("Another backend resolved a Codex approval with the same ID".into());
    }
    wire(
        ui,
        "codex",
        generation,
        json!({"method":"serverRequest/resolved","params":{"threadId":"target","requestId":"permission:p-cross"}}),
    );
    old.response("accept");
    if approval(ui, "codex", "permission:p-cross").is_some() {
        return Err("Codex resolution retained its approval dialog".into());
    }
    wire(ui, "codex", generation, request("legacy-status"));
    let old = opened_approval(ui, "codex", "legacy-status").await?;
    wire(
        ui,
        "codex",
        generation,
        json!({"method":"studio/appServer/status","params":{"state":"stopped"}}),
    );
    old.response("accept");
    if approval(ui, "codex", "legacy-status").is_some() {
        return Err("Lost backend readiness retained a legacy approval".into());
    }
    wire(
        ui,
        "codex",
        generation,
        json!({"method":"studio/appServer/status","params":{"state":"ready"}}),
    );
    wire(ui, "codex", generation, request("legacy-reuse"));
    let old = opened_approval(ui, "codex", "legacy-reuse").await?;
    reconnect(ui).await?;
    old.response("accept");
    wire(ui, "codex", generation, request("legacy-reuse"));
    if approval(ui, "codex", "legacy-reuse").is_some() {
        return Err("An old generation reopened an approval".into());
    }
    let current = ui.state.borrow().socket_generation;
    wire(ui, "codex", current, request("legacy-reuse"));
    click(
        &opened_approval(ui, "codex", "legacy-reuse").await?,
        "Decline",
    )?;
    until("scoped Codex response", || {
        std::fs::read_to_string(root.join("requests.jsonl")).is_ok_and(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .any(|v| v["id"] == "legacy-reuse" && v.get("method").is_none())
        })
    })
    .await?;
    if ui.composer_text() != "Codex draft retained" {
        return Err("Input lifecycle changed the other backend's draft".into());
    }
    Ok(
        json!({"ok":true,"initialRecovery":true,"projectSwitchOrigin":true,"backendSwitchOrigin":true,"multipleAndCustom":true,"explicitReject":true,"foreignResolutionGuard":true,"externalResolutionNoReply":true,"legacyGenerationAndResolution":true,"staleSnapshot":true,"lostAckNoReplay":true,"unacceptedManualRecovery":true,"draftRetention":true,"historyMappedWhileRecoveryHeldMs":load_ms,"workflowHeartbeatIncludingCapture":pulse.stop()}),
    )
}
