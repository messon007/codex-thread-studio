//! Bounded MCP inventory projection. Never pass tool schemas/resources to GTK.
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Debug)]
pub struct Server {
    pub name: String,
    pub detail: String,
    pub status: String,
}
#[derive(Debug)]
pub struct Page {
    pub servers: Vec<Arc<Server>>,
    pub cursor: Option<String>,
}
fn clean(value: &Value, length: usize) -> String {
    value
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(length)
        .collect()
}
fn status_label(value: &str) -> &str {
    match value {
        "oAuth" => "OAuth",
        "bearerToken" => "Bearer token",
        "notLoggedIn" => "Sign-in required",
        "unsupported" => "Authentication unsupported",
        "unknown" => "Authentication unknown",
        "notStarted" => "Not started",
        "starting" => "Connecting…",
        "connected" => "Connected",
        "authenticationRequired" => "Authentication required",
        "failed" => "Connection failed",
        "cancelled" => "Cancelled",
        "disabled" => "Disabled",
        other => other,
    }
}
pub fn project(result: &Value) -> Result<Page, String> {
    let rows = result["data"]
        .as_array()
        .ok_or("Invalid MCP status response")?;
    if rows.len() > 100 {
        return Err("MCP status page exceeds the 100-server budget".into());
    }
    let mut servers = Vec::new();
    for server in rows {
        let name = clean(&server["name"], 256);
        if name.is_empty() {
            return Err("MCP server has no name".into());
        }
        let tools = server["tools"]
            .as_object()
            .ok_or("Invalid MCP tool inventory")?
            .len();
        let resources = server["resources"]
            .as_array()
            .ok_or("Invalid MCP resource inventory")?
            .len();
        let templates = server["resourceTemplates"].as_array().map_or(0, Vec::len);
        let auth = clean(&server["authStatus"], 64);
        let runtime = clean(&server["runtimeStatus"], 64);
        let error = clean(&server["toolsError"], 400);
        let mut detail = format!(
            "{tools} tool{} · {resources} resource{} · {templates} template{}",
            if tools == 1 { "" } else { "s" },
            if resources == 1 { "" } else { "s" },
            if templates == 1 { "" } else { "s" }
        );
        if !error.is_empty() {
            detail.push_str(&format!("\n{error}"));
        }
        servers.push(Arc::new(Server {
            name,
            detail,
            status: if runtime.is_empty() {
                status_label(&auth).to_owned()
            } else {
                format!("{} · {}", status_label(&runtime), status_label(&auth))
            },
        }));
    }
    let cursor = match result.get("nextCursor") {
        None | Some(Value::Null) => None,
        Some(Value::String(cursor)) if cursor.is_empty() => None,
        Some(Value::String(cursor)) if cursor.len() <= 4096 => Some(cursor.clone()),
        _ => return Err("Invalid MCP pagination cursor".into()),
    };
    Ok(Page { servers, cursor })
}
/// Other array responses (session catalogs, skills, search) keep their own types.
pub fn prepare(payload: &mut Value) -> Option<Result<Page, String>> {
    if payload.get("method").is_some() || payload.get("id").is_none() {
        return None;
    }
    let result = &payload["result"];
    let rows = result["data"].as_array()?;
    if rows.is_empty() {
        return Some(project(result));
    }
    if !rows
        .iter()
        .any(|row| row.get("authStatus").is_some() && row.get("tools").is_some())
    {
        return None;
    }
    let page = project(result);
    payload["result"] = Value::Null;
    Some(page)
}
#[derive(Default)]
pub struct Inventory {
    pub servers: Vec<Arc<Server>>,
    cursors: HashSet<String>,
    pages: usize,
}
impl Inventory {
    pub fn append(&mut self, page: Page) -> Result<Option<String>, String> {
        if self.pages >= 10 || self.servers.len() + page.servers.len() > 1000 {
            return Err("MCP inventory exceeds 1,000 servers / 10 pages; the displayed inventory is partial".into());
        }
        if page
            .cursor
            .as_ref()
            .is_some_and(|cursor| self.cursors.contains(cursor))
        {
            return Err(
                "The backend repeated an MCP cursor; the displayed inventory is partial".into(),
            );
        }
        self.pages += 1;
        self.servers.extend(page.servers);
        if let Some(cursor) = &page.cursor {
            self.cursors.insert(cursor.clone());
        }
        Ok(page.cursor)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn worker_drops_schemas_resources_and_preserves_bounded_counts_and_status() {
        let mut payload = json!({"id":1,"result":{"data":[{"name":"native\n中文","authStatus":"oAuth","runtimeStatus":"connected","tools":{"secret-schema":{"inputSchema":{"big":"x".repeat(100000)}}},"resources":[{"uri":"private://resource"}],"resourceTemplates":[],"toolsError":null}],"nextCursor":"next"}});
        let page = prepare(&mut payload).unwrap().unwrap();
        assert!(payload["result"]["data"].is_null());
        assert_eq!(page.servers[0].name, "native中文");
        assert!(page.servers[0].detail.starts_with("1 tool · 1 resource"));
        assert_eq!(page.servers[0].status, "Connected · OAuth");
        let mut inventory = Inventory::default();
        assert_eq!(inventory.append(page).unwrap().as_deref(), Some("next"));
        assert!(inventory
            .append(Page {
                servers: vec![],
                cursor: Some("next".into())
            })
            .is_err());
        assert_eq!(inventory.servers.len(), 1);
        let mut other = json!({"id":2,"result":{"data":[{"id":"session","name":"A"}]}});
        assert!(prepare(&mut other).is_none());
        assert_eq!(other["result"]["data"][0]["id"], "session");
    }
    #[test]
    fn empty_missing_and_over_budget_pages_are_distinct() {
        assert!(project(&json!({})).is_err());
        assert!(project(&json!({"data":[]})).unwrap().servers.is_empty());
        assert!(project(&json!({"data":[{"name":"invalid"}]})).is_err());
        assert!(project(&json!({"data":[],"nextCursor":"x".repeat(4097)})).is_err());
        let mut inventory = Inventory::default();
        for _ in 0..10 {
            inventory
                .append(Page {
                    servers: vec![],
                    cursor: None,
                })
                .unwrap();
        }
        assert!(inventory
            .append(Page {
                servers: vec![],
                cursor: None
            })
            .is_err());
    }
}
