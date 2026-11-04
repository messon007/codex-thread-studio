//! Bounded isolated utility tasks. No request is sent to the source conversation.
use crate::{
    gateway::Api,
    model::Backend,
    translation_text::{self as text, Content},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};
pub fn next_ticket() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub backend: Backend,
    pub socket: u64,
    pub thread: String,
    pub cwd: String,
    pub model: String,
    pub ready: bool,
}
#[derive(Clone, Debug)]
pub struct Profile {
    pub source: Arc<str>,
    pub engine: String,
    pub backend: String,
    pub model: String,
    pub effort: String,
}
#[derive(Clone, Debug)]
pub struct ResultValue {
    pub content: Arc<Content>,
    pub warning: Option<String>,
    pub cached: bool,
}
#[derive(Debug)]
pub enum Update {
    Profile(Profile),
    Done(Result<ResultValue, String>),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    engine: String,
    backend: String,
    socket: u64,
    cwd: String,
    model: String,
    effort: String,
    source: String,
}
struct Cached {
    content: Arc<Content>,
    bytes: usize,
}
impl crate::byte_cache::Weight for Cached {
    fn weight(&self) -> usize {
        self.bytes
    }
}
type Cache = crate::byte_cache::ByteCache<Key, Cached>;
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache::new(
            std::num::NonZeroUsize::new(64).unwrap(),
            4 * 1024 * 1024,
        ))
    })
}
fn cancelled(flag: &AtomicBool) -> Result<(), String> {
    if flag.load(Ordering::Acquire) {
        Err("Utility task cancelled".into())
    } else {
        Ok(())
    }
}
/// Await a local HTTP utility result without retaining a cancelled UI task.
/// Provider mutations use their receipt/ACK cleanup paths instead.
pub async fn cancellable<T>(
    request: impl std::future::Future<Output = Result<T, String>>,
    stop: &AtomicBool,
) -> Result<T, String> {
    cancelled(stop)?;
    tokio::pin!(request);
    loop {
        tokio::select! {
            value=&mut request=>{cancelled(stop)?;return value;},
            _=tokio::time::sleep(Duration::from_millis(50))=>cancelled(stop)?,
        }
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
#[derive(Default)]
struct Output {
    limit: usize,
    thread: String,
    turn: String,
    items: Vec<(String, String)>,
    done: Option<Result<(), String>>,
}
impl Output {
    fn observe(&mut self, value: &Value) -> Result<(), String> {
        let p = &value["params"];
        if p["threadId"].as_str() != Some(self.thread.as_str()) || self.thread.is_empty() {
            return Ok(());
        }
        let method = value["method"].as_str().unwrap_or("");
        if let Some(turn) = p.get("turnId") {
            let turn = turn
                .as_str()
                .filter(|id| valid_id(id))
                .ok_or("Invalid utility task turn identity")?;
            if !self.turn.is_empty() && self.turn != turn {
                return Ok(());
            }
        }
        match method {
            "turn/started" => {
                if self.turn.is_empty() {
                    self.turn = p
                        .pointer("/turn/id")
                        .and_then(Value::as_str)
                        .filter(|id| valid_id(id))
                        .ok_or("Invalid utility task turn identity")?
                        .into();
                }
            }
            "item/agentMessage/delta" => {
                let id = p["itemId"]
                    .as_str()
                    .ok_or("Utility task item has no identity")?;
                let delta = p["delta"].as_str().ok_or("Invalid utility text delta")?;
                if let Some((_, value)) = self.items.iter_mut().find(|(key, _)| key == id) {
                    value.push_str(delta);
                } else {
                    self.items.push((id.into(), delta.into()));
                }
            }
            "item/completed" => {
                if p["item"]["type"] == "agentMessage" {
                    let id = p["item"]["id"]
                        .as_str()
                        .ok_or("Utility task item has no identity")?;
                    let content = p["item"]["text"].as_str().ok_or("Invalid utility text")?;
                    if let Some((_, value)) = self.items.iter_mut().find(|(key, _)| key == id) {
                        *value = content.into();
                    } else {
                        self.items.push((id.into(), content.into()));
                    }
                }
            }
            "turn/completed" => {
                let turn = &p["turn"];
                let id = turn["id"]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or("Invalid utility task completion identity")?;
                if !self.turn.is_empty() && turn["id"].as_str() != Some(self.turn.as_str()) {
                    return Ok(());
                }
                self.turn = id.into();
                for item in turn["items"].as_array().into_iter().flatten() {
                    if item["type"] == "agentMessage" {
                        if let (Some(id), Some(value)) =
                            (item["id"].as_str(), item["text"].as_str())
                        {
                            if let Some((_, text)) =
                                self.items.iter_mut().find(|(key, _)| key == id)
                            {
                                *text = value.into();
                            } else {
                                self.items.push((id.into(), value.into()));
                            }
                        }
                    }
                }
                self.done = Some(
                    if turn["status"] == "completed" && turn["error"].is_null() {
                        Ok(())
                    } else {
                        Err(turn
                            .pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("Utility task failed or was interrupted")
                            .into())
                    },
                );
            }
            "error" => {
                self.done = Some(Err(p
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Utility task failed")
                    .into()));
            }
            _ => {}
        }
        if self.items.len() > 64
            || self
                .items
                .iter()
                .map(|(_, value)| value.len())
                .sum::<usize>()
                > if self.limit == 0 {
                    text::MAX_RESULT
                } else {
                    self.limit
                }
        {
            Err("Utility task output exceeds its budget".into())
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    fn result(&self) -> Option<Result<Content, String>> {
        self.raw_result()
            .map(|result| result.and_then(|value| text::parse(&value)))
    }
    fn raw_result(&self) -> Option<Result<String, String>> {
        self.done.as_ref().map(|done| {
            done.clone().map(|()| {
                self.items
                    .iter()
                    .map(|(_, value)| value.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        })
    }
}
struct Socket {
    wire: WebSocketStream<MaybeTlsStream<TcpStream>>,
    nonce: String,
    sequence: u64,
    generation: Option<u64>,
    output: Output,
    early: VecDeque<Value>,
    early_bytes: usize,
}
impl Socket {
    async fn connect(api: &Api, context: &Context, ticket: u64) -> Result<Self, String> {
        let request = api.socket_request(&context.backend.socket_path)?;
        let config = WebSocketConfig::default().max_message_size(Some(32 * 1024 * 1024));
        let (wire, _) = tokio::time::timeout(
            Duration::from_secs(15),
            connect_async_with_config(request, Some(config), false),
        )
        .await
        .map_err(|_| "Utility task backend connection timed out")?
        .map_err(|e| e.to_string())?;
        let mut socket = Self {
            wire,
            nonce: format!("gnome-utility-{}-{ticket}", std::process::id()),
            sequence: 0,
            generation: None,
            output: Output::default(),
            early: VecDeque::new(),
            early_bytes: 0,
        };
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let value = socket.receive().await?;
                if value["method"] == "studio/appServer/status"
                    && value["params"]["state"] == "ready"
                {
                    socket.generation = value["params"]["generation"].as_u64();
                    return Ok::<_, String>(());
                }
            }
        })
        .await
        .map_err(|_| "Utility task backend did not become ready")??;
        Ok(socket)
    }
    async fn receive(&mut self) -> Result<Value, String> {
        loop {
            let frame = self
                .wire
                .next()
                .await
                .ok_or("Utility task backend disconnected")?
                .map_err(|e| e.to_string())?;
            match frame {
                Message::Text(bytes) => {
                    let value = crate::worker::compute(move || {
                        serde_json::from_slice::<Value>(bytes.as_bytes())
                            .map_err(|_| "Invalid utility backend response".into())
                    })
                    .await?;
                    if value["method"] == "studio/appServer/status" {
                        if self.generation.is_some()
                            && value["params"]["generation"].as_u64() != self.generation
                        {
                            return Err("Utility task stopped because the backend restarted".into());
                        }
                        if matches!(value["params"]["state"].as_str(), Some("error" | "stopped")) {
                            return Err("Utility task backend stopped".into());
                        }
                    }
                    return Ok(value);
                }
                Message::Ping(data) => self
                    .wire
                    .send(Message::Pong(data))
                    .await
                    .map_err(|e| e.to_string())?,
                Message::Close(_) => return Err("Utility task backend disconnected".into()),
                _ => {}
            }
        }
    }
    fn observe(&mut self, value: Value) -> Result<(), String> {
        if self.output.thread.is_empty() {
            if matches!(
                value["method"].as_str(),
                Some(
                    "item/agentMessage/delta"
                        | "item/completed"
                        | "turn/started"
                        | "turn/completed"
                )
            ) {
                self.early_bytes = self.early_bytes.saturating_add(value.to_string().len());
                if self.early.len() >= 256 || self.early_bytes > 2 * 1024 * 1024 {
                    return Err("Utility task prelude exceeds its budget".into());
                }
                self.early.push_back(value);
            }
            Ok(())
        } else {
            self.output.observe(&value)
        }
    }
    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.sequence += 1;
        let id = format!("{}-{}", self.nonce, self.sequence);
        self.wire
            .send(Message::Text(
                json!({"id":id,"method":method,"params":params})
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let mut value = self.receive().await?;
                if value["id"].as_str() == Some(id.as_str()) {
                    if let Some(error) = value.get("error") {
                        return Err(error["message"]
                            .as_str()
                            .unwrap_or("Utility task backend rejected the request")
                            .into());
                    }
                    return Ok(value
                        .get_mut("result")
                        .map(Value::take)
                        .unwrap_or(Value::Null));
                }
                self.observe(value)?;
            }
        })
        .await
        .map_err(|_| "Utility task request timed out; it was not resent")?
    }
    async fn codex(
        &mut self,
        context: &Context,
        profile: &Profile,
        stop: &AtomicBool,
    ) -> Result<ResultValue, String> {
        let result = self
            .structured(
                context,
                &StructuredTask {
                    instructions: text::INSTRUCTIONS.into(),
                    input: text::input(&profile.source),
                    schema: text::schema(),
                    model: profile.model.clone(),
                    effort: profile.effort.clone(),
                    service_tier: String::new(),
                    max_result: text::MAX_RESULT,
                    read_completed_thread: false,
                },
                stop,
            )
            .await?;
        let content = crate::worker::compute(move || text::parse(&result.text))
            .await
            .map_err(|error| match &result.warning {
                Some(warning) => format!("{error}; {warning}"),
                None => error,
            })?;
        Ok(ResultValue {
            content: Arc::new(content),
            warning: result.warning,
            cached: false,
        })
    }
    async fn structured(
        &mut self,
        context: &Context,
        task: &StructuredTask,
        stop: &AtomicBool,
    ) -> Result<StructuredResult, String> {
        self.output.limit = task.max_result;
        cancelled(stop)?;
        let mut params = json!({"cwd":context.cwd,"ephemeral":true,"approvalPolicy":"never","sandbox":"read-only","developerInstructions":task.instructions});
        if !task.model.is_empty() {
            params["model"] = task.model.clone().into();
        }
        let started = self.rpc("thread/start", params).await?;
        let id = started
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or("The backend did not create a utility task")?
            .to_owned();
        if !valid_id(&id) || id == context.thread {
            return Err("The backend did not return an isolated utility identity".into());
        }
        self.output.thread = id.clone();
        let result = async {
            while let Some(value) = self.early.pop_front() {
                self.output.observe(&value)?;
            }
            self.early_bytes = 0;
            cancelled(stop)?;
            let mut params = json!({
                "threadId":id,"cwd":context.cwd,
                "input":[{"type":"text","text":task.input}],"outputSchema":task.schema
            });
            if !task.model.is_empty() {
                params["model"] = task.model.clone().into();
            }
            if !task.effort.is_empty() {
                params["effort"] = task.effort.clone().into();
            }
            if !task.service_tier.is_empty() {
                params["serviceTier"] = task.service_tier.clone().into();
            }
            let turn = self.rpc("turn/start", params).await?;
            let turn_id = turn
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .filter(|id| valid_id(id))
                .ok_or("The backend accepted a utility task without a turn identity")?;
            if !self.output.turn.is_empty() && self.output.turn != turn_id {
                return Err("Utility task turn identity changed".into());
            }
            self.output.turn = turn_id.into();
            let text = tokio::time::timeout(Duration::from_secs(150), async {
                loop {
                    cancelled(stop)?;
                    if let Some(result) = self.output.raw_result() {
                        return result;
                    }
                    tokio::select! {
                        value = self.receive() => { let value = value?; self.observe(value)?; },
                        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                    }
                }
            })
            .await
            .map_err(|_| "Utility task timed out")??;
            if task.read_completed_thread {
                cancelled(stop)?;
                let value = self
                    .rpc("thread/read", json!({"threadId":id,"includeTurns":true}))
                    .await?;
                let thread = id.clone();
                let turn = self.output.turn.clone();
                let limit = task.max_result;
                let result =
                    crate::worker::compute(move || completed_output(value, &thread, &turn, limit))
                        .await?;
                cancelled(stop)?;
                Ok(result)
            } else {
                Ok(text)
            }
        }
        .await;
        // The owned ephemeral identity is never the source conversation. Mutations
        // are single-attempt, including cancellation after an ambiguous turn reply.
        if self.output.done.is_none()
            && (result.is_err() || stop.load(Ordering::Acquire))
            && !self.output.turn.is_empty()
        {
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                self.rpc(
                    "turn/interrupt",
                    json!({"threadId":id,"turnId":self.output.turn}),
                ),
            )
            .await;
        }
        let cleanup = tokio::time::timeout(
            Duration::from_secs(10),
            self.rpc("thread/delete", json!({"threadId":id})),
        )
        .await;
        let warning = match cleanup {
            Ok(Ok(_)) => None,
            _ => Some("Utility task cleanup was not confirmed".into()),
        };
        let _ = self.wire.close(None).await;
        match result {
            Ok(text) => Ok(StructuredResult { text, warning }),
            Err(error) => Err(match warning {
                Some(warning) => format!("{error}; {warning}"),
                None => error,
            }),
        }
    }
}
/// An isolated, bounded structured task shared by native utility workflows.
pub struct StructuredTask {
    pub instructions: String,
    pub input: String,
    pub schema: Value,
    pub model: String,
    pub effort: String,
    pub service_tier: String,
    pub max_result: usize,
    /// Require authoritative final output, rather than possibly incomplete event text.
    /// Used by decisions which may authorize another source-conversation turn.
    pub read_completed_thread: bool,
}

fn completed_output(
    value: Value,
    thread: &str,
    turn: &str,
    limit: usize,
) -> Result<String, String> {
    if value.pointer("/thread/id").and_then(Value::as_str) != Some(thread) {
        return Err("Utility result belongs to a different conversation".into());
    }
    let latest = value
        .pointer("/thread/turns")
        .and_then(Value::as_array)
        .and_then(|turns| turns.last())
        .ok_or("Utility result has no completed turn")?;
    if latest["id"].as_str() != Some(turn)
        || latest["status"] != "completed"
        || !latest["error"].is_null()
    {
        return Err("Utility result is no longer the identified completed turn".into());
    }
    let text = latest["items"]
        .as_array()
        .ok_or("Utility result has no item array")?
        .iter()
        .rev()
        .find(|item| item["type"] == "agentMessage")
        .and_then(|item| item["text"].as_str())
        .filter(|text| !text.is_empty())
        .ok_or("Utility result has no final assistant message")?;
    if text.len() > limit {
        return Err("Utility task output exceeds its budget".into());
    }
    Ok(text.into())
}
pub struct StructuredResult {
    pub text: String,
    pub warning: Option<String>,
}
pub async fn structured(
    api: &Api,
    ticket: u64,
    context: &Context,
    task: StructuredTask,
    stop: &AtomicBool,
) -> Result<StructuredResult, String> {
    if context.backend.kind != "codex" || !context.ready {
        return Err("A ready Codex-compatible backend is required for this AI task".into());
    }
    if task.max_result == 0 || task.max_result > 512 * 1024 || task.input.len() > 1024 * 1024 {
        return Err("Structured task exceeds its budget".into());
    }
    let mut socket = Socket::connect(api, context, ticket).await?;
    socket.structured(context, &task, stop).await
}
pub async fn execute(
    api: Api,
    ticket: u64,
    context: Context,
    selection: String,
    stop: Arc<AtomicBool>,
    events: async_channel::Sender<crate::protocol::Event>,
) -> Result<ResultValue, String> {
    cancelled(&stop)?;
    let source = crate::worker::compute(move || text::source(&selection)).await?;
    let settings = crate::translation_settings::load(&api).await?.values;
    let local = settings.engine == "ollama";
    if !local && !context.ready {
        return Err("The current backend is not ready for translation".into());
    }
    let profile = Profile {
        source: Arc::from(source.as_str()),
        engine: settings.engine.clone(),
        backend: if local {
            "ollama".into()
        } else {
            context.backend.name.clone()
        },
        model: if local {
            settings.ollama_model
        } else {
            settings
                .models
                .get(&context.backend.id)
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| context.model.clone())
        },
        effort: if local || context.backend.kind != "codex" {
            String::new()
        } else {
            settings
                .efforts
                .get(&context.backend.id)
                .cloned()
                .unwrap_or_else(|| {
                    if context.backend.kind == "codex" {
                        "low".into()
                    } else {
                        String::new()
                    }
                })
        },
    };
    let key = Key {
        engine: profile.engine.clone(),
        backend: if local {
            "ollama".into()
        } else {
            context.backend.id.clone()
        },
        socket: if local { 0 } else { context.socket },
        cwd: if local {
            String::new()
        } else {
            context.cwd.clone()
        },
        model: profile.model.clone(),
        effort: profile.effort.clone(),
        source,
    };
    let _ = events
        .send(crate::protocol::Event::Translation {
            ticket,
            update: Update::Profile(profile.clone()),
        })
        .await;
    let hit = cache()
        .lock()
        .map_err(|_| "Translation cache unavailable")?
        .update(&key, |v| v.content.clone());
    if let Some(content) = hit {
        cancelled(&stop)?;
        return Ok(ResultValue {
            content,
            warning: None,
            cached: true,
        });
    }
    cancelled(&stop)?;
    let result = if local {
        let body = json!({"model":profile.model,"text":profile.source});
        let value = cancellable(api.command("/studio/ollama/translate", &body), &stop).await?;
        let content = crate::worker::compute(move || text::parse(&value.to_string())).await?;
        ResultValue {
            content: Arc::new(content),
            warning: None,
            cached: false,
        }
    } else if context.backend.kind == "codex" {
        let mut socket = Socket::connect(&api, &context, ticket).await?;
        socket.codex(&context, &profile, &stop).await?
    } else if context.backend.kind == "opencode" {
        crate::opencode::utility_translation(&api, ticket, &context, &profile, &stop).await?
    } else {
        return Err("This backend does not support translation".into());
    };
    cancelled(&stop)?;
    if result.warning.is_none() {
        let bytes = key.source.capacity()
            + key.cwd.capacity()
            + key.backend.capacity()
            + key.engine.capacity()
            + key.model.capacity()
            + key.effort.capacity()
            + result.content.translation.capacity()
            + result.content.source_pronunciation.capacity()
            + result.content.translation_pronunciation.capacity()
            + 512;
        cache()
            .lock()
            .map_err(|_| "Translation cache unavailable")?
            .insert(
                key,
                Cached {
                    content: result.content.clone(),
                    bytes,
                },
            );
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_before_ack_is_correlated_and_unrelated_messages_ignored() {
        let mut output = Output {
            thread: "utility".into(),
            ..Default::default()
        };
        output.observe(&json!({"method":"item/completed","params":{"threadId":"source","item":{"id":"x","type":"agentMessage","text":"wrong"}}})).unwrap();
        assert!(output.items.is_empty());
        output.observe(&json!({"method":"item/completed","params":{"threadId":"utility","item":{"id":"owned","type":"agentMessage","text":"{\"translation\":\"你好\"}"}}})).unwrap();
        output.observe(&json!({"method":"turn/completed","params":{"threadId":"utility","turn":{"id":"early","status":"completed"}}})).unwrap();
        assert_eq!(output.result().unwrap().unwrap().translation, "你好");
    }
    #[test]
    fn utility_output_is_bounded_and_failures_are_terminal() {
        let mut output = Output {
            thread: "utility".into(),
            ..Default::default()
        };
        assert!(output.observe(&json!({"method":"item/agentMessage/delta","params":{"threadId":"utility","itemId":"a","delta":"x".repeat(text::MAX_RESULT+1)}})).is_err());
        let mut output = Output {
            thread: "utility".into(),
            ..Default::default()
        };
        output.observe(&json!({"method":"turn/completed","params":{"threadId":"utility","turn":{"id":"t","status":"failed","error":{"message":"denied"}}}})).unwrap();
        assert_eq!(output.result().unwrap().unwrap_err(), "denied");
    }
    #[test]
    fn structured_tasks_have_an_independent_output_budget_and_literal_json_result() {
        let mut output = Output {
            thread: "map-task".into(),
            limit: 128,
            ..Default::default()
        };
        let value = r#"{"goal":"<literal> 中文😀"}"#;
        output.observe(&json!({"method":"item/completed","params":{"threadId":"map-task","item":{"id":"answer","type":"agentMessage","text":value}}})).unwrap();
        output.observe(&json!({"method":"turn/completed","params":{"threadId":"map-task","turn":{"id":"turn","status":"completed"}}})).unwrap();
        assert_eq!(output.raw_result().unwrap().unwrap(), value);
        assert!(output.observe(&json!({"method":"item/agentMessage/delta","params":{"threadId":"map-task","itemId":"answer","delta":"x".repeat(129)}})).is_err());
    }
}

#[cfg(test)]
#[path = "translation_utility_tests.rs"]
mod utility_tests;
