//! Bounded approval decisions projected on a Rust worker, never inferred by GTK.
use serde_json::{json, Value};
#[path = "approval_permissions.rs"]
mod permissions;
pub use permissions::Profile;

#[derive(Debug)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub explanation: String,
    pub persistent: bool,
    pub result: Value,
}
#[derive(Debug)]
pub struct Request {
    pub details: String,
    pub actions: Vec<Action>,
    pub close: String,
    pub permissions: Option<Profile>,
}
fn text(value: &Value, max: usize) -> Result<&str, String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control))
        .ok_or_else(|| "Invalid approval policy text".into())
}
fn prefix(value: &Value) -> Result<Vec<String>, String> {
    let rows = value
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 64)
        .ok_or("Invalid command-prefix policy")?;
    // An argv element may be empty or contain whitespace/newlines. Show JSON
    // quoting in the review and return it exactly; never split shell text.
    rows.iter()
        .map(|v| {
            v.as_str()
                .filter(|s| s.len() <= 4096 && !s.contains('\0'))
                .map(str::to_owned)
                .ok_or_else(|| "Invalid command-prefix argument".into())
        })
        .collect()
}
fn network(value: &Value) -> Result<(), String> {
    let object = value.as_object().ok_or("Invalid network policy")?;
    if object.len() != 2 || !matches!(value["action"].as_str(), Some("allow" | "deny")) {
        return Err("Invalid network policy action".into());
    }
    text(&value["host"], 1024)?;
    Ok(())
}
fn decision(value: &Value, index: usize) -> Result<Action, String> {
    let simple = match value.as_str() {
        Some("accept") => Some(("accept", "Allow once")),
        Some("acceptForSession") => Some(("session", "Allow for session")),
        Some("decline") => Some(("deny", "Decline")),
        Some("cancel") => Some(("cancel", "Decline and stop turn")),
        Some(_) => return Err("Unsupported approval decision".into()),
        None => None,
    };
    if let Some((id, label)) = simple {
        return Ok(Action {
            id: id.into(),
            label: label.into(),
            explanation: String::new(),
            persistent: false,
            result: json!({"decision":value}),
        });
    }
    let object = value
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or("Invalid approval decision")?;
    let (label, explanation) = if let Some(inner) = object.get("acceptWithExecpolicyAmendment") {
        if inner.as_object().is_none_or(|o| o.len() != 1) {
            return Err("Invalid command-prefix policy".into());
        }
        let argv = prefix(&inner["execpolicy_amendment"])?;
        ("Allow command prefix in future".into(), format!("Approve this command and save a rule for future commands beginning with these exact arguments:\n{}", serde_json::to_string_pretty(&argv).map_err(|e|e.to_string())?))
    } else if let Some(inner) = object.get("applyNetworkPolicyAmendment") {
        if inner.as_object().is_none_or(|o| o.len() != 1) {
            return Err("Invalid network policy".into());
        }
        let rule = &inner["network_policy_amendment"];
        network(rule)?;
        let allow = rule["action"] == "allow";
        (
            if allow {
                "Allow host in future"
            } else {
                "Deny host in future"
            }
            .into(),
            format!(
                "Save a network rule for future requests.\nAction: {}\nHost: {}",
                if allow { "allow" } else { "deny" },
                serde_json::to_string(&rule["host"]).map_err(|e| e.to_string())?
            ),
        )
    } else {
        return Err("Unsupported approval decision".into());
    };
    if serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > 16384 {
        return Err("Approval policy exceeds 16 KiB".into());
    }
    Ok(Action {
        id: format!("policy-{index}"),
        label,
        explanation,
        persistent: true,
        result: json!({"decision":value}),
    })
}
fn details(params: &Value) -> Result<String, String> {
    let mut remaining = params
        .as_object()
        .ok_or("Invalid approval request")?
        .clone();
    // Routing identities are in the fixed dialog caption; proposals are
    // reviewed in the policy picker. Keep all other fields visible, including
    // fields introduced by a newer server.
    for key in [
        "threadId",
        "turnId",
        "itemId",
        "approvalId",
        "startedAtMs",
        "availableDecisions",
        "proposedExecpolicyAmendment",
        "proposedNetworkPolicyAmendments",
    ] {
        remaining.remove(key);
    }
    let mut sections = Vec::new();
    for (key, title) in [
        ("kind", "Action"),
        ("command", "Command or terminal input"),
        ("cwd", "Working directory"),
        ("environmentId", "Execution environment"),
        ("reason", "Reason"),
        ("grantRoot", "Requested write root for this session"),
        ("networkApprovalContext", "Network request"),
        ("additionalPermissions", "Additional permissions requested"),
        ("permissions", "Permissions requested"),
        ("permission", "Permission requested"),
        ("patterns", "Requested patterns"),
        ("always", "Patterns covered by session approval"),
        ("commandActions", "Parsed command actions"),
    ] {
        if let Some(value) = remaining.remove(key).filter(|v| !v.is_null()) {
            let value = match value.as_str().filter(|s| !s.contains('\0')) {
                Some(text) => text.to_owned(),
                None => serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            };
            sections.push(format!("{title}\n{value}"));
        }
    }
    if !remaining.is_empty() {
        sections.push(format!(
            "Additional request details\n{}",
            serde_json::to_string_pretty(&remaining).map_err(|e| e.to_string())?
        ));
    }
    let details = if sections.is_empty() {
        "Review the requested action before responding.".into()
    } else {
        sections.join("\n\n")
    };
    if details.len() > 131072 {
        return Err("Approval presentation exceeds 128 KiB".into());
    }
    Ok(details)
}
pub fn parse(method: &str, params: &Value) -> Result<Request, String> {
    if !params.is_object() || serde_json::to_vec(params).map_err(|e| e.to_string())?.len() > 65536 {
        return Err(
            "Approval details exceed 64 KiB or are invalid; inspect the source conversation".into(),
        );
    }
    let permissions = if method == "item/permissions/requestApproval" {
        Some(Profile::parse(&params["permissions"])?)
    } else {
        None
    };
    let mut summary = params.clone();
    if permissions.is_some() {
        summary.as_object_mut().unwrap().remove("permissions");
    }
    let details = details(&summary)?;
    let mut actions = Vec::new();
    match method {
        "item/commandExecution/requestApproval" => {
            let values = if params["availableDecisions"].is_null() {
                let mut values = vec![
                    json!("decline"),
                    json!("cancel"),
                    json!("acceptForSession"),
                    json!("accept"),
                ];
                if !params["proposedExecpolicyAmendment"].is_null() {
                    prefix(&params["proposedExecpolicyAmendment"])?;
                    values.push(json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":params["proposedExecpolicyAmendment"]}}));
                }
                if !params["proposedNetworkPolicyAmendments"].is_null() {
                    for rule in params["proposedNetworkPolicyAmendments"]
                        .as_array()
                        .filter(|v| v.len() <= 16)
                        .ok_or("Invalid proposed network rules")?
                    {
                        network(rule)?;
                        values.push(json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":rule}}));
                    }
                }
                values
            } else {
                params["availableDecisions"]
                    .as_array()
                    .filter(|v| v.len() <= 32)
                    .ok_or("Invalid available approval decisions")?
                    .clone()
            };
            for value in values {
                let action = decision(&value, actions.len())?;
                if !actions.iter().any(|a: &Action| a.result == action.result) {
                    actions.push(action);
                }
            }
        }
        "item/fileChange/requestApproval" | "opencode/permission" => {
            for v in ["decline", "acceptForSession", "accept"]
                .into_iter()
                .chain((method != "opencode/permission").then_some("cancel"))
            {
                actions.push(decision(&json!(v), actions.len())?);
            }
        }
        "item/permissions/requestApproval" => {
            actions.push(Action {
                id: "deny".into(),
                label: "Decline".into(),
                explanation: String::new(),
                persistent: false,
                result: json!({"permissions":{}}),
            });
            for (id, label, scope) in [
                ("session", "Allow for session", "session"),
                ("accept", "Allow for this turn", "turn"),
            ] {
                actions.push(Action {
                    id: id.into(),
                    label: label.into(),
                    explanation: String::new(),
                    persistent: false,
                    result: json!({"scope":scope,"permissions":params["permissions"]}),
                });
            }
            actions.push(Action{id:"review".into(),label:"Allow for turn with review".into(),explanation:String::new(),persistent:false,result:json!({"scope":"turn","permissions":params["permissions"],"strictAutoReview":true})});
        }
        _ => return Err("This interaction is not supported by the native GNOME frontend".into()),
    }
    let close = if actions.iter().any(|a| a.id == "deny") {
        "deny"
    } else if actions.iter().any(|a| a.id == "cancel") {
        "cancel"
    } else {
        "dismiss"
    };
    Ok(Request {
        details,
        actions,
        close: close.into(),
        permissions,
    })
}
impl Request {
    /// Resolve only a worker-owned selection snapshot, never live GTK state.
    pub fn answer(
        &self,
        response: &str,
        policy: Option<usize>,
        selected: &[usize],
    ) -> Result<Option<Value>, String> {
        let action = if response == "policy" {
            policy.and_then(|i| self.actions.iter().filter(|a| a.persistent).nth(i))
        } else {
            self.actions
                .iter()
                .find(|a| !a.persistent && a.id == response)
        };
        let Some(action) = action else {
            return Ok(None);
        };
        let mut result = action.result.clone();
        if response != "deny" {
            if let Some(profile) = &self.permissions {
                result["permissions"] = profile.selected(selected)?;
            }
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const COMMAND: &str = "item/commandExecution/requestApproval";
    #[test]
    fn available_decisions_are_exact_ordered_and_deduplicated() {
        let exec = json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cargo","test","--package","中文 package"]}});
        let deny = json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":{"action":"deny","host":"*.example.invalid"}}});
        let request=parse(COMMAND,&json!({"availableDecisions":[exec,"cancel",deny,exec],"proposedExecpolicyAmendment":["unavailable"]})).unwrap();
        assert_eq!(request.actions.len(), 3);
        assert!(request.actions[0].persistent);
        assert_eq!(request.actions[0].result, json!({"decision":exec}));
        assert_eq!(request.actions[1].id, "cancel");
        assert_eq!(request.actions[2].result, json!({"decision":deny}));
        assert_eq!(request.close, "cancel");
        assert!(!request
            .actions
            .iter()
            .any(|a| a.id == "accept" || a.id == "session"));
        assert!(request.actions[0].explanation.contains("中文 package"));
        assert!(request.actions[2].explanation.contains("*.example.invalid"));
    }
    #[test]
    fn legacy_proposals_are_explicit_rules_without_changing_once_or_session() {
        let request=parse(COMMAND,&json!({"proposedExecpolicyAmendment":["git","status"],"proposedNetworkPolicyAmendments":[{"host":"example.invalid","action":"allow"}]})).unwrap();
        assert_eq!(request.actions.len(), 6);
        assert_eq!(request.actions.iter().filter(|a| a.persistent).count(), 2);
        assert_eq!(
            request
                .actions
                .iter()
                .find(|a| a.id == "accept")
                .unwrap()
                .result,
            json!({"decision":"accept"})
        );
        assert_eq!(
            request
                .actions
                .iter()
                .find(|a| a.id == "session")
                .unwrap()
                .result,
            json!({"decision":"acceptForSession"})
        );
        let empty = parse(COMMAND, &json!({"availableDecisions":[]})).unwrap();
        assert!(empty.actions.is_empty());
        assert_eq!(empty.close, "dismiss");
        let argv = json!(["printf", "", "a\nb", "中文 with spaces"]);
        let exact = parse(COMMAND, &json!({"proposedExecpolicyAmendment":argv})).unwrap();
        let rule = exact.actions.iter().find(|a| a.persistent).unwrap();
        assert_eq!(
            rule.result["decision"]["acceptWithExecpolicyAmendment"]["execpolicy_amendment"],
            argv
        );
        assert!(rule.explanation.contains("a\\nb"));
    }
    #[test]
    fn malformed_or_unknown_decisions_never_acquire_grants() {
        for d in [
            json!("future-allow"),
            json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":[]}}),
            json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":{"host":"safe.invalid\nother","action":"allow"}}}),
            json!({"applyNetworkPolicyAmendment":{"network_policy_amendment":{"host":"safe.invalid","action":"future"}}}),
            json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["git"],"extra":"hidden"}}),
        ] {
            assert!(parse(COMMAND, &json!({"availableDecisions":[d]})).is_err());
        }
        assert!(parse(COMMAND, &json!({"availableDecisions":false})).is_err());
        assert!(parse(
            COMMAND,
            &json!({"availableDecisions":vec![json!("accept");33]})
        )
        .is_err());
        assert!(parse(COMMAND, &json!({"command":"x".repeat(65536)})).is_err());
    }
    #[test]
    fn other_methods_cannot_inherit_command_policies() {
        for method in ["item/fileChange/requestApproval", "opencode/permission"] {
            let r = parse(method, &json!({"proposedExecpolicyAmendment":["git"]})).unwrap();
            assert!(!r.actions.iter().any(|a| a.persistent));
            assert_eq!(
                r.actions.iter().any(|a| a.id == "cancel"),
                method != "opencode/permission"
            );
        }
        let permissions = json!({"fileSystem":{"entries":[{"path":{"type":"glob_pattern","pattern":"/project/**/*.rs"},"access":"write"}]},"network":{"enabled":true}});
        let r = parse(
            "item/permissions/requestApproval",
            &json!({"permissions":permissions}),
        )
        .unwrap();
        assert_eq!(
            r.actions.iter().find(|a| a.id == "session").unwrap().result,
            json!({"scope":"session","permissions":permissions})
        );
        assert_eq!(
            r.actions.iter().find(|a| a.id == "deny").unwrap().result,
            json!({"permissions":{}})
        );
        assert!(parse("future/requestApproval", &json!({})).is_err());
    }
    #[test]
    fn review_summary_prioritizes_execution_scope_and_retains_unknown_fields() {
        let r=parse(COMMAND,&json!({"threadId":"internal-routing-thread","itemId":"internal-item","startedAtMs":123,"cwd":"/project 中文","command":"printf 'hello'","reason":"Network required","environmentId":"remote-env","additionalPermissions":{"network":{"enabled":true}},"futurePermission":{"scope":"visible-future-scope"},"availableDecisions":["accept","decline"]})).unwrap();
        assert!(r
            .details
            .starts_with("Command or terminal input\nprintf 'hello'"));
        for value in [
            "/project 中文",
            "Network required",
            "remote-env",
            "enabled",
            "visible-future-scope",
        ] {
            assert!(r.details.contains(value));
        }
        for value in [
            "internal-routing-thread",
            "internal-item",
            "availableDecisions",
        ] {
            assert!(!r.details.contains(value));
        }
        let nul = parse(COMMAND, &json!({"command":"hidden\0scope"})).unwrap();
        assert!(nul.details.contains("hidden\\u0000scope"));
        assert!(!nul.details.contains('\0'));
    }
    #[test]
    fn permission_scope_and_strict_review_are_independent_of_the_selected_subset() {
        let r = parse(
            "item/permissions/requestApproval",
            &json!({"permissions":{"network":{"enabled":true},"fileSystem":{"write":["/a","/b"]}}}),
        )
        .unwrap();
        for (response, scope) in [
            ("accept", "turn"),
            ("session", "session"),
            ("review", "turn"),
        ] {
            let result = r.answer(response, None, &[2]).unwrap().unwrap();
            assert_eq!(
                result["permissions"],
                json!({"fileSystem":{"write":["/b"]}})
            );
            assert_eq!(result["scope"], scope);
            assert_eq!(result["strictAutoReview"] == true, response == "review");
        }
        assert_eq!(
            r.answer("deny", None, &[999]).unwrap().unwrap(),
            json!({"permissions":{}})
        );
        assert!(r.answer("accept", None, &[999]).is_err());
        assert!(r.answer("policy", Some(0), &[0]).unwrap().is_none());
        assert!(
            r.answer("review", None, &[]).unwrap().unwrap()["permissions"]
                .as_object()
                .unwrap()
                .is_empty()
        );
        assert!(!parse("item/commandExecution/requestApproval", &json!({}))
            .unwrap()
            .actions
            .iter()
            .any(|a| a.id == "review"));
    }
}
