//! Backend-specific Router transport; all mutations are single-attempt.
use crate::{opencode_server::RouterOperation as Oc, GatewayState};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};
#[path = "native_router_command.rs"]
pub(crate) mod command;

pub struct Session {
    pub backend: String,
    pub id: String,
    pub cwd: String,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseItem {
    pub id: String,
    pub text: String,
    pub excerpt: bool,
}
pub struct Turn {
    pub map_update: Option<Result<crate::session_map_protocol::Update, String>>,
    pub items: Vec<ResponseItem>,
    pub done: bool,
    pub success: bool,
    pub text: String,
    pub error: String,
}
fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
pub async fn read(
    server: &crate::codex_app_server::CodexAppServer,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    if !matches!(
        method,
        "thread/read" | "thread/list" | "thread/turns/list" | "skills/list" | "plugin/list"
    ) {
        return Err("Router initialization retries are restricted to reads".into());
    }
    for attempt in 0..100 {
        match server.request_router(method, params.clone()).await {
            Ok(value) => return Ok(value),
            Err(error) if error.contains("still initializing") && attempt < 99 => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(error) => return Err(error),
        }
    }
    Err("Router backend did not initialize".into())
}
impl Session {
    pub fn key(&self) -> String {
        format!("{}:{}", self.backend, self.id)
    }
    pub fn new(key: &str, cwd: &str) -> Result<Self, String> {
        let (backend, id) = key.split_once(':').ok_or("Invalid routed session")?;
        if !crate::valid_router_session_key(key) || key.len() > 320 {
            return Err("Invalid routed session".into());
        }
        Ok(Self {
            backend: backend.into(),
            id: id.into(),
            cwd: cwd.into(),
        })
    }
    pub fn codex<'a>(
        &self,
        state: &'a GatewayState,
    ) -> Result<Option<&'a crate::codex_app_server::CodexAppServer>, String> {
        if let Some(instance) = state.codex_backends.get(&self.backend) {
            Ok(Some(&instance.server))
        } else if self.backend == "opencode" {
            Ok(None)
        } else {
            Err("The routed backend is no longer configured".into())
        }
    }
    pub async fn summary(&self, state: &GatewayState) -> Result<Value, String> {
        let mut thread = if let Some(server) = self.codex(state)? {
            let mut result = read(
                server,
                "thread/read",
                json!({"threadId":self.id,"includeTurns":false}),
            )
            .await?;
            result
                .get_mut("thread")
                .map(Value::take)
                .ok_or("Backend returned no thread summary")?
        } else {
            let value = state
                .opencode
                .router_request(Oc::Read(&self.id), &self.cwd)
                .await?;
            if state
                .utility_sessions
                .visibility()?
                .hidden(&self.backend, &value)
            {
                return Err("Temporary utility sessions are not Router targets".into());
            }
            let statuses = state.opencode.router_request(Oc::Status, &self.cwd).await?;
            json!({"id":value["id"],"title":value["title"],"cwd":value["directory"],"parentThreadId":value["parentID"],"status":if matches!(field(&statuses[&self.id],"type"),"busy"|"retry"){"active"}else{"idle"},"permission":value["permission"],"archived":value.pointer("/time/archived").and_then(Value::as_i64).is_some_and(|value|value>0)})
        };
        if thread["id"].as_str() != Some(self.id.as_str())
            || thread["cwd"].as_str() != Some(self.cwd.as_str())
        {
            return Err("Routed session identity or project directory changed".into());
        }
        if thread["ephemeral"] == true
            || thread["archived"] == true
            || thread["parentThreadId"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        {
            return Err("The Router target is no longer an ordinary active session".into());
        }
        let status = thread["status"]
            .as_str()
            .or_else(|| thread.pointer("/status/type").and_then(Value::as_str))
            .unwrap_or("")
            .to_owned();
        thread["busy"] = json!(matches!(
            status.as_str(),
            "active" | "running" | "busy" | "retry"
        ));
        Ok(thread)
    }
    pub async fn prepare(&self, state: &GatewayState, controller: bool) -> Result<(), String> {
        // Match the native settings contract: project profiles apply to Codex;
        // OpenCode keeps its inherited server environment.
        if self.codex(state)?.is_none() {
            if controller {
                let value = state
                    .opencode
                    .router_request(Oc::Read(&self.id), &self.cwd)
                    .await?;
                if !disabled(&value["permission"]) {
                    let value = state
                        .opencode
                        .router_request(Oc::Restrict(&self.id), &self.cwd)
                        .await?;
                    if !disabled(&value["permission"]) {
                        return Err("OpenCode did not confirm the Router tool restriction".into());
                    }
                }
            }
            return Ok(());
        }
        let path = state.environment_path.clone();
        let lock = state.environment_lock.clone();
        let root = self.cwd.clone();
        let (configured, environment) = tokio::task::spawn_blocking(move || {
            let _guard = lock.lock().map_err(|_| "Environment lock unavailable")?;
            let profile = crate::environment_config::read_public(&path, &root)?;
            let environment = if profile.configured {
                crate::environment_config::app_server_environment(&path, &root)?
            } else {
                Default::default()
            };
            Ok::<_, String>((profile.configured, environment))
        })
        .await
        .map_err(|_| "Environment worker stopped")??;
        if let Some(server) = self.codex(state)? {
            let mut params = json!({"threadId":self.id,"excludeTurns":true});
            let update_environment = configured
                || state.router_runtime.environment_applied(
                    &self.backend,
                    server.current_generation(),
                    &self.id,
                )?;
            if update_environment {
                state.router_runtime.note_environment(
                    &self.backend,
                    server.current_generation(),
                    &self.id,
                    true,
                )?;
                params["config"] =
                    json!({"shell_environment_policy":{"inherit":"all","set":environment}});
            }
            if controller {
                params["approvalPolicy"] = json!("never");
                params["sandbox"] = json!("read-only");
                // The controller emits a routing decision under a strict JSON
                // schema; source-conversation Map instructions must not leak in.
                params["developerInstructions"] = Value::Null;
                params["dynamicTools"] = json!([]);
            }
            if controller {
                crate::native_router_map::configure(server, params).await?;
            } else {
                server.request_router("thread/resume", params).await?;
            }
            if update_environment {
                state.router_runtime.note_environment(
                    &self.backend,
                    server.current_generation(),
                    &self.id,
                    configured,
                )?;
            }
        }
        Ok(())
    }
    async fn messages(&self, state: &GatewayState) -> Result<Value, String> {
        // Images can dominate otherwise small history pages. Reduce only the
        // read window on a local byte-budget error; mutations are never retried.
        for limit in [100, 16, 4, 2] {
            match state
                .opencode
                .router_request(Oc::Messages(&self.id, limit), &self.cwd)
                .await
            {
                Err(error)
                    if error == "OpenCode Router response exceeds its byte budget" && limit > 2 =>
                {
                    continue
                }
                result => return result,
            }
        }
        Err("OpenCode Router history exceeds its byte budget".into())
    }
    pub async fn start(&self, state: &GatewayState, mut params: Value) -> Result<String, String> {
        if let Some(server) = self.codex(state)? {
            params["threadId"] = self.id.clone().into();
            let value = server.request_router("turn/start", params).await?;
            return value.pointer("/turn/id").and_then(Value::as_str).filter(|id|!id.is_empty()&&id.len()<=256).map(str::to_owned).ok_or_else(||"Backend accepted the turn without a usable ID; inspect the session before retrying".into());
        }
        let before = self.messages(state).await?;
        let known = before
            .as_array()
            .ok_or("Invalid OpenCode message page")?
            .iter()
            .filter_map(|value| {
                value
                    .pointer("/info/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect::<HashSet<_>>();
        if let Some(body) = command::body(&params)? {
            let result = state
                .opencode
                .router_request(Oc::Command(&self.id, &body), &self.cwd)
                .await
                .map_err(|error| {
                    format!(
                        "{error}. The command was not resent; inspect the target before retrying."
                    )
                })?;
            return command::identity(&result, &self.id, &known);
        }
        let input = params["input"].as_array().ok_or("Missing routed prompt")?;
        let parts = input
            .iter()
            .map(crate::native_router::image_input::opencode_part)
            .collect::<Result<Vec<_>, _>>()?;
        let mut body = json!({"parts":parts});
        if let Some(model) = params["model"].as_str().filter(|value| !value.is_empty()) {
            let (provider, model) = model
                .split_once('/')
                .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
                .ok_or("Choose an OpenCode provider/model before routing")?;
            body["model"] = json!({"providerID":provider,"modelID":model});
        }
        if params["developerInstructions"].is_string() {
            body["system"] = params["developerInstructions"].take();
        }
        if params["outputSchema"].is_object() {
            body["format"] =
                json!({"type":"json_schema","schema":params["outputSchema"].take(),"retryCount":2});
        }
        state
            .opencode
            .router_request(Oc::Prompt(&self.id, &body), &self.cwd)
            .await?;
        for _ in 0..10 {
            let messages = self.messages(state).await?;
            let matched = messages
                .as_array()
                .ok_or("Invalid OpenCode message page")?
                .iter()
                .rev()
                .find(|message| {
                    let info = &message["info"];
                    info["role"] == "user"
                        && info["id"].as_str().is_some_and(|id| !known.contains(id))
                        && message["parts"].as_array().is_some_and(|echo| {
                            echo.len() == parts.len()
                                && echo.iter().zip(&parts).all(|(a, b)| {
                                    crate::native_router::image_input::same_part(a, b)
                                })
                        })
                });
            if let Some(id) =
                matched.and_then(|message| message.pointer("/info/id").and_then(Value::as_str))
            {
                return Ok(id.into());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("OpenCode accepted the request but its identity is unknown; inspect the session before retrying".into())
    }
    pub async fn turn(&self, state: &GatewayState, id: &str) -> Result<Turn, String> {
        if let Some(server) = self.codex(state)? {
            let result = read(
                server,
                "thread/turns/list",
                json!({"threadId":self.id,"limit":4,"sortDirection":"desc","itemsView":"full"}),
            )
            .await?;
            let turn = result["data"]
                .as_array()
                .ok_or("Invalid turn status page")?
                .iter()
                .find(|turn| turn["id"] == id)
                .ok_or(
                    "The routed turn is outside the status window; inspect its source conversation",
                )?;
            let status = field(turn, "status");
            let mut text = String::new();
            let mut map_update = None;
            let mut items = Vec::new();
            for item in turn["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| item["type"] == "agentMessage" && item["phase"] != "commentary")
            {
                if !text.is_empty() {
                    text.push('\n');
                }
                map_update = crate::session_map_protocol::parse(field(item, "text"));
                let visible = crate::session_map_protocol::visible(field(item, "text"));
                text.push_str(visible);
                source(&mut items, field(item, "id"), visible);
                if text.len() > 1024 * 1024 {
                    return Err("Routed response exceeds its 1 MiB processing budget; open the source session".into());
                }
            }
            return Ok(Turn {
                map_update,
                items,
                done: matches!(status, "completed" | "failed" | "interrupted"),
                success: status == "completed",
                text,
                error: turn
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
            });
        }
        let messages = self.messages(state).await?;
        let messages = messages.as_array().ok_or("Invalid OpenCode message page")?;
        if !messages
            .iter()
            .any(|message| message.pointer("/info/id").and_then(Value::as_str) == Some(id))
        {
            return Err(
                "The routed message is outside the status window; open the source session".into(),
            );
        }
        let statuses = state.opencode.router_request(Oc::Status, &self.cwd).await?;
        let done = !matches!(field(&statuses[&self.id], "type"), "busy" | "retry");
        let mut text = String::new();
        let mut structured = None;
        let mut items = Vec::new();
        let mut structured_items = Vec::new();
        let mut error = String::new();
        let mut finalized = false;
        for message in messages
            .iter()
            .filter(|message| message.pointer("/info/parentID").and_then(Value::as_str) == Some(id))
        {
            let info = &message["info"];
            finalized |= info
                .pointer("/time/completed")
                .is_some_and(|value| !value.is_null())
                || info.get("error").is_some_and(|value| !value.is_null());
            if let Some(value) = info.get("structured").filter(|value| !value.is_null()) {
                let body = value.to_string();
                source(
                    &mut structured_items,
                    &format!("{}-structured", field(info, "id")),
                    &body,
                );
                structured = Some(body);
            }
            if let Some(value) = info.get("error").filter(|value| !value.is_null()) {
                error = value.to_string();
            }
            for part in message["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|part| part["type"] == "text")
            {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(field(part, "text"));
                source(&mut items, field(part, "id"), field(part, "text"));
                if text.len() > 1024 * 1024 {
                    return Err("Routed response exceeds its 1 MiB processing budget; open the source session".into());
                }
            }
        }
        Ok(Turn {
            map_update: None,
            items: if structured.is_some() {
                structured_items
            } else {
                items
            },
            done: done && finalized,
            success: error.is_empty(),
            text: structured.unwrap_or(text),
            error,
        })
    }
    pub async fn stop(&self, state: &GatewayState, id: &str) -> Result<(), String> {
        if let Some(server) = self.codex(state)? {
            server
                .request_router("turn/interrupt", json!({"threadId":self.id,"turnId":id}))
                .await?;
        } else {
            let messages = self.messages(state).await?;
            let latest = messages
                .as_array()
                .into_iter()
                .flatten()
                .rev()
                .find(|message| {
                    message.pointer("/info/role").and_then(Value::as_str) == Some("user")
                })
                .and_then(|message| message.pointer("/info/id").and_then(Value::as_str));
            if latest != Some(id) {
                return Err(
                    "OpenCode has a different active message; Studio will not stop it".into(),
                );
            }
            state
                .opencode
                .router_request(Oc::Abort(&self.id), &self.cwd)
                .await?;
        }
        Ok(())
    }
}
use crate::opencode_permissions::tools_disabled as disabled;

// Keep original message identities while bounding the duplicated preview payload.
pub(crate) fn source(items: &mut Vec<ResponseItem>, id: &str, text: &str) {
    if id.is_empty() || id.len() > 256 || items.len() >= 128 {
        return;
    }
    let remaining = (32usize * 1024).saturating_sub(items.iter().map(|item| item.text.len()).sum());
    if remaining == 0 {
        return;
    }
    let mut end = remaining.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    items.push(ResponseItem {
        id: id.into(),
        text: text[..end].into(),
        excerpt: end < text.len(),
    });
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn preview_budget_keeps_real_ids_and_utf8_without_oversized_identity() {
        let mut items = Vec::new();
        source(&mut items, "answer", "中文😀".repeat(10_000).as_str());
        assert_eq!(items[0].id, "answer");
        assert!(items[0].excerpt);
        assert!(items[0].text.len() <= 32 * 1024);
        source(&mut items, "second", "end");
        assert!(items.iter().map(|i| i.text.len()).sum::<usize>() <= 32 * 1024);
        let before = items.len();
        source(&mut items, &"x".repeat(257), "ignored");
        assert_eq!(items.len(), before);
    }
}
