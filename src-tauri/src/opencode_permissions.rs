//! OpenCode permission rules are ordered; only a final catch-all denies every tool.
pub fn tools_disabled(permission: &serde_json::Value) -> bool {
    permission
        .as_array()
        .and_then(|rules| rules.last())
        .is_some_and(|rule| {
            rule["permission"] == "*" && rule["pattern"] == "*" && rule["action"] == "deny"
        })
}
