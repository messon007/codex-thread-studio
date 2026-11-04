//! Real provider-wire decisions; the private provider records but executes none.
use super::*;
#[path = "permission_acceptance.rs"]
mod permissions;
fn dialog(ui: &Ui, id: &str) -> Option<adw::MessageDialog> {
    let state = ui.state.borrow();
    let key = Scope::new(&state.backend, state.socket_generation, &json!(id))?;
    ui.approvals.borrow().get(&key).map(|a| a.dialog.clone())
}
async fn open(ui: &Ui, id: &str) -> Result<adw::MessageDialog, String> {
    until("approval controls mapped", || {
        dialog(ui, id).is_some_and(|d| d.is_mapped())
    })
    .await?;
    Ok(dialog(ui, id).unwrap())
}
fn picker(widget: &gtk::Widget) -> Option<gtk::DropDown> {
    if let Some(p) = widget.downcast_ref::<gtk::DropDown>() {
        return Some(p.clone());
    }
    let mut c = widget.first_child();
    while let Some(w) = c {
        c = w.next_sibling();
        if let Some(p) = picker(&w) {
            return Some(p);
        }
    }
    None
}
fn contains(widget: &gtk::Widget, text: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|l| l.text().contains(text))
    {
        return true;
    }
    let mut c = widget.first_child();
    while let Some(w) = c {
        c = w.next_sibling();
        if contains(&w, text) {
            return true;
        }
    }
    false
}
#[allow(deprecated)] // GTK 4.14 named-color compatibility, like the other captures.
async fn capture(ui: &Ui, id: &str, root: &Path, name: &str) -> Result<(), String> {
    let d = dialog(ui, id).ok_or("Approval disappeared before capture")?;
    // Observe a real mapped frame rather than forcing allocation/frame signals.
    for _ in 0..30 {
        if let (Some(child), Some(renderer)) = (d.child(), d.renderer()) {
            if child.is_mapped() && child.width() > 0 && child.height() > 0 {
                let snapshot = gtk::Snapshot::new();
                let color = child
                    .style_context()
                    .lookup_color("window_bg_color")
                    .unwrap_or_else(|| gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0));
                snapshot.append_color(
                    &color,
                    &gtk::graphene::Rect::new(
                        0.0,
                        0.0,
                        child.width() as f32,
                        child.height() as f32,
                    ),
                );
                gtk::WidgetPaintable::new(Some(&child)).snapshot(
                    &snapshot,
                    child.width() as f64,
                    child.height() as f64,
                );
                if let Some(node) = snapshot.to_node() {
                    let texture = renderer.render_texture(&node, None);
                    let stride = texture.width() as usize * 4;
                    let mut pixels = vec![0; stride * texture.height() as usize];
                    texture.download(&mut pixels, stride);
                    if pixels.chunks_exact(4).any(|p| p != &pixels[..4]) {
                        texture
                            .save_to_png(root.join(name))
                            .map_err(|e| e.to_string())?;
                        return Ok(());
                    }
                }
            }
        }
        glib::timeout_future(Duration::from_millis(40)).await;
    }
    Err("Approval did not paint within the capture budget".into())
}
fn request(id: &str, method: &str, params: Value) -> Value {
    let mut params = params;
    params["threadId"] = "target".into();
    params["turnId"] = "private-turn".into();
    params["itemId"] = "private-item".into();
    params["startedAtMs"] = 123.into();
    json!({"id":id,"method":method,"params":params})
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<(), String> {
    let exec = json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cargo","test","--package","中文 package"]}});
    let allow = json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":{"host":"api.example.invalid","action":"allow"}}});
    let deny = json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":{"host":"*.example.invalid","action":"deny"}}});
    for (id, decision, label) in [
        ("policy-exec", exec.clone(), "中文 package"),
        ("policy-allow", allow.clone(), "api.example.invalid"),
        ("policy-deny", deny.clone(), "*.example.invalid"),
    ] {
        emit(
            root,
            json!([request(
                id,
                "item/commandExecution/requestApproval",
                json!({"command":"private test command","availableDecisions":["decline",decision]})
            )]),
        )
        .await?;
        let d = open(ui, id).await?;
        if d.has_response("accept")
            || d.has_response("session")
            || d.has_response("cancel")
            || d.is_response_enabled("policy")
        {
            return Err("Approval invented an unavailable or preselected decision".into());
        }
        let p = picker(d.upcast_ref()).ok_or("Native policy picker is missing")?;
        if p.selected() != 0 || !replies(root, &json!(id)).is_empty() {
            return Err("Policy selected/submitted on receipt".into());
        }
        p.set_selected(1);
        until("selected policy scope", || {
            d.is_response_enabled("policy") && contains(d.upcast_ref(), label)
        })
        .await?;
        if localized() && id == "policy-exec" {
            switch_language(ui, "en-US", 2).await?;
            if p.selected() != 1
                || !d.is_response_enabled("policy")
                || !contains(d.upcast_ref(), label)
            {
                return Err("English translation changed the chosen approval policy".into());
            }
            switch_language(ui, "zh-CN", 1).await?;
            if p.selected() != 1
                || !contains(d.upcast_ref(), "应用选中的策略")
                || !contains(d.upcast_ref(), label)
                || !replies(root, &json!(id)).is_empty()
            {
                return Err(
                    "Live approval translation changed/submitted a policy or lost its scope".into(),
                );
            }
        }
        capture(ui, id, root, &format!("{id}.png")).await?;
        approval_click(&d, "Apply selected policy")?;
        if answered(root, &json!(id)).await?["result"] != json!({"decision":decision}) {
            return Err("Policy reply changed server-proposed scope".into());
        }
        d.response("policy");
    }
    for (id, method) in [
        ("cancel-command", "item/commandExecution/requestApproval"),
        ("cancel-files", "item/fileChange/requestApproval"),
    ] {
        emit(root,json!([request(id,method,json!({"command":"private stop","grantRoot":"/private/project","availableDecisions":["accept","cancel"]}))])).await?;
        let d = open(ui, id).await?;
        if method.contains("commandExecution") && d.has_response("deny") {
            return Err("Unavailable decline was offered".into());
        }
        approval_click(&d, "Decline and stop turn")?;
        if answered(root, &json!(id)).await?["result"] != json!({"decision":"cancel"}) {
            return Err("Stop-turn approval became ordinary decline".into());
        }
    }
    emit(
        root,
        json!([request(
            "empty-decisions",
            "item/commandExecution/requestApproval",
            json!({"availableDecisions":[]})
        )]),
    )
    .await?;
    let d = open(ui, "empty-decisions").await?;
    if d.has_response("accept")
        || d.has_response("session")
        || d.has_response("policy")
        || d.has_response("deny")
    {
        return Err("Empty decision list acquired an action".into());
    }
    approval_click(&d, "Close")?;
    if answered(root, &json!("empty-decisions")).await?["error"]["code"] != -32001 {
        return Err("Empty decisions returned a successful grant".into());
    }
    emit(root,json!([request("bad-policy","item/commandExecution/requestApproval",json!({"availableDecisions":[{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":[]}}]}))])).await?;
    let d = open(ui, "bad-policy").await?;
    if picker(d.upcast_ref()).is_some() || d.has_response("accept") {
        return Err("Malformed policy acquired approval controls".into());
    }
    approval_click(&d, "Close")?;
    if answered(root, &json!("bad-policy")).await?["error"]["code"] != -32601 {
        return Err("Malformed policy was accepted".into());
    }
    emit(
        root,
        json!([request(
            "legacy-proposal-once",
            "item/commandExecution/requestApproval",
            json!({"command":"private legacy test","proposedExecpolicyAmendment":["cargo","test"]})
        )]),
    )
    .await?;
    let d = open(ui, "legacy-proposal-once").await?;
    picker(d.upcast_ref())
        .ok_or("Legacy proposal missing")?
        .set_selected(1);
    approval_click(&d, "Allow once")?;
    if answered(root, &json!("legacy-proposal-once")).await?["result"]
        != json!({"decision":"accept"})
    {
        return Err("Allow once accidentally saved a policy".into());
    }
    emit(
        root,
        json!([request(
            "resolved-policy",
            "item/commandExecution/requestApproval",
            json!({"availableDecisions":["decline",exec]})
        )]),
    )
    .await?;
    let old = open(ui, "resolved-policy").await?;
    picker(old.upcast_ref())
        .ok_or("Pending policy picker missing")?
        .set_selected(1);
    emit(root,json!([{"method":"serverRequest/resolved","params":{"threadId":"target","requestId":"resolved-policy"}}])).await?;
    until("resolved policy closes", || {
        dialog(ui, "resolved-policy").is_none()
    })
    .await?;
    old.response("policy");
    if !replies(root, &json!("resolved-policy")).is_empty() {
        return Err("Resolved policy sent a stale amendment".into());
    }
    permissions::run(ui, root).await?;
    Ok(())
}
