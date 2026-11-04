//! Shared native command metadata and explicit future-message permission choices.
use crate::composer_refs::{Kind, Suggestion};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
pub const COMMANDS: &[(&str, &str)] = &[
    ("model", "Choose model and reasoning effort"),
    ("fast", "Toggle Codex Fast mode"),
    ("permissions", "Set permissions for future messages"),
    ("status", "View current session and turn settings"),
    ("compact", "Compact conversation context"),
    ("review", "Review uncommitted code changes"),
    ("diff", "View the accumulated conversation turn diff"),
    ("skills", "Browse skills and installed plugins"),
    ("mcp", "View MCP server status and inventory counts"),
    (
        "router",
        "Edit Thread Router settings and session responsibility",
    ),
    ("subagents", "Browse the main agent and its subagents"),
    ("rename", "Rename this session"),
    ("fork", "Fork this session"),
    ("new", "Create a session"),
    ("copy", "Copy the latest assistant response"),
    ("archive", "Archive this Codex session"),
    ("delete", "Delete this session with confirmation"),
];
pub fn known(name: &str) -> bool {
    COMMANDS.iter().any(|(command, _)| *command == name) || name == "agent"
}
pub fn catalog(opencode: bool) -> Arc<Vec<Arc<Suggestion>>> {
    Arc::new(
        COMMANDS
            .iter()
            .filter(|(name, _)| {
                !opencode || !matches!(*name, "fast" | "archive" | "mcp" | "subagents")
            })
            .map(|(name, description)| {
                Arc::new(Suggestion {
                    kind: Kind::Command,
                    name: (*name).into(),
                    path: format!("command:{name}"),
                    title: format!("/{name}"),
                    detail: (*description).into(),
                })
            })
            .collect(),
    )
}
pub fn slash(text: &str) -> Option<&str> {
    let value = text.trim().strip_prefix('/')?;
    if known(value) {
        Some(value)
    } else {
        None
    }
}
pub fn shell(text: &str) -> Option<&str> {
    text.trim_start().strip_prefix('!').map(str::trim)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Inherit,
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}
impl Access {
    pub fn label(self) -> &'static str {
        match self {
            Self::Inherit => "Inherit session",
            Self::ReadOnly => "Read only",
            Self::WorkspaceWrite => "Workspace write",
            Self::FullAccess => "Full access",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Policy {
    pub access: Access,
    pub root: String,
}
#[derive(Default)]
pub struct Policies(HashMap<String, Policy>);
impl Policies {
    pub fn get(&self, key: &str) -> Option<&Policy> {
        self.0.get(key)
    }
    pub fn set(&mut self, key: &str, root: &str, access: Access) -> Result<(), String> {
        if access == Access::Inherit {
            self.0.remove(key);
            return Ok(());
        }
        if root.is_empty() {
            return Err("Choose a session with a project folder first".into());
        }
        if !self.0.contains_key(key) && self.0.len() >= 128 {
            return Err("Reset an unused session to inherited permissions first".into());
        }
        self.0.insert(
            key.into(),
            Policy {
                access,
                root: root.into(),
            },
        );
        Ok(())
    }
    pub fn router_snapshot(&self) -> Vec<Value> {
        self.0.iter().map(|(key,policy)| json!({"sessionKey":key,"cwd":policy.root,"access":match policy.access {
            Access::Inherit=>"inherit",Access::ReadOnly=>"readOnly",Access::WorkspaceWrite=>"workspaceWrite",Access::FullAccess=>"fullAccess"
        }})).collect()
    }
    pub fn forget(&mut self, key: &str) {
        self.0.remove(key);
    }
    pub fn apply(
        &self,
        key: &str,
        root: &str,
        codex: bool,
        params: &mut Value,
    ) -> Result<(), String> {
        if !codex {
            return Ok(());
        }
        let Some(policy) = self.get(key) else {
            return Ok(());
        };
        if policy.root != root {
            return Err("The project changed; choose its message permissions again".into());
        }
        match policy.access {
            Access::Inherit => {}
            Access::ReadOnly => {
                params["approvalPolicy"] = json!("on-request");
                params["sandboxPolicy"] = json!({"type":"readOnly"});
            }
            Access::WorkspaceWrite => {
                params["approvalPolicy"] = json!("on-request");
                params["sandboxPolicy"] = json!({"type":"workspaceWrite","writableRoots":[policy.root],"networkAccess":false});
            }
            Access::FullAccess => {
                params["approvalPolicy"] = json!("never");
                params["sandboxPolicy"] = json!({"type":"dangerFullAccess"});
            }
        }
        Ok(())
    }
}
#[derive(Default)]
pub struct Shells(HashMap<String, (u64, Option<String>)>);
impl Shells {
    pub fn request_failed(&mut self, key: &str) {
        if self.0.get(key).is_some_and(|(_, turn)| turn.is_none()) {
            self.0.remove(key);
        }
    }
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    pub fn begin(&mut self, key: &str, generation: u64) -> Result<(), String> {
        if !self.0.contains_key(key) && self.0.len() >= 128 {
            return Err("Too many active shell sessions".into());
        }
        self.0.insert(key.into(), (generation, None));
        Ok(())
    }
    pub fn forget(&mut self, key: &str) {
        self.0.remove(key);
    }
    pub fn disconnected(&mut self, backend: Option<&str>) {
        self.0.retain(|key, _| {
            backend.is_some_and(|backend| !key.starts_with(&format!("{backend}:")))
        });
    }
    pub fn history(&mut self, key: &str, generation: u64, active: Option<&str>, shell: bool) {
        if let Some(turn) = active {
            if shell || self.contains(key) {
                self.0.insert(key.into(), (generation, Some(turn.into())));
            }
        } else {
            self.forget(key);
        }
    }
    pub fn observe(&mut self, key: &str, generation: u64, method: &str, params: &Value) -> bool {
        let Some((stored, turn)) = self.0.get_mut(key) else {
            return false;
        };
        if *stored != generation {
            return false;
        }
        let id = params
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .or(params["turnId"].as_str())
            .unwrap_or("");
        if method == "turn/started" && turn.is_none() && !id.is_empty() {
            *turn = Some(id.into());
        }
        if method == "turn/completed"
            && (id.is_empty() || turn.as_ref().is_none_or(|turn| turn == id))
        {
            self.0.remove(key);
            return true;
        }
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_paths_and_shell_whitespace_are_unambiguous() {
        assert_eq!(slash(" /status \n"), Some("status"));
        assert_eq!(slash("/home/me/file.md"), None);
        assert_eq!(slash("/review these files"), None);
        assert_eq!(shell(" \n!echo 中文😀 \n"), Some("echo 中文😀"));
        assert!(catalog(true).iter().all(|command| !matches!(
            command.name.as_str(),
            "fast" | "archive" | "mcp" | "subagents"
        )));
    }
    #[test]
    fn permissions_are_scoped_to_original_session_project_and_backend() {
        let mut policies = Policies::default();
        policies
            .set("codex:a", "/project", Access::WorkspaceWrite)
            .unwrap();
        let mut params = json!({});
        policies
            .apply("codex:a", "/project", true, &mut params)
            .unwrap();
        assert_eq!(params["approvalPolicy"], "on-request");
        assert_eq!(params["sandboxPolicy"]["networkAccess"], false);
        assert_eq!(params["sandboxPolicy"]["writableRoots"][0], "/project");
        assert!(policies
            .apply("codex:a", "/other", true, &mut json!({}))
            .is_err());
        let mut other = json!({});
        policies
            .apply("opencode:a", "/project", false, &mut other)
            .unwrap();
        assert!(other.get("sandboxPolicy").is_none());
        policies
            .set("codex:a", "/project", Access::Inherit)
            .unwrap();
        let mut inherited = json!({});
        policies
            .apply("codex:a", "/other", true, &mut inherited)
            .unwrap();
        assert_eq!(inherited, json!({}));
    }
    #[test]
    fn shell_acknowledgement_and_idle_do_not_stand_in_for_turn_completion() {
        let mut shells = Shells::default();
        shells.begin("codex:a", 1).unwrap();
        assert!(!shells.observe(
            "codex:a",
            1,
            "thread/status/changed",
            &json!({"status":{"type":"idle"}})
        ));
        assert!(shells.contains("codex:a"));
        shells.observe(
            "codex:a",
            1,
            "turn/started",
            &json!({"turn":{"id":"shell-turn"}}),
        );
        assert!(!shells.observe(
            "codex:a",
            0,
            "turn/completed",
            &json!({"turn":{"id":"shell-turn"}})
        ));
        assert!(!shells.observe(
            "codex:a",
            1,
            "turn/completed",
            &json!({"turn":{"id":"older-turn"}})
        ));
        assert!(shells.observe(
            "codex:a",
            1,
            "turn/completed",
            &json!({"turn":{"id":"shell-turn"}})
        ));
    }
}
