//! Exact permission subsets and late validation guards over private providers.
use super::*;
fn controls(widget: &gtk::Widget) -> Vec<gtk::CheckButton> {
    if let Some(c) = widget.downcast_ref::<gtk::CheckButton>() {
        return vec![c.clone()];
    }
    let mut result = Vec::new();
    let mut c = widget.first_child();
    while let Some(w) = c {
        c = w.next_sibling();
        result.extend(controls(&w));
    }
    result
}
fn profile() -> Value {
    json!({"network":{"enabled":true},"fileSystem":{"entries":[
    {"access":"write","path":{"type":"path","path":"/private/project 中文/output.rs"}},
    {"access":"read","path":{"type":"glob_pattern","pattern":"/private/project/**/*.md"}},
    {"access":"deny","path":{"type":"path","path":"/private/project/secrets"}},
    {"access":"write","path":{"type":"special","value":{"kind":"project_roots","subpath":"out"}}}
],"read":["/outside/notes"],"write":["/private/project 中文/output.rs","C:\\project\\output.rs"],"globScanMaxDepth":4}})
}
async fn show(ui: &Ui, root: &Path, id: &str, value: Value) -> Result<adw::MessageDialog, String> {
    emit(root,json!([request(id,"item/permissions/requestApproval",json!({"cwd":"/private/project 中文","reason":"Choose the permissions needed for this task","permissions":value}))])).await?;
    open(ui, id).await
}
fn pending(ui: &Ui, id: &str) -> bool {
    let s = ui.state.borrow();
    let key = Scope::new(&s.backend, s.socket_generation, &json!(id)).unwrap();
    ui.approvals
        .borrow()
        .get(&key)
        .is_some_and(|a| a.pending.get())
}
fn local_event(ui: &Rc<Ui>, payload: Value) {
    let s = ui.state.borrow();
    let (backend, generation) = (s.backend.clone(), s.socket_generation);
    drop(s);
    ui.event(Event::Wire {
        backend,
        generation,
        payload,
        prepared: protocol::Prepared::default(),
    });
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<(), String> {
    let value = profile();
    let d = show(ui, root, "permissions-subset-review", value.clone()).await?;
    let check = controls(d.upcast_ref());
    if check.len() != 6 || check.iter().any(|c| !c.is_active()) {
        return Err("Requested aliases were duplicated or grants were silently changed".into());
    }
    approval_click(&d, "Clear selection")?;
    check[2].set_active(true);
    check[2].grab_focus();
    if !contains(d.upcast_ref(), &selected_text(1, 6))
        || !contains(d.upcast_ref(), "Deny file access")
    {
        return Err("Permission selection or read-only restrictions were not shown".into());
    }
    if localized() {
        let before = check
            .iter()
            .map(|button| button.is_active())
            .collect::<Vec<_>>();
        switch_language(ui, "en-US", 2).await?;
        if !contains(d.upcast_ref(), "1 of 6 requested permissions selected") {
            return Err("Existing permission counter did not switch to English".into());
        }
        switch_language(ui, "zh-CN", 1).await?;
        if !contains(d.upcast_ref(), "已选择 1/6 项请求权限")
            || check
                .iter()
                .map(|button| button.is_active())
                .collect::<Vec<_>>()
                != before
            || !replies(root, &json!("permissions-subset-review")).is_empty()
        {
            return Err("Live permission translation changed/submitted the selected grants".into());
        }
    }
    capture(
        ui,
        "permissions-subset-review",
        root,
        "permissions-subset.png",
    )
    .await?;
    approval_click(&d, "Allow for turn with review")?;
    let result = answered(root, &json!("permissions-subset-review")).await?;
    let selected = json!({"fileSystem":{"entries":[value["fileSystem"]["entries"][1],value["fileSystem"]["entries"][2]],"read":[],"write":[],"globScanMaxDepth":4}});
    if result["result"] != json!({"scope":"turn","permissions":selected,"strictAutoReview":true}) {
        return Err(format!("Strict-review subset changed: {}", result));
    }
    let d = show(ui, root, "permissions-subset-session", value.clone()).await?;
    approval_click(&d, "Clear selection")?;
    controls(d.upcast_ref())[5].set_active(true);
    approval_click(&d, "Allow for session")?;
    let expected = json!({"fileSystem":{"entries":[value["fileSystem"]["entries"][2]],"read":[],"write":["C:\\project\\output.rs"],"globScanMaxDepth":4}});
    if answered(root, &json!("permissions-subset-session")).await?["result"]
        != json!({"scope":"session","permissions":expected})
    {
        return Err("Session grant added permissions or lost restrictions".into());
    }
    let d = show(ui, root, "permissions-none", value.clone()).await?;
    approval_click(&d, "Clear selection")?;
    approval_click(&d, "Allow for this turn")?;
    if answered(root, &json!("permissions-none")).await?["result"]
        != json!({"scope":"turn","permissions":{}})
    {
        return Err("An empty selection granted permissions".into());
    }
    let d = show(ui, root, "permissions-all", value.clone()).await?;
    approval_click(&d, "Clear selection")?;
    approval_click(&d, "Select all")?;
    if !contains(d.upcast_ref(), &selected_text(6, 6)) {
        return Err("Select all did not restore all scopes".into());
    }
    approval_click(&d, "Allow for this turn")?;
    if answered(root, &json!("permissions-all")).await?["result"]
        != json!({"scope":"turn","permissions":value})
    {
        return Err("Full selection changed the original profile".into());
    }
    let d = show(
        ui,
        root,
        "permissions-unsupported",
        json!({"network":{"enabled":true},"futurePermission":true}),
    )
    .await?;
    if !controls(d.upcast_ref()).is_empty() || d.has_response("accept") || d.has_response("review")
    {
        return Err("An unsupported profile acquired grant controls".into());
    }
    approval_click(&d, "Close")?;
    if answered(root, &json!("permissions-unsupported")).await?["error"]["code"] != -32601 {
        return Err("Unsupported permission profile was granted".into());
    }
    let d = show(ui, root, "permissions-cancel-pending", value.clone()).await?;
    approval_click(&d, "Allow for turn with review")?;
    if !pending(ui, "permissions-cancel-pending") {
        return Err("Approval did not enter bounded worker validation".into());
    }
    // No main-loop yield: closing overrides a grant whose worker callback has
    // not yet committed. It must produce only the explicit decline.
    d.close();
    if answered(root, &json!("permissions-cancel-pending")).await?["result"]
        != json!({"permissions":{}})
    {
        return Err("Closing a validating approval still granted permissions".into());
    }
    let d = show(ui, root, "permissions-resolved-pending", value.clone()).await?;
    approval_click(&d, "Allow for this turn")?;
    if !pending(ui, "permissions-resolved-pending") {
        return Err("Grant was sent synchronously from GTK".into());
    }
    local_event(
        ui,
        json!({"method":"serverRequest/resolved","params":{"threadId":"target","requestId":"permissions-resolved-pending"}}),
    );
    if dialog(ui, "permissions-resolved-pending").is_some() {
        return Err("Resolved validation stayed open".into());
    }
    let d = show(ui, root, "permissions-disconnected-pending", value.clone()).await?;
    approval_click(&d, "Allow for this turn")?;
    local_event(
        ui,
        json!({"method":"studio/appServer/status","params":{"state":"stopped"}}),
    );
    if dialog(ui, "permissions-disconnected-pending").is_some() {
        return Err("Disconnected validation stayed open".into());
    }
    emit(
        root,
        json!([{"method":"studio/appServer/status","params":{"state":"ready"}}]),
    )
    .await?;
    until("backend ready after permission cancellation", || {
        ui.state.borrow().ready && ui.state.borrow().backend_ready.contains("codex")
    })
    .await?;
    let d = show(ui, root, "permissions-reconnect-pending", value).await?;
    approval_click(&d, "Allow for session")?;
    let old_generation = ui.state.borrow().socket_generation;
    ui.reconnect();
    until("permission validation reconnect", || {
        ui.state.borrow().socket_generation != old_generation
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.state.borrow().options_loaded
    })
    .await?;
    let d = show(
        ui,
        root,
        "permissions-reconnect-pending",
        json!({"network":{"enabled":true}}),
    )
    .await?;
    approval_click(&d, "Decline")?;
    if answered(root, &json!("permissions-reconnect-pending")).await?["result"]
        != json!({"permissions":{}})
    {
        return Err("Old permission validation crossed connection generations".into());
    }
    let large = json!({"fileSystem":{"read":(0..128).map(|i|format!("/private/{i}/file.md")).collect::<Vec<_>>()}});
    let d = show(ui, root, "permissions-large", large).await?;
    if controls(d.upcast_ref()).len() != 128 {
        return Err("Bounded large permission list was truncated".into());
    }
    let start = std::time::Instant::now();
    approval_click(&d, "Clear selection")?;
    approval_click(&d, "Select all")?;
    if !contains(d.upcast_ref(), &selected_text(128, 128)) {
        return Err("Large permission selection count diverged".into());
    }
    let bulk_ms = start.elapsed().as_millis();
    if bulk_ms > 1000 {
        return Err("Bounded permission selection blocked GTK for over a second".into());
    }
    approval_click(&d, "Decline")?;
    if answered(root, &json!("permissions-large")).await?["result"] != json!({"permissions":{}}) {
        return Err("Decline granted the selected large profile".into());
    }
    for id in [
        "permissions-resolved-pending",
        "permissions-disconnected-pending",
    ] {
        if !replies(root, &json!(id)).is_empty() {
            return Err(format!("Late validation answered {id}"));
        }
    }
    std::fs::write(
        root.join("permission-selection.json"),
        serde_json::to_vec_pretty(&json!({"grants":128,"bulkSelectionMs":bulk_ms})).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn selected_text(selected: usize, total: usize) -> String {
    crate::language::format(
        "{selected} of {total} requested permissions selected",
        &[
            ("selected", &selected.to_string()),
            ("total", &total.to_string()),
        ],
    )
}
