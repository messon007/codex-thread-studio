//! Real protocol worker -> native widgets -> private provider replies.
use super::*;
use crate::{
    interaction::Input,
    interaction_ui::{Panel, Scope},
};
use std::path::Path;
#[path = "approval_acceptance.rs"]
mod approvals;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(12) {
            return Err(format!("Input acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn emit(root: &Path, values: Value) -> Result<(), String> {
    let root = root.to_path_buf();
    gio::spawn_blocking(move || {
        let temp = root.join("input-events.tmp");
        let path = root.join("input-events.json");
        std::fs::write(&temp, serde_json::to_vec(&values).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Input fixture worker stopped")?
}
fn panel(ui: &Ui, id: &Value) -> Option<Rc<Panel>> {
    let state = ui.state.borrow();
    let key = Scope::new(&state.backend, state.socket_generation, id)?;
    ui.interactions.borrow().get(&key).cloned()
}
async fn opened(ui: &Ui, id: &Value) -> Result<Rc<Panel>, String> {
    until("open form", || {
        panel(ui, id).is_some_and(|p| {
            p.dialog.is_mapped()
                && p.dialog
                    .child()
                    .is_some_and(|w| w.is_mapped() && w.width() > 0 && w.height() > 0)
        })
    })
    .await?;
    Ok(panel(ui, id).unwrap())
}
async fn capture(panel: &Panel, path: &Path) -> Result<(), String> {
    // Mapping/allocation can precede the first real paint on Wayland. Observe
    // completed GTK renders; never manufacture allocations or frame callbacks.
    let mut last = String::new();
    for _ in 0..30 {
        match super::smoke::capture_mapped_dialog(&panel.dialog, path) {
            Ok(()) => return Ok(()),
            Err(error) => last = error,
        }
        glib::timeout_future(Duration::from_millis(40)).await;
    }
    Err(last)
}
fn question(id: Value) -> Value {
    json!({"id":id,"method":"item/tool/requestUserInput","params":{"threadId":"target","turnId":"input-turn","itemId":"input-item","isBlocking":true,"questions":[
    {"id":"choice","header":"Native interface","question":"Choose the desktop interface","options":[{"label":"GTK4","description":"Native GNOME controls"},{"label":"Other","description":"Alternative"}]},
    {"id":"custom","header":"Details","question":"Add a custom response","isOther":true,"options":[{"label":"Default answer","description":"Use the existing setting"}]},
    {"id":"secret","header":"Private response","question":"Enter the test-only secret","isSecret":true}]}})
}
fn form(id: Value) -> Value {
    json!({"id":id,"method":"mcpServer/elicitation/request","params":{"serverName":"Private test MCP server","threadId":"target","mode":"form","message":"Review these values before submitting.","requestedSchema":{"type":"object","required":["count","date","flavor","tags","title"],"properties":{
    "count":{"title":"Count","type":"integer","minimum":1,"maximum":5,"default":3.0},
    "date":{"title":"Date","type":"string","format":"date"},
    "enabled":{"title":"Enable feature","type":"boolean","default":true},
    "flavor":{"title":"Color","type":"string","oneOf":[{"const":"red-value","title":"Red"},{"const":"blue-value","title":"Blue"}],"default":"red-value"},
    "optional":{"title":"Optional note","type":"string"},
    "ratio":{"title":"Ratio","type":"number","minimum":0.25,"maximum":1,"default":0.5},
    "tags":{"title":"Categories","type":"array","minItems":1,"maxItems":2,"items":{"anyOf":[{"const":"a","title":"Alpha"},{"const":"b","title":"Beta"}]},"default":["b"]},
    "title":{"title":"Title","type":"string","minLength":2,"maxLength":8,"default":"中文"}}}}})
}
fn replies(root: &Path, id: &Value) -> Vec<Value> {
    std::fs::read_to_string(root.join("requests.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["id"] == *id && v.get("method").is_none())
        .collect()
}
fn approval_click(dialog: &adw::MessageDialog, text: &str) -> Result<(), String> {
    fn find(widget: &gtk::Widget, text: &str) -> Option<gtk::Button> {
        if let Some(button) = widget
            .downcast_ref::<gtk::Button>()
            .filter(|b| b.label().as_deref() == Some(text))
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            if let Some(button) = find(&w, text) {
                return Some(button);
            }
        }
        None
    }
    find(dialog.upcast_ref(), &crate::language::tr(text))
        .ok_or("Approval action button is missing")?
        .emit_clicked();
    Ok(())
}
async fn switch_language(ui: &Rc<Ui>, language: &str, index: u32) -> Result<(), String> {
    let controls = ui.language.bind();
    controls.smoke_select(index);
    controls.smoke_apply();
    until("shared language save while input is open", || {
        !controls.saving() && ui.language.smoke_language().as_deref() == Some(language)
    })
    .await
}
fn localized() -> bool {
    std::env::var("STUDIO_GNOME_SMOKE_INPUT_LANGUAGE")
        .ok()
        .as_deref()
        == Some("zh-CN")
}
async fn answered(root: &Path, id: &Value) -> Result<Value, String> {
    until("reply on provider wire", || !replies(root, id).is_empty()).await?;
    let rows = replies(root, id);
    if rows.len() != 1 {
        return Err("Native input was answered more than once".into());
    }
    Ok(rows[0].clone())
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    let mut pulse = super::conversation_pages_acceptance::Probe::new();
    until("ordinary session", || {
        ui.language.smoke_language().is_some()
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.state.borrow().options_loaded
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| t.id == "target")
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Keep my unsent input draft 中文😀 ");
    let qid = json!("questions");
    let started = std::time::Instant::now();
    emit(root, json!([question(qid.clone()), question(qid.clone())])).await?;
    let p = opened(ui, &qid).await?;
    let mapped_ms = started.elapsed().as_millis();
    if ui.interactions.borrow().len() != 1 {
        return Err("Replayed question created duplicate dialogs".into());
    }
    if !p.smoke_secret(2) {
        return Err("Secret answer uses a visible text entry".into());
    }
    if localized() {
        p.smoke_set(1, Input::Text("  Submit {host} 中文😀  ".into()))?;
        p.smoke_set(2, Input::Text("private-test-value".into()))?;
        let before = p.smoke_inputs();
        if p.dialog.title() != "代理需要你的输入" {
            return Err("Question dialog did not start in Chinese".into());
        }
        switch_language(ui, "en-US", 2).await?;
        if p.dialog.title() != "Agent needs your input" || p.smoke_inputs() != before {
            return Err(
                "Live English switch changed an input value or failed to update its dialog".into(),
            );
        }
        switch_language(ui, "zh-CN", 1).await?;
        if p.dialog.title() != "代理需要你的输入"
            || p.smoke_inputs() != before
            || !replies(root, &qid).is_empty()
        {
            return Err("Live Chinese switch changed/submitted question inputs".into());
        }
        p.smoke_set(1, Input::Choices(vec![0]))?;
        p.smoke_set(2, Input::Text(String::new()))?;
    }
    p.smoke_click("Submit")?;
    until("secret validation", || {
        !p.smoke_busy() && p.smoke_error().is_some()
    })
    .await?;
    if !replies(root, &qid).is_empty() {
        return Err("Incomplete answers reached the provider".into());
    }
    p.smoke_set(0, Input::Choices(vec![0]))?;
    p.smoke_set(1, Input::Text("  custom 中文😀  ".into()))?;
    p.smoke_set(1, Input::Choices(vec![0]))?;
    if p.smoke_inputs()[1] != Input::Choices(vec![0]) {
        return Err(
            "Choosing a normal answer after typing custom text still uses the custom answer".into(),
        );
    }
    p.smoke_choose_other(1)?;
    if p.smoke_inputs()[1] != Input::Text("  custom 中文😀  ".into()) {
        return Err("Switching back to Other lost the exact typed answer".into());
    }
    p.smoke_set(2, Input::Text("private-test-value".into()))?;
    capture(&p, &root.join("input-questions.png")).await?;
    p.smoke_click("Submit")?;
    let reply = answered(root, &qid).await?;
    if reply["result"]["answers"]["choice"]["answers"] != json!(["GTK4"])
        || reply["result"]["answers"]["custom"]["answers"] != json!(["  custom 中文😀  "])
    {
        return Err("Choice or exact custom answer changed on the wire".into());
    }
    until("question dismissal", || panel(ui, &qid).is_none()).await?;
    let option_id = json!("question-option-after-custom");
    emit(root, json!([question(option_id.clone())])).await?;
    let selected = opened(ui, &option_id).await?;
    selected.smoke_set(1, Input::Text("Retain this unused custom text".into()))?;
    selected.smoke_set(1, Input::Choices(vec![0]))?;
    selected.smoke_set(2, Input::Text("private-test-value".into()))?;
    selected.smoke_click("Submit")?;
    if answered(root, &option_id).await?["result"]["answers"]["custom"]["answers"]
        != json!(["Default answer"])
    {
        return Err("Visible normal choice did not match the submitted answer".into());
    }
    let fid = json!(17);
    emit(root, json!([form(fid.clone())])).await?;
    let p = opened(ui, &fid).await?;
    p.smoke_click("Submit")?;
    until("required validation", || {
        !p.smoke_busy() && p.smoke_error().is_some()
    })
    .await?;
    p.smoke_set(0, Input::Text("7".into()))?;
    p.smoke_click("Submit")?;
    until("range validation", || {
        !p.smoke_busy() && p.smoke_error().is_some_and(|e| e.contains("range"))
    })
    .await?;
    p.smoke_set(0, Input::Text("3.0".into()))?;
    p.smoke_set(1, Input::Text("2025-02-29".into()))?;
    p.smoke_click("Submit")?;
    until("date validation", || {
        !p.smoke_busy() && p.smoke_error().is_some_and(|e| e.contains("date"))
    })
    .await?;
    if !replies(root, &fid).is_empty() {
        return Err("Invalid form values reached the provider".into());
    }
    p.smoke_set(1, Input::Text("2024-02-29".into()))?;
    p.smoke_set(2, Input::Boolean(false))?;
    p.smoke_set(3, Input::Choices(vec![1]))?;
    p.smoke_set(5, Input::Text("0.75".into()))?;
    p.smoke_set(6, Input::Choices(vec![0, 1]))?;
    p.smoke_set(7, Input::Text("中文😀".into()))?;
    capture(&p, &root.join("input-mcp-form.png")).await?;
    p.smoke_click("Submit")?;
    let reply = answered(root, &fid).await?;
    if reply["result"]
        != json!({"action":"accept","content":{"count":3,"date":"2024-02-29","enabled":false,"flavor":"blue-value","ratio":0.75,"tags":["a","b"],"title":"中文😀"}})
    {
        return Err(format!("MCP values lost their types: {reply}"));
    }
    for (id, action) in [("decline", "Decline"), ("cancel", "Cancel")] {
        let id = json!(id);
        emit(root, json!([form(id.clone())])).await?;
        opened(ui, &id).await?.smoke_click(action)?;
        let reply = answered(root, &id).await?;
        if reply["result"] != json!({"action":action.to_lowercase()}) {
            return Err("MCP cancellation semantics changed".into());
        }
    }
    let id = json!("question-cancel");
    emit(root, json!([question(id.clone())])).await?;
    opened(ui, &id).await?.dialog.close();
    let reply = answered(root, &id).await?;
    if reply["error"]["code"] != -32001 || reply.get("result").is_some() {
        return Err("Question cancellation was reported as empty successful answers".into());
    }
    let uid = json!("url");
    emit(root,json!([{"id":uid,"method":"mcpServer/elicitation/request","params":{"serverName":"URL test MCP","threadId":"target","mode":"url","url":"https://example.invalid/native-auth?state=test","message":"Continue securely in your browser"}}])).await?;
    let p = opened(ui, &uid).await?;
    if root.join("portal-uris.json").exists() {
        return Err("Elicitation URL opened without a click".into());
    }
    capture(&p, &root.join("input-mcp-url.png")).await?;
    p.smoke_click("Open link")?;
    until("explicit URL launch", || {
        root.join("portal-uris.json").exists()
    })
    .await?;
    p.smoke_click("Completed")?;
    if answered(root, &uid).await?["result"] != json!({"action":"accept"}) {
        return Err("URL response exposed form content".into());
    }
    let held = json!("url-pending");
    std::fs::write(root.join("hold-uri"), b"hold").map_err(|e| e.to_string())?;
    emit(root,json!([{"id":held,"method":"mcpServer/elicitation/request","params":{"serverName":"Pending URL MCP","threadId":"target","mode":"url","url":"https://example.invalid/pending-auth","message":"Pending portal cancellation test"}}])).await?;
    opened(ui, &held).await?.smoke_click("Open link")?;
    until("second URL launch", || {
        std::fs::read_to_string(root.join("portal-uris.json"))
            .is_ok_and(|s| serde_json::from_str::<Vec<String>>(&s).is_ok_and(|v| v.len() == 2))
    })
    .await?;
    emit(root,json!([{"method":"serverRequest/resolved","params":{"threadId":"target","requestId":held}}])).await?;
    until("pending URL expired", || panel(ui, &held).is_none()).await?;
    until("pending URI cancellation", || {
        let trace = std::fs::read_to_string(root.join("portal-trace.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok());
        let closed = std::fs::read_to_string(root.join("input-portal-closed.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok());
        trace
            .and_then(|v| v.into_iter().rev().find(|v| v["event"] == "request"))
            .and_then(|v| v["handle"].as_str().map(str::to_owned))
            .is_some_and(|handle| closed.is_some_and(|v| v.contains(&handle)))
    })
    .await?;
    std::fs::remove_file(root.join("hold-uri")).map_err(|e| e.to_string())?;
    // Numeric and string request IDs have separate identities; malformed or old
    // resolution notifications cannot dismiss a newer unrelated form.
    emit(root, json!([question(json!(7)), question(json!("7"))])).await?;
    let old = opened(ui, &json!(7)).await?;
    let other = opened(ui, &json!("7")).await?;
    emit(root,json!([{"method":"serverRequest/resolved","params":{"threadId":"target"}},{"method":"serverRequest/resolved","params":{"threadId":"target","requestId":7}}])).await?;
    until("resolved request", || panel(ui, &json!(7)).is_none()).await?;
    if panel(ui, &json!("7")).is_none() {
        return Err("Resolution mixed numeric and string IDs".into());
    }
    old.smoke_click("Submit")?;
    other.smoke_click("Cancel")?;
    answered(root, &json!("7")).await?;
    if !replies(root, &json!(7)).is_empty() {
        return Err("Resolved request sent a second reply".into());
    }
    // Unsupported schema cannot acquire a Submit action; declining still works.
    let bad = json!("unsupported");
    let mut request = form(bad.clone());
    request["params"]["requestedSchema"]["properties"]["nested"] = json!({"type":"object"});
    emit(root, json!([request])).await?;
    let p = opened(ui, &bad).await?;
    if p.smoke_click("Submit").is_ok() {
        return Err("Unsupported schema can be accepted".into());
    }
    p.smoke_click("Decline")?;
    answered(root, &bad).await?;
    // Exercise the actual session-cache approval response, not a synthetic result.
    emit(root,json!([{"id":"approval-session","method":"item/commandExecution/requestApproval","params":{"threadId":"target","command":"private-test-command","availableDecisions":["accept","acceptForSession","decline"]}}])).await?;
    let approval = || {
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
            .find(|d| {
                d.is_visible()
                    && d.heading().as_deref()
                        == Some(&crate::language::tr("Agent needs your input"))
            })
    };
    until("approval", || approval().is_some()).await?;
    let dialog = approval().unwrap();
    if !dialog.has_response("session") {
        return Err("Session approval action is missing".into());
    }
    approval_click(&dialog, "Allow for session")?;
    if answered(root, &json!("approval-session")).await?["result"]
        != json!({"decision":"acceptForSession"})
    {
        return Err("Session approval became an allow-once response".into());
    }
    until("approval dismissed", || approval().is_none()).await?;
    emit(root,json!([{"id":"permissions-session","method":"item/permissions/requestApproval","params":{"threadId":"target","permissions":{"network":{"enabled":true}}}}])).await?;
    until("permission profile approval", || approval().is_some()).await?;
    approval_click(&approval().unwrap(), "Allow for session")?;
    if answered(root, &json!("permissions-session")).await?["result"]
        != json!({"scope":"session","permissions":{"network":{"enabled":true}}})
    {
        return Err("Permission grant lost its session scope".into());
    }
    until("permission dismissed", || approval().is_none()).await?;
    emit(root,json!([{"id":"restricted-approval","method":"item/commandExecution/requestApproval","params":{"threadId":"target","command":"Restricted available decisions","availableDecisions":["accept","decline"]}}])).await?;
    until("restricted approval", || approval().is_some()).await?;
    let d = approval().unwrap();
    if d.has_response("session") {
        return Err("Unavailable session decision was offered".into());
    }
    approval_click(&d, "Decline")?;
    answered(root, &json!("restricted-approval")).await?;
    approvals::run(ui, root).await?;
    let lost = json!("status-lost-input");
    emit(root, json!([question(lost.clone())])).await?;
    let old_status = opened(ui, &lost).await?;
    emit(
        root,
        json!([{"method":"studio/appServer/status","params":{"state":"stopped"}}]),
    )
    .await?;
    until("backend status loss closes form", || {
        panel(ui, &lost).is_none()
    })
    .await?;
    old_status.smoke_click("Cancel")?;
    if !replies(root, &lost).is_empty() {
        return Err("Lost backend form answered while unavailable".into());
    }
    emit(
        root,
        json!([{"method":"studio/appServer/status","params":{"state":"ready"}}]),
    )
    .await?;
    until("restored backend status", || {
        ui.state.borrow().backend_ready.contains("codex")
    })
    .await?;
    let stale = json!("reconnect-input");
    emit(root, json!([question(stale.clone())])).await?;
    let old = opened(ui, &stale).await?;
    let generation = ui.state.borrow().socket_generation;
    ui.reconnect();
    until("reconnect closes old input", || {
        ui.state.borrow().socket_generation != generation
            && ui.interactions.borrow().is_empty()
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
    })
    .await?;
    old.smoke_click("Submit")?;
    if !replies(root, &stale).is_empty() {
        return Err("Old dialog answered after reconnect".into());
    }
    emit(root, json!([question(stale.clone())])).await?;
    opened(ui, &stale).await?.smoke_click("Cancel")?;
    answered(root, &stale).await?;
    if ui.composer_text() != "Keep my unsent input draft 中文😀 " {
        return Err("Input workflow lost the unsent composer draft".into());
    }
    Ok(
        json!({"ok":true,"localizedControls":localized(),"choiceAndOther":true,"secretEntry":true,"typedMcpForm":true,"validationBeforeReply":true,"cancelAndDecline":true,"explicitUrl":true,"resolvedRequestIsolation":true,"reconnectIsolation":true,"sessionApproval":true,"permissionSubsets":true,"strictAutoReview":true,"lateApprovalValidation":true,"permissionAliases":true,"persistentApprovalPolicies":true,"availableDecisions":true,"approvalStopTurn":true,"draftRetention":true,"firstFormMappedMs":mapped_ms,"workflowHeartbeatIncludingCaptures":pulse.stop()}),
    )
}
