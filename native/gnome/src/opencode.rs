//! OpenCode HTTP/SSE adapter into the shared native conversation presentation.
//! Only server status events/status snapshots determine completion. Never send
//! a client-created OpenCode message ID or infer idle from an assistant message.
use crate::{
    gateway::Api,
    model::{text, Backend},
    protocol::{Command, Event},
};
use futures_util::StreamExt;
use reqwest::Method;
use serde_json::{json, Value};
use sse_stream::SseByteStream;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::mpsc;
#[path = "opencode_requests.rs"]
mod requests;

// A single allowed 10 MiB image file part can occupy 13.4 MiB as base64.
// Release raw events after worker projection; never retain URLs in user rows.
const SSE_LIMIT: usize = 16 * 1024 * 1024;
#[derive(Default)]
struct Projection {
    requests: requests::Book,
    users: UserCache,
    sequence: u64,
    directories: HashMap<String, String>,
    roles: HashMap<String, String>,
    message_turns: HashMap<String, String>,
    latest_turns: HashMap<String, String>,
    parts: HashSet<String>,
    status: HashMap<String, (u64, Value)>,
    changed: HashMap<String, u64>,
}
type Shared = Arc<Mutex<Projection>>;

#[derive(Default)]
struct UserParts {
    images: Vec<(String, crate::conversation_images::Image)>,
    parts: Vec<(String, String)>,
    truncated: bool,
}
impl crate::byte_cache::Weight for UserParts {
    fn weight(&self) -> usize {
        self.images
            .iter()
            .map(|(id, image)| id.capacity() + image.weight())
            .sum::<usize>()
            + self.parts.capacity() * std::mem::size_of::<(String, String)>()
            + self
                .parts
                .iter()
                .map(|(id, text)| id.capacity() + text.capacity())
                .sum::<usize>()
    }
}
impl UserParts {
    fn set(&mut self, id: &str, text: &str, append: bool) {
        if id.is_empty() || id.len() > 256 {
            return;
        }
        let index = self
            .parts
            .iter()
            .position(|(part, _)| part == id)
            .unwrap_or_else(|| {
                self.parts.push((id.into(), String::new()));
                self.parts.len() - 1
            });
        if !append {
            self.parts[index].1.clear();
        }
        let mut end = text.len().min(128 * 1024);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.parts[index].1.push_str(&text[..end]);
        self.truncated |= end < text.len();
        let mut left = 128usize * 1024;
        let mut retained = 0;
        for (_, text) in self.parts.iter_mut().take(400) {
            let mut end = text.len().min(left);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            if end < text.len() {
                text.truncate(end);
                text.shrink_to_fit();
                self.truncated = true;
            }
            left = left.saturating_sub(end + 1);
            retained += 1;
            if left == 0 {
                break;
            }
        }
        if retained < self.parts.len() {
            self.parts.truncate(retained);
            self.truncated = true;
        }
    }
    fn file(&mut self, part: &Value, id: &str) {
        self.set(id, "[Attached image/file]", false);
        self.images.retain(|(key, _)| key != id);
        if self.images.len() < 16 {
            if let Some(image) = crate::conversation_images::Image::from_part(part) {
                self.images.push((id.into(), image));
            }
        }
    }
    fn item(&self, id: &str) -> Value {
        let mut text = self
            .parts
            .iter()
            .filter(|(id, _)| !self.images.iter().any(|(image, _)| image == id))
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if self.truncated {
            text.push_str("\n… Output truncated in the native view.");
        }
        let mut content = if text.is_empty() {
            Vec::new()
        } else {
            vec![json!({"type":"text","text":text})]
        };
        content.extend(self.images.iter().map(|(_, image)| image.part()));
        let mut item = json!({"id":id,"turnId":id,"type":"userMessage","content":content});
        if self.truncated {
            item["_studioTruncated"] = true.into();
        }
        item
    }
}
struct UserCache(crate::byte_cache::ByteCache<String, UserParts>);
impl Default for UserCache {
    fn default() -> Self {
        Self(crate::byte_cache::ByteCache::new(
            std::num::NonZeroUsize::new(64).unwrap(),
            8 * 1024 * 1024,
        ))
    }
}
impl UserCache {
    fn replace(&mut self, id: &str, parts: &Value) -> Value {
        let mut user = UserParts::default();
        for (index, part) in parts.as_array().into_iter().flatten().enumerate() {
            let fallback = format!("part-{index}");
            let part_id = part["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .unwrap_or(&fallback);
            if part["type"] == "file" {
                user.file(part, part_id);
            } else if part["type"] == "text" {
                user.set(part_id, text(part, "text"), false);
            }
        }
        let item = user.item(id);
        self.0.insert(id.into(), user);
        item
    }
    fn file(&mut self, id: &str, part: &Value) -> Value {
        let mut user = self.0.remove(id).unwrap_or_default();
        user.file(part, text(part, "id"));
        let item = user.item(id);
        self.0.insert(id.into(), user);
        item
    }
    fn update(&mut self, id: &str, part: &str, text: &str, append: bool) -> Value {
        let mut user = self.0.remove(id).unwrap_or_default();
        if !append {
            user.images.retain(|(id, _)| id != part);
        }
        user.set(part, text, append);
        let item = user.item(id);
        self.0.insert(id.into(), user);
        item
    }
}

fn segment(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}
fn endpoint(path: &str, directory: &str, query: &[(&str, String)]) -> String {
    let mut encoded = url::form_urlencoded::Serializer::new(String::new());
    if !directory.is_empty() {
        encoded.append_pair("directory", directory);
    }
    for (key, value) in query {
        encoded.append_pair(key, value);
    }
    let query = encoded.finish();
    format!(
        "/opencode{path}{}{}",
        if query.is_empty() { "" } else { "?" },
        query
    )
}

async fn emit(
    events: &async_channel::Sender<Event>,
    backend: &str,
    generation: u64,
    mut payload: Value,
) -> Result<(), String> {
    let (payload, prepared) = crate::worker::compute(move || {
        let prepared = crate::protocol::prepare_presentation(&mut payload);
        Ok((payload, prepared))
    })
    .await?;
    events
        .send(Event::Wire {
            backend: backend.into(),
            generation,
            payload,
            prepared,
        })
        .await
        .map_err(|error| error.to_string())
}

pub async fn connect(
    api: Api,
    backend: Backend,
    generation: u64,
    mut commands: mpsc::Receiver<Command>,
    events: async_channel::Sender<Event>,
) -> Result<(), String> {
    let info = api.json(Method::GET, "/studio/opencode", None).await?;
    if info["reachable"] == false {
        return Err(info["error"]
            .as_str()
            .unwrap_or("OpenCode is unavailable")
            .into());
    }
    let response = api.opencode_events().await?;
    // Bound bytes fed to the mature SSE parser between dispatched events; its
    // own line/data buffers otherwise have no size limit. UTF-8, CRLF, comments
    // and multiline data remain the library's responsibility.
    let budget = Arc::new(AtomicUsize::new(0));
    let counted = budget.clone();
    let chunks = response.bytes_stream().map(move |chunk| {
        let chunk =
            chunk.map_err(|error| std::io::Error::other(error.without_url().to_string()))?;
        if counted
            .fetch_add(chunk.len(), Ordering::Relaxed)
            .saturating_add(chunk.len())
            > SSE_LIMIT
        {
            return Err(std::io::Error::other(
                "OpenCode event exceeds the native stream limit",
            ));
        }
        Ok(chunk)
    });
    let mut stream = Box::pin(SseByteStream::new(chunks));
    let state = Shared::default();
    let mut jobs = tokio::task::JoinSet::new();
    emit(
        &events,
        &backend.id,
        generation,
        json!({"method":"studio/appServer/status","params":{"state":"ready"}}),
    )
    .await?;
    let mut recoveries = tokio::task::JoinSet::new();
    let mut recovery_queue = VecDeque::<(String, String)>::new();
    let mut recovery_active = HashSet::<String>::new();
    loop {
        while recoveries.len() < 2 {
            let Some((directory, thread)) = recovery_queue.pop_front() else {
                break;
            };
            recovery_active.insert(directory.clone());
            let api = api.clone();
            let state = state.clone();
            recoveries.spawn(async move {
                let result = recover_requests(&api, &state, &directory).await;
                (directory, thread, result)
            });
        }
        tokio::select! {
            command=commands.recv()=>match command {
                Some(Command::Supervision{request,..})=>{let _=events.send(Event::Supervision{backend:backend.id.clone(),generation,update:crate::turn_supervision_host::Update::Rejected{request:request.request(),thread:request.thread().map(str::to_owned),message:"Supervision requires a Codex session".into()}}).await;},
                Some(Command::Rpc{id,method,mut params,root,..})=>{
                    if params.get("cwd").is_none(){if let Some(root)=root.filter(|root|!root.is_empty()){params["cwd"]=root.into();}}
                    if jobs.len()>=8 {emit(&events,&backend.id,generation,json!({"id":id,"error":{"message":"Too many OpenCode requests; try again"}})).await?;continue;}
                    let recover=matches!(method.as_str(),"thread/read"|"thread/resume");
                    let api=api.clone();let state=state.clone();
                    jobs.spawn(async move{let result=rpc(&api,&state,&method,params).await;(id,result,recover)});
                }
                Some(Command::Reply{id,result,..})=>{
                    if jobs.len()>=8{return Err("OpenCode input queue is full; reconnect to resolve pending requests".into());}
                    let api=api.clone();let state=state.clone();
                    jobs.spawn(async move{let outcome=reply(&api,&state,id,result).await;(0,outcome,false)});
                }
                Some(Command::Shutdown)|None=>{jobs.abort_all();recoveries.abort_all();return Ok(());}
                _=>{}
            },
            Some(result)=jobs.join_next(),if !jobs.is_empty()=>{
                let (id,result,recover)=result.map_err(|error|error.to_string())?;
                if id==0 {if let Err(error)=result {return Err(error);}continue;}
                let project=if recover{result.as_ref().ok().and_then(|v|{
                    let directory=v.pointer("/thread/cwd")?.as_str()?;let thread=v.pointer("/thread/id")?.as_str()?;
                    requests::directory(directory).then(||(directory.to_owned(),thread.to_owned()))
                })}else{None};
                let payload=match result {Ok(result)=>json!({"id":id,"result":result}),Err(error)=>json!({"id":id,"error":{"message":error}})};
                emit(&events,&backend.id,generation,payload).await?;
                if let Some((directory,thread))=project{
                    if !recovery_active.contains(&directory)&&!recovery_queue.iter().any(|(d,_)|d==&directory){
                        if recovery_queue.len()==16{recovery_queue.pop_back();}
                        recovery_queue.push_front((directory,thread));
                    }
                }
            },
            Some(result)=recoveries.join_next(),if !recoveries.is_empty()=>{
                let (directory,thread,result)=result.map_err(|error|error.to_string())?;
                recovery_active.remove(&directory);
                // Project on this event loop, immediately before emission. A
                // background task must not queue already-normalized requests
                // that could outlive a subsequently processed SSE resolution.
                let result=match result{
                    Ok(snapshot)=>{
                        let state=state.clone();
                        crate::worker::compute(move||{
                            let mut state=state.lock().map_err(|_|"OpenCode request registry unavailable")?;
                            project_recovery(&mut state,&directory,snapshot)
                        }).await
                    }
                    Err(error)=>Err(error),
                };
                let values=match result{Ok(values)=>values,Err(error)=>vec![json!({"method":"error","params":{"threadId":thread,"error":{"message":format!("OpenCode pending requests could not be recovered: {error}")}}})]};
                for value in values{emit(&events,&backend.id,generation,value).await?;}
            },
            event=stream.next()=>{
                let event=event.ok_or("OpenCode event stream closed; reconnect before sending")?.map_err(|error|error.to_string())?;
                budget.store(0,Ordering::Relaxed);
                let Some(data)=event.data else {continue;};
                let state=state.clone();
                let values=crate::worker::compute(move||{
                    let value:Value=serde_json::from_str(&data).map_err(|_|"OpenCode sent invalid event JSON")?;
                    Ok(normalize_event(&mut state.lock().unwrap(),value))
                }).await?;
                for value in values {emit(&events,&backend.id,generation,value).await?;}
            }
        }
    }
}

struct RecoverySnapshot {
    marker: u64,
    inventories: Vec<(String, Value)>,
}
async fn recover_requests(
    api: &Api,
    state: &Shared,
    directory: &str,
) -> Result<RecoverySnapshot, String> {
    let marker = state
        .lock()
        .map_err(|_| "OpenCode request registry unavailable")?
        .requests
        .snapshot();
    let mut snapshots = Vec::new();
    let mut count = 0;
    for (kind, path) in [("permission", "/permission"), ("question", "/question")] {
        let value = api
            .json(Method::GET, &endpoint(path, directory, &[]), None)
            .await?;
        let rows = value
            .as_array()
            .ok_or("Invalid OpenCode pending request inventory")?;
        count += rows.len();
        if count > 256 {
            return Err("OpenCode pending request inventory exceeds 256 entries".into());
        }
        snapshots.push((kind.to_owned(), value));
    }
    Ok(RecoverySnapshot {
        marker,
        inventories: snapshots,
    })
}
fn project_recovery(
    state: &mut Projection,
    directory: &str,
    snapshot: RecoverySnapshot,
) -> Result<Vec<Value>, String> {
    if !state.requests.snapshot_valid(snapshot.marker) {
        return Err(
            "OpenCode pending requests changed beyond the recovery window; refresh this project"
                .into(),
        );
    }
    let mut values = Vec::new();
    for (kind, rows) in snapshot.inventories {
        for properties in rows
            .as_array()
            .ok_or("Invalid OpenCode pending request inventory")?
        {
            values.extend(normalize_event(state, json!({"directory":directory,"payload":{"type":format!("{kind}.asked"),"properties":properties}})));
        }
    }
    Ok(values)
}

pub(crate) fn normalized_thread(session: &Value) -> Value {
    json!({"id":session["id"],"title":session["title"],"cwd":session["directory"]})
}
pub(crate) async fn catalog(api: &Api, cursor: Option<&str>) -> Result<Value, String> {
    let mut page = api
        .json(
            Method::POST,
            "/studio/native/utility-session",
            Some(&json!({"operation":"catalog","backend":"opencode","cursor":cursor})),
        )
        .await?;
    crate::worker::compute(move || {
        let rows = page
            .get_mut("data")
            .and_then(Value::as_array_mut)
            .ok_or("Invalid OpenCode catalog page")?;
        if rows.len() > 100 {
            return Err("OpenCode catalog page exceeds 100 sessions".into());
        }
        let data = rows.iter().map(normalized_thread).collect::<Vec<_>>();
        Ok(json!({"data":data,"nextCursor":page["nextCursor"]}))
    })
    .await
}
fn running(status: &Value) -> bool {
    matches!(
        status.as_str().unwrap_or_else(|| text(status, "type")),
        "busy" | "retry" | "running"
    )
}
fn session_path(id: &str, suffix: &str) -> String {
    format!("/session/{}{suffix}", segment(id))
}
pub fn session_metadata_path(id: &str, directory: &str) -> String {
    endpoint(&session_path(id, ""), directory, &[])
}
pub fn status_path(directory: &str) -> String {
    endpoint("/session/status", directory, &[])
}
pub fn message_page_path(id: &str, directory: &str, cursor: Option<&str>, limit: usize) -> String {
    let mut query = vec![("limit", limit.clamp(1, 200).to_string())];
    if let Some(cursor) = cursor {
        query.push(("before", cursor.into()));
    }
    endpoint(&session_path(id, "/message"), directory, &query)
}

pub fn message_path(id: &str, directory: &str, message: &str) -> String {
    endpoint(
        &session_path(id, &format!("/message/{}", segment(message))),
        directory,
        &[],
    )
}

/// One stable user item per message, assistant/tool items retain their part IDs.
/// The caller owns the page-size/transport budget and performs this on a worker.
fn user_item(id: &str, parts: &Value) -> Value {
    let content = parts
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| {
            if part["type"] == "text" {
                Some(json!({"type":"text","text":part["text"]}))
            } else if part["type"] == "file" {
                Some(
                    crate::conversation_images::Image::from_part(part)
                        .map(|i| i.part())
                        .unwrap_or(json!({"type":"text","text":"[Attached file]"})),
                )
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    json!({"id":id,"turnId":id,"type":"userMessage","content":content})
}
pub fn message_items(messages: &Value) -> Result<Vec<Value>, String> {
    let messages = messages
        .as_array()
        .ok_or("OpenCode returned an invalid message page")?;
    let mut output = Vec::new();
    let mut last_user = String::new();
    for message in messages {
        let info = &message["info"];
        let id = text(info, "id");
        let role = text(info, "role");
        if id.is_empty() || id.len() > 256 {
            return Err("OpenCode returned an invalid message identity".into());
        }
        if role == "user" {
            last_user = id.into();
            output.push(user_item(id, &message["parts"]));
            continue;
        }
        let turn = info["parentID"]
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap_or(if last_user.is_empty() { id } else { &last_user });
        for part in message["parts"].as_array().into_iter().flatten() {
            if let Some(mut item) = part_item(part) {
                item["turnId"] = json!(turn);
                output.push(item);
            }
        }
        if role == "assistant" {
            if let Some(item) = structured_item(info, turn) {
                output.push(item);
            }
        }
    }
    Ok(output)
}

fn parts_fingerprint(parts: &[Value]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for part in parts {
        match text(part, "type") {
            "text" if !text(part, "text").is_empty() => {
                "text".hash(&mut hash);
                text(part, "text").hash(&mut hash);
            }
            "file" => {
                "file".hash(&mut hash);
                text(part, "mime").hash(&mut hash);
                text(part, "url").hash(&mut hash);
            }
            _ => {}
        }
    }
    hash.finish()
}
fn image_parts(input: Value) -> Result<Vec<Value>, String> {
    let mut output = Vec::new();
    let mut images = 0;
    let mut encoded_bytes = 0;
    for mut part in input
        .as_array()
        .ok_or("Invalid OpenCode message input")?
        .iter()
        .cloned()
    {
        match text(&part, "type") {
            "text" => {
                if !text(&part, "text").is_empty() {
                    output.push(json!({"type":"text","text":part["text"].take()}));
                }
            }
            "image" => {
                images += 1;
                let url = text(&part, "url");
                let (prefix, encoded) = url.split_once(',').ok_or("Invalid image URL")?;
                let mime = prefix
                    .strip_prefix("data:")
                    .and_then(|value| value.strip_suffix(";base64"))
                    .filter(|mime| crate::attachments::MIMES.contains(mime))
                    .ok_or("Only inline PNG, JPEG, WebP and GIF images can be sent")?;
                encoded_bytes += encoded.len();
                if images > crate::attachments::MAX_IMAGES
                    || encoded.len() > (crate::attachments::MAX_FILE + 2) / 3 * 4
                    || encoded_bytes > (crate::attachments::MAX_TOTAL + 2) / 3 * 4
                    || encoded.is_empty()
                    || !encoded
                        .bytes()
                        .all(|ch| ch.is_ascii_alphanumeric() || b"+/=".contains(&ch))
                {
                    return Err("OpenCode image limit exceeded or invalid image data".into());
                }
                let extension = match mime {
                    "image/jpeg" => "jpg",
                    "image/webp" => "webp",
                    "image/gif" => "gif",
                    _ => "png",
                };
                output.push(json!({"type":"file","mime":mime,"filename":format!("image-{images}.{extension}"),"url":url}));
            }
            "file" => {
                let root = std::fs::canonicalize(text(&part, "root"))
                    .map_err(|_| "The reference project is unavailable")?;
                let path = std::path::Path::new(text(&part, "path"));
                let path = std::fs::canonicalize(if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    root.join(path)
                })
                .map_err(|_| "The referenced file no longer exists")?;
                if !path.starts_with(&root) || !path.is_file() {
                    return Err("File references must stay within their originating project".into());
                }
                let url = url::Url::from_file_path(&path).map_err(|_| "Invalid reference path")?;
                let label = format!("@{}", text(&part, "path"));
                output.push(json!({"type":"file","mime":"text/plain","filename":part["path"],"url":url.as_str(),"source":{"type":"file","path":part["path"],"text":{"value":label,"start":0,"end":label.encode_utf16().count()}}}));
            }
            _ => {
                return Err(
                    "This OpenCode input type has not been ported; the message was not sent".into(),
                )
            }
        }
    }
    if output.is_empty() {
        return Err("Enter a message or attach an image before sending".into());
    }
    Ok(output)
}

fn command_result(result: Value, input: Value) -> Result<Value, String> {
    let info = &result["info"];
    let id = if info["role"] == "user" {
        text(info, "id")
    } else {
        text(info, "parentID")
    };
    if id.is_empty() {
        return Err("OpenCode completed the command without returning its user message ID. Refresh before retrying; the command was not resent.".into());
    }
    let mut items = vec![json!({"id":id,"type":"userMessage","content":input})];
    for part in result["parts"].as_array().into_iter().flatten() {
        if let Some(item) = part_item(part) {
            items.push(item);
        }
    }
    if info["role"] == "assistant" {
        if let Some(item) = structured_item(info, id) {
            items.push(item);
        }
    }
    let failed = info.get("error").is_some_and(|error| !error.is_null());
    let error = if failed {
        let message = info
            .pointer("/error/data/message")
            .or_else(|| info.pointer("/error/message"))
            .and_then(Value::as_str)
            .unwrap_or("OpenCode command failed");
        json!({"message":message.chars().take(2048).collect::<String>()})
    } else {
        Value::Null
    };
    Ok(
        json!({"turn":{"id":id,"status":if failed{"failed"}else{"completed"},"items":items,"error":error}}),
    )
}
async fn rpc(api: &Api, state: &Shared, method: &str, mut params: Value) -> Result<Value, String> {
    let identity = text(&params, "threadId").to_owned();
    let id = identity.as_str();
    let directory = if let Some(cwd) = params["cwd"]
        .as_str()
        .or_else(|| params.pointer("/roots/0").and_then(Value::as_str))
        .or_else(|| params.pointer("/cwds/0").and_then(Value::as_str))
    {
        cwd.to_owned()
    } else {
        state
            .lock()
            .unwrap()
            .directories
            .get(id)
            .cloned()
            .unwrap_or_default()
    };
    let get = |path: &str| endpoint(path, &directory, &[]);
    match method {
        "thread/shellCommand"=>{
            let (provider,model)=params["model"].as_str().and_then(|model|model.split_once('/')).ok_or("Choose an OpenCode model before running a shell command")?;
            let result=api.command(&get(&session_path(id,"/shell")),&json!({"agent":"build","model":{"providerID":provider,"modelID":model},"command":params["command"]})).await?;
            let input=json!([{"type":"text","text":format!("!{}",text(&params,"command"))}]);
            crate::worker::compute(move||command_result(result,input)).await
        }
        "thread/compact/start"|"review/start"=>{
            let command=if method=="review/start"{"review"}else{"compact"};
            let result=api.command(&get(&session_path(id,"/command")),&json!({"command":command,"arguments":"","agent":"build"})).await?;
            let input=json!([{"type":"text","text":format!("/{command}")}]);
            crate::worker::compute(move||command_result(result,input)).await
        }
        "fuzzyFileSearch"=>{
            let query=params["query"].as_str().unwrap_or("");
            let value=api.json(Method::GET,&endpoint("/find/file",&directory,&[("query",query.into()),("type","file".into()),("dirs","false".into()),("limit","200".into())]),None).await?;
            crate::worker::compute(move||Ok(json!({"files":value.as_array().into_iter().flatten().take(200).filter_map(|path|path.as_str()).map(|path|json!({"path":path})).collect::<Vec<_>>()}))).await
        }
        "skills/list"=>{
            let value=api.json(Method::GET,&get("/command"),None).await?;
            crate::worker::compute(move||Ok(json!({"data":[{"skills":value.as_array().into_iter().flatten().take(512).filter(|command|command["source"]=="skill").map(|command|json!({"name":command["name"],"path":command["name"],"description":command["description"],"enabled":true})).collect::<Vec<_>>()}]}))).await
        }
        "thread/list"=>{
            let value=catalog(api,params["cursor"].as_str()).await?;
            let state=state.clone();crate::worker::compute(move||{
                let sessions=value["data"].as_array().ok_or("OpenCode catalog is not a session list")?;
                let mut state=state.lock().unwrap();if state.directories.len()>2048{state.directories.clear();}
                for session in sessions {state.directories.insert(text(session,"id").into(),text(session,"cwd").into());}
                Ok(value)
            }).await
        }
        "model/list"=>{
            let value=api.json(Method::GET,&get("/config/providers"),None).await?;
            crate::worker::compute(move||{
                let mut models=Vec::new();
                if let Some(providers)=value["providers"].as_array(){for provider in providers.iter().take(100){if let Some(entries)=provider["models"].as_object(){for (id,_) in entries.iter().take(200){models.push(json!({"model":format!("{}/{id}",text(provider,"id"))}));}}}}
                Ok(json!({"data":models}))
            }).await
        }
        "thread/resume"|"thread/read"=>{
            let session=api.json(Method::GET,&get(&session_path(id,"")),None).await?;
            let directory=text(&session,"directory");
            state.lock().unwrap().directories.insert(id.into(),directory.into());
            if method=="thread/read" && params["includeTurns"]==false {
                return crate::worker::compute(move||Ok(json!({"thread":normalized_thread(&session)}))).await;
            }
            for _ in 0..3 {
                let sequence=state.lock().unwrap().changed.get(id).copied().unwrap_or(0);
                let messages=api.json(Method::GET,&endpoint(&session_path(id,"/message"),directory,&[("limit","200".into())]),None).await?;
                let statuses=api.json(Method::GET,&endpoint("/session/status",directory,&[]),None).await?;
                let state=state.clone();let session=session.clone();let status=statuses[id].clone();
                let result=crate::worker::compute(move||{
                    let mut projection=state.lock().unwrap();
                    if projection.changed.get(text(&session,"id")).copied().unwrap_or(0)!=sequence{return Ok(None);}
                    Ok(Some(history(&mut projection,&session,&messages,&status)))
                }).await?;
                if let Some(thread)=result{return Ok(json!({"thread":thread}));}
            }
            Err("OpenCode changed while history was loading; refresh the session".into())
        }
        "thread/start"=>{
            let value=api.json(Method::POST,&get("/session"),Some(&if let Some(name)=params["name"].as_str(){json!({"title":name})}else{json!({})})).await?;
            state.lock().unwrap().directories.insert(text(&value,"id").into(),text(&value,"directory").into());
            Ok(json!({"thread":normalized_thread(&value)}))
        }
        "turn/start"=>{
            let skills=params["input"].as_array().into_iter().flatten().filter(|part|part["type"]=="skill").collect::<Vec<_>>();
            if !skills.is_empty() {
                if skills.len()!=1 || params["input"].as_array().into_iter().flatten().any(|part|part["type"]!="skill"&&part["type"]!="text"){return Err("OpenCode accepts one skill command at a time, without file, plugin or image attachments".into());}
                let name=text(skills[0],"name");
                let arguments=params["input"].as_array().into_iter().flatten().filter(|part|part["type"]=="text").map(|part|text(part,"text")).collect::<Vec<_>>().join("\n");
                let result=api.command(&get(&session_path(id,"/command")),&json!({"command":name,"arguments":arguments,"agent":"build"})).await?;
                let input=params["input"].take();
                return crate::worker::compute(move||command_result(result,input)).await;
            }
            let message_path=endpoint(&session_path(id,"/message"),&directory,&[("limit","200".into())]);
            let before=api.json(Method::GET,&message_path,None).await?;
            let known=before.as_array().into_iter().flatten().filter_map(|message|message.pointer("/info/id").and_then(Value::as_str)).map(str::to_owned).collect::<HashSet<_>>();
            let (body,expected)=crate::worker::compute(move||prompt_body(params)).await?;
            // Identity belongs to OpenCode. In particular, never add messageID.
            api.json(Method::POST,&get(&session_path(id,"/prompt_async")),Some(&body)).await?;
            for _ in 0..10 {
                let messages=api.json(Method::GET,&message_path,None).await?;
                let known=known.clone();
                let matched=crate::worker::compute(move|| {
                    Ok(messages.as_array().into_iter().flatten().rev().find(|message|message.pointer("/info/role").and_then(Value::as_str)==Some("user")&&message.pointer("/info/id").and_then(Value::as_str).is_some_and(|id|!known.contains(id))&&message["parts"].as_array().is_some_and(|parts|parts_fingerprint(parts)==expected)).map(|message|message["info"]["id"].clone()))
                }).await?;
                if let Some(id)=matched {return Ok(json!({"turn":{"id":id}}));}
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err("OpenCode accepted the message but its identity is not yet available. Refresh before retrying; the prompt was not resent.".into())
        }
        "thread/name/set"=>api.json(Method::PATCH,&get(&session_path(id,"")),Some(&json!({"title":params["name"]}))).await,
        "thread/fork"=>{
            let value=api.json(Method::POST,&get(&session_path(id,"/fork")),Some(&json!({}))).await?;
            state.lock().unwrap().directories.insert(text(&value,"id").into(),text(&value,"directory").into());
            Ok(json!({"thread":normalized_thread(&value)}))
        }
        "thread/delete"=>api.json(Method::DELETE,&get(&session_path(id,"")),None).await,
        "thread/archive"|"thread/unarchive"=>Err("OpenCode does not provide separate archive/restore operations".into()),
        "turn/interrupt"=>{
            if let Some(expected)=params.get("expectedUserMessageId") {
                let expected=expected.as_str().filter(|id|!id.is_empty()&&id.len()<=256&&!id.chars().any(char::is_control)).ok_or("Invalid generation message identity")?.to_owned();
                let messages=api.json(Method::GET,&endpoint(&session_path(id,"/message"),&directory,&[("limit","200".into())]),None).await?;
                let session=id.to_owned();
                crate::worker::compute(move||validate_generation_stop(&messages,&session,&expected)).await?;
            }
            api.json(Method::POST,&get(&session_path(id,"/abort")),None).await
        },
        "turn/steer"=>Err("Messages cannot be added while OpenCode is running. Wait for completion or stop it first.".into()),
        _=>Err(format!("OpenCode does not support {method} in the native client yet")),
    }
}

fn validate_generation_stop(messages: &Value, session: &str, expected: &str) -> Result<(), String> {
    let rows = messages
        .as_array()
        .filter(|r| r.len() <= 200)
        .ok_or("Invalid generation message history")?;
    let latest = rows
        .iter()
        .rev()
        .find(|m| m.pointer("/info/role").and_then(Value::as_str) == Some("user"));
    if expected.is_empty()
        || latest.is_none_or(|m| {
            m.pointer("/info/id").and_then(Value::as_str) != Some(expected)
                || m.pointer("/info/sessionID")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id != session)
        })
    {
        return Err("OpenCode has a different active message; Studio will not stop it".into());
    }
    Ok(())
}

fn prompt_body(mut params: Value) -> Result<(Value, u64), String> {
    let parts = image_parts(params["input"].take())?;
    let expected = parts_fingerprint(&parts);
    let mut body = json!({"parts":parts});
    if let Some((provider, model)) = params["model"]
        .as_str()
        .and_then(|model| model.split_once('/'))
    {
        body["model"] = json!({"providerID":provider,"modelID":model});
    }
    if !params["developerInstructions"].is_null() {
        let instructions = params["developerInstructions"]
            .as_str()
            .ok_or("OpenCode instructions must be text")?;
        if instructions.len() > 512 * 1024 {
            return Err("OpenCode instructions exceed 512 KiB".into());
        }
        if !instructions.is_empty() {
            body["system"] = params["developerInstructions"].take();
        }
    }
    if !params["outputSchema"].is_null() {
        if !params["outputSchema"].is_object() {
            return Err("OpenCode output schema must be an object".into());
        }
        struct Budget(usize);
        impl std::io::Write for Budget {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if bytes.len() > self.0 {
                    return Err(std::io::Error::other("JSON size limit"));
                }
                self.0 -= bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        serde_json::to_writer(Budget(512 * 1024), &params["outputSchema"])
            .map_err(|_| "OpenCode output schema exceeds 512 KiB")?;
        body["format"] =
            json!({"type":"json_schema","schema":params["outputSchema"].take(),"retryCount":2});
    }
    Ok((body, expected))
}

fn structured_item(info: &Value, turn: &str) -> Option<Value> {
    let id = text(info, "id");
    if id.is_empty() || id.len() > 256 {
        return None;
    }
    let value = info.get("structured").filter(|value| !value.is_null())?;
    Some(
        json!({"id":format!("{id}-structured"),"turnId":turn,"type":"agentMessage","text":serde_json::to_string(value).ok()?}),
    )
}

fn part_item(part: &Value) -> Option<Value> {
    let id = &part["id"];
    Some(match text(part, "type") {
        "text" => json!({"id":id,"type":"agentMessage","text":part["text"]}),
        "reasoning" => json!({"id":id,"type":"reasoning","summary":[{"text":part["text"]}]}),
        "tool" => {
            json!({"id":id,"type":"commandExecution","command":part["tool"],"aggregatedOutput":part["state"]["output"].as_str().or(part["state"]["error"].as_str()).unwrap_or(""),"status":part["state"]["status"],"opencode":part})
        }
        _ => return None,
    })
}

fn history(state: &mut Projection, session: &Value, messages: &Value, status: &Value) -> Value {
    let mut items = Vec::new();
    let mut last_user = String::new();
    for message in messages
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .take(200)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let info = &message["info"];
        let role = text(info, "role");
        let id = text(info, "id");
        if role == "user" {
            last_user = id.into();
            state
                .latest_turns
                .insert(text(session, "id").into(), id.into());
        }
        let turn_id = if role == "user" {
            id
        } else {
            info["parentID"]
                .as_str()
                .filter(|value| !value.is_empty())
                .unwrap_or(if last_user.is_empty() { id } else { &last_user })
        };
        state.message_turns.insert(id.into(), turn_id.into());
        state.roles.insert(id.into(), role.into());
        if role == "user" {
            state.users.replace(id, &message["parts"]);
            // Keep only bounded live assembly state, but project the complete
            // history value so the separate output snapshot can retain it.
            items.push(user_item(id, &message["parts"]));
            continue;
        }
        for part in message["parts"].as_array().into_iter().flatten().take(400) {
            if let Some(mut item) = part_item(part) {
                item["turnId"] = json!(turn_id);
                state.parts.insert(text(part, "id").into());
                items.push(item);
            }
        }
        if role == "assistant" {
            if let Some(item) = structured_item(info, turn_id) {
                items.push(item);
            }
        }
    }
    if items.len() > 400 {
        items.drain(..items.len() - 400);
    }
    let mut thread = normalized_thread(session);
    if last_user.is_empty() {
        last_user = format!("opencode-{}", text(session, "id"));
    }
    thread["turns"] = json!([{"id":last_user,"status":if running(status){"inProgress"}else{"completed"},"items":items}]);
    thread
}

fn normalize_event(state: &mut Projection, event: Value) -> Vec<Value> {
    let envelope_directory = event.get("directory").and_then(Value::as_str);
    let event = event.get("payload").unwrap_or(&event);
    let properties = &event["properties"];
    let kind = text(event, "type");
    if state.roles.len() > 4096 {
        state.roles.clear();
        state.message_turns.clear();
    }
    if state.latest_turns.len() > 2048 {
        state.latest_turns.clear();
    }
    if state.parts.len() > 4096 {
        state.parts.clear();
    }
    if state.status.len() > 2048 {
        state.status.clear();
    }
    let session = properties["sessionID"]
        .as_str()
        .or(properties
            .pointer("/info/sessionID")
            .and_then(Value::as_str))
        .or(properties
            .pointer("/part/sessionID")
            .and_then(Value::as_str))
        .unwrap_or("");
    if kind.starts_with("message.") || kind.starts_with("session.") {
        state.sequence += 1;
        if state.changed.len() > 4096 {
            state.changed.clear();
        }
        state.changed.insert(session.into(), state.sequence);
    }
    let notification = |method: &str, params: Value| json!({"method":method,"params":params});
    match kind {
        "message.updated" => {
            let info = &properties["info"];
            let id = text(info, "id");
            if text(info, "role") == "user" {
                state.latest_turns.insert(session.into(), id.into());
            }
            let turn_id = if text(info, "role") == "user" {
                id
            } else {
                info["parentID"]
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .or_else(|| state.latest_turns.get(session).map(String::as_str))
                    .unwrap_or(id)
            };
            state.message_turns.insert(id.into(), turn_id.into());
            state
                .roles
                .insert(text(info, "id").into(), text(info, "role").into());
            if text(info, "role") == "assistant" {
                if let Some(item) = structured_item(info, turn_id) {
                    return vec![notification(
                        "item/completed",
                        json!({"threadId":session,"turnId":turn_id,"item":item}),
                    )];
                }
            }
            Vec::new()
        }
        "message.part.updated" => {
            let part = &properties["part"];
            let role = state
                .roles
                .get(text(part, "messageID"))
                .map(String::as_str)
                .unwrap_or("assistant");
            if role == "user" && (part["type"] == "text" || part["type"] == "file") {
                let id = text(part, "messageID");
                let item = if part["type"] == "file" {
                    state.users.file(id, part)
                } else {
                    state
                        .users
                        .update(id, text(part, "id"), text(part, "text"), false)
                };
                return vec![notification(
                    "item/completed",
                    json!({"threadId":session,"turnId":id,"item":item}),
                )];
            }
            if let Some(item) = part_item(part) {
                let turn_id = state
                    .message_turns
                    .get(text(part, "messageID"))
                    .or_else(|| state.latest_turns.get(session))
                    .cloned()
                    .unwrap_or_default();
                state.parts.insert(text(part, "id").into());
                vec![notification(
                    "item/completed",
                    json!({"threadId":session,"turnId":turn_id,"item":item}),
                )]
            } else {
                Vec::new()
            }
        }
        "message.part.delta" => {
            let id = text(properties, "partID");
            let message = text(properties, "messageID");
            if state.roles.get(message).map(String::as_str) == Some("user")
                && text(properties, "field") == "text"
            {
                let item = state
                    .users
                    .update(message, id, text(properties, "delta"), true);
                return vec![notification(
                    "item/completed",
                    json!({"threadId":session,"turnId":message,"item":item}),
                )];
            }
            let mut values = Vec::new();
            if state.parts.insert(id.into()) {
                let turn_id = state
                    .message_turns
                    .get(text(properties, "messageID"))
                    .or_else(|| state.latest_turns.get(session))
                    .cloned()
                    .unwrap_or_default();
                values.push(notification(
                    "item/started",
                    json!({"threadId":session,"turnId":turn_id,"item":{"id":id,"type":"agentMessage","text":""}}),
                ));
            }
            values.push(notification(
                "item/agentMessage/delta",
                json!({"threadId":session,"itemId":id,"delta":properties["delta"]}),
            ));
            values
        }
        "session.status" | "session.idle" => {
            let status = if kind == "session.idle" {
                json!({"type":"idle"})
            } else {
                properties["status"].clone()
            };
            state
                .status
                .insert(session.into(), (state.sequence, status.clone()));
            if running(&status) {
                vec![notification(
                    "turn/started",
                    json!({"threadId":session,"turn":{"id":format!("opencode-{session}")}}),
                )]
            } else if status == "idle" || status["type"] == "idle" {
                vec![notification("turn/completed", json!({"threadId":session}))]
            } else {
                Vec::new()
            }
        }
        "session.diff" => vec![notification(
            "turn/diff/updated",
            json!({"threadId":session,"diff":properties["diff"].as_str().map(str::to_owned).unwrap_or_else(|| serde_json::to_string_pretty(properties.get("diff").unwrap_or(properties)).unwrap_or_default())}),
        )],
        "session.error" => vec![notification(
            "error",
            json!({"threadId":session,"error":{"message":properties.pointer("/error/data/message").or(properties.pointer("/error/message")).and_then(Value::as_str).unwrap_or("OpenCode session failed")}}),
        )],
        "permission.asked" | "question.asked" => {
            let kind = if kind == "permission.asked" {
                "permission"
            } else {
                "question"
            };
            let directory = envelope_directory
                .or_else(|| state.directories.get(session).map(String::as_str))
                .unwrap_or("");
            match state.requests.ask(kind, properties, directory) {
                Ok(Some(id)) => {
                    let mut params = properties.clone();
                    params["cwd"] = directory.into();
                    vec![json!({"id":id,"method":format!("opencode/{kind}"),"params":params})]
                }
                Ok(None) => Vec::new(),
                Err(error) => vec![notification(
                    "error",
                    json!({"threadId":session,"error":{"message":error}}),
                )],
            }
        }
        "permission.replied" | "question.replied" | "question.rejected" => {
            let kind = if kind == "permission.replied" {
                "permission"
            } else {
                "question"
            };
            let directory = envelope_directory
                .or_else(|| state.directories.get(session).map(String::as_str))
                .unwrap_or("");
            state
                .requests
                .resolve(kind, properties, directory)
                .map(|id| {
                    vec![notification(
                        "serverRequest/resolved",
                        json!({"threadId":session,"requestId":id}),
                    )]
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

async fn reply(
    api: &Api,
    state: &Shared,
    id: Value,
    result: Result<Value, crate::protocol::ReplyError>,
) -> Result<Value, String> {
    let route = state
        .lock()
        .map_err(|_| "OpenCode request registry unavailable")?
        .requests
        .claim(&id)?;
    let (path, body) = reply_body(id, result)?;
    api.json(
        Method::POST,
        &endpoint(&path, &route.directory, &[]),
        Some(&body),
    )
    .await
}
fn reply_body(
    id: Value,
    result: Result<Value, crate::protocol::ReplyError>,
) -> Result<(String, Value), String> {
    let id = id.as_str().ok_or("Invalid OpenCode interaction")?;
    let (kind, id) = id
        .split_once(':')
        .ok_or("Invalid OpenCode interaction identity")?;
    let result = result.unwrap_or(Value::Null);
    let (path, body) = match kind {
        "permission" => (
            format!("/permission/{}/reply", segment(id)),
            json!({"reply":match result["decision"].as_str(){Some("accept")=>"once",Some("acceptForSession")=>"always",_=>"reject"}}),
        ),
        "question" if result["answers"].is_array() => (
            format!("/question/{}/reply", segment(id)),
            json!({"answers":result["answers"]}),
        ),
        "question" => (format!("/question/{}/reject", segment(id)), json!({})),
        _ => return Err("Unsupported OpenCode interaction".into()),
    };
    Ok((path, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tool_projection_retains_original_input_error_metadata_and_server_identity() {
        let part = json!({"id":"server-tool-part","messageID":"server-message","sessionID":"session","type":"tool","callID":"owned-call","tool":"bash","state":{"status":"error","input":{"command":"printf '<literal> 中文😀'"},"error":"Failed","metadata":{"exit":23},"time":{"start":1,"end":2}}});
        let value = part_item(&part).unwrap();
        assert_eq!(value["opencode"], part);
        assert_eq!(value["id"], "server-tool-part");
        let item = crate::model::Item::from_value(&value);
        assert!(item.details.is_some());
        assert!(item.body.contains("Failed"));
    }
    #[tokio::test]
    async fn generation_stop_rejects_a_newer_message_before_sending_abort() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = Api::from_access_url(&format!(
            "http://{}/#token=0123456789abcdef0123456789abcdef",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let server = tokio::spawn(async move {
            for step in 0..3 {
                let (mut stream, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut header = Vec::new();
                while !header.windows(4).any(|s| s == b"\r\n\r\n") {
                    let mut bytes = [0; 2048];
                    let n = stream.read(&mut bytes).await.unwrap();
                    assert!(n > 0);
                    header.extend_from_slice(&bytes[..n]);
                }
                let header = String::from_utf8(header).unwrap();
                let first = header
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .collect::<Vec<_>>();
                let url = url::Url::parse(&format!("http://localhost{}", first[1])).unwrap();
                assert_eq!(
                    url.query_pairs().find(|(k, _)| k == "directory").unwrap().1,
                    "/project 中文"
                );
                let value = if step < 2 {
                    assert_eq!(first[0], "GET");
                    assert_eq!(url.path(), "/opencode/session/s/message");
                    assert_eq!(
                        url.query_pairs().find(|(k, _)| k == "limit").unwrap().1,
                        "200"
                    );
                    json!([{"info":{"id":if step==0{"foreign-user"}else{"owned-user"},"role":"user","sessionID":"s"}}])
                } else {
                    assert_eq!(first[0], "POST");
                    assert_eq!(url.path(), "/opencode/session/s/abort");
                    json!(true)
                };
                let body = value.to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let state = Shared::default();
        let params = json!({"threadId":"s","cwd":"/project 中文","turnId":"owned-user","expectedUserMessageId":"owned-user"});
        assert!(rpc(&api, &state, "turn/interrupt", params.clone())
            .await
            .unwrap_err()
            .contains("different active message"));
        assert_eq!(
            rpc(&api, &state, "turn/interrupt", params).await.unwrap(),
            json!(true)
        );
        server.await.unwrap();
        assert!(validate_generation_stop(
            &json!([{"info":{"id":"owned-user","role":"user","sessionID":"foreign-session"}}]),
            "s",
            "owned-user"
        )
        .is_err());
    }
    #[test]
    fn completed_recovery_is_projected_after_live_resolutions() {
        let mut state = Projection::default();
        // The HTTP task completed before this SSE event, but its result has
        // not yet been emitted by the connection's ordered event loop.
        let snapshot = RecoverySnapshot {
            marker: state.requests.snapshot(),
            inventories: vec![(
                "question".into(),
                json!([
                    {"id":"resolved","sessionID":"s"},
                    {"id":"still-pending","sessionID":"s"}
                ]),
            )],
        };
        normalize_event(
            &mut state,
            json!({"directory":"/origin","payload":{
                "type":"question.rejected","properties":{"requestID":"resolved","sessionID":"s"}
            }}),
        );
        let projected = project_recovery(&mut state, "/origin", snapshot).unwrap();
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0]["id"], "question:still-pending");
        assert!(state.requests.claim(&json!("question:resolved")).is_err());
    }
    #[test]
    fn global_requests_keep_envelope_scope_and_ignore_foreign_resolution() {
        let mut state = Projection::default();
        state.directories.insert("s".into(), "/cached".into());
        let asked = normalize_event(
            &mut state,
            json!({"directory":"/actual 中文","payload":{"type":"permission.asked","properties":{"id":"p","sessionID":"s","permission":"bash"}}}),
        );
        assert_eq!(asked[0]["id"], "permission:p");
        assert_eq!(asked[0]["params"]["cwd"], "/actual 中文");
        state
            .directories
            .insert("s".into(), "/selected later".into());
        assert_eq!(
            state
                .requests
                .claim(&json!("permission:p"))
                .unwrap()
                .directory,
            "/actual 中文"
        );
        assert!(normalize_event(&mut state,json!({"directory":"/wrong","payload":{"type":"permission.replied","properties":{"requestID":"p","sessionID":"s"}}})).is_empty());
        let resolved = normalize_event(
            &mut state,
            json!({"directory":"/actual 中文","payload":{"type":"permission.replied","properties":{"requestID":"p","sessionID":"s"}}}),
        );
        assert_eq!(resolved[0]["method"], "serverRequest/resolved");
        assert_eq!(resolved[0]["params"]["requestId"], "permission:p");
        assert!(state.requests.claim(&json!("permission:p")).is_err());
    }
    #[test]
    fn session_permission_uses_always_without_expanding_once_or_decline() {
        for (decision, expected) in [
            ("accept", "once"),
            ("acceptForSession", "always"),
            ("decline", "reject"),
            ("unknown", "reject"),
        ] {
            let (path, body) = reply_body(
                json!("permission:server/id"),
                Ok(json!({"decision":decision})),
            )
            .unwrap();
            assert_eq!(path, "/permission/server%2Fid/reply");
            assert_eq!(body, json!({"reply":expected}));
        }
        assert_eq!(
            reply_body(
                json!("question:q"),
                Err(crate::protocol::ReplyError {
                    code: -32001,
                    message: "Cancelled".into()
                })
            )
            .unwrap()
            .0,
            "/question/q/reject"
        );
    }
    #[test]
    fn structured_output_preserves_identity_turn_and_json_across_history_pages_and_events() {
        let decision = json!({"action":"clarify","message":"Which 中文 session? 😀","targetSessionKey":"","forwardedPrompt":"","reason":""});
        let info = json!({"id":"a","sessionID":"s","role":"assistant","parentID":"server-user","structured":decision});
        let messages = json!([
            {"info":{"id":"server-user","role":"user"},"parts":[{"type":"text","text":"Route this","id":"u-part"}]},
            {"info":info,"parts":[]}
        ]);
        let raw = message_items(&messages).unwrap();
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[1]["id"], "a-structured");
        assert_eq!(raw[1]["turnId"], "server-user");
        assert_eq!(
            serde_json::from_str::<Value>(raw[1]["text"].as_str().unwrap()).unwrap(),
            decision
        );
        let mut state = Projection::default();
        let restored = history(
            &mut state,
            &json!({"id":"s"}),
            &messages,
            &json!({"type":"busy"}),
        );
        assert_eq!(restored["turns"][0]["status"], "inProgress");
        assert_eq!(restored["turns"][0]["items"][1], raw[1]);
        let events = normalize_event(
            &mut state,
            json!({"type":"message.updated","properties":{"info":info}}),
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["method"], "item/completed");
        assert_eq!(events[0]["params"]["threadId"], "s");
        assert_eq!(events[0]["params"]["turnId"], "server-user");
        assert_eq!(events[0]["params"]["item"], raw[1]);
        let completed = command_result(json!({"info":info,"parts":[]}), json!([])).unwrap();
        assert_eq!(completed["turn"]["items"][1], raw[1]);
        let mut updated = info.clone();
        updated["structured"] = json!({"updated":true});
        let changed = normalize_event(
            &mut state,
            json!({"type":"message.updated","properties":{"info":updated}}),
        );
        assert_eq!(changed[0]["params"]["item"]["id"], "a-structured");
        assert_eq!(changed[0]["method"], "item/completed");
    }
    #[test]
    fn malformed_or_over_budget_prompt_options_fail_before_sending_a_model_message() {
        for invalid in [
            json!({"developerInstructions":false}),
            json!({"developerInstructions":"中".repeat(200_000)}),
            json!({"outputSchema":[]}),
            json!({"outputSchema":{"description":"x".repeat(513*1024)}}),
            // Count JSON escaping, not only the in-memory string size.
            json!({"outputSchema":{"description":"\u{0001}".repeat(90_000)}}),
        ] {
            let mut invalid = invalid;
            invalid["input"] = json!([]);
            assert!(prompt_body(invalid).is_err());
        }
        let (ordinary, _) =
            prompt_body(json!({"input":[{"type":"text","text":"space "}]})).unwrap();
        assert_eq!(ordinary, json!({"parts":[{"type":"text","text":"space "}]}));
    }
    #[tokio::test]
    async fn controller_prompt_keeps_system_schema_and_server_owned_message_identity_on_the_wire() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for step in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(4)))
                    .unwrap();
                let mut bytes = Vec::new();
                let boundary = loop {
                    let mut block = [0u8; 2048];
                    let count = socket.read(&mut block).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&block[..count]);
                    if let Some(index) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        break index + 4;
                    }
                };
                let header = String::from_utf8(bytes[..boundary].to_vec()).unwrap();
                let lower = header.to_ascii_lowercase();
                assert!(lower.contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
                let first = header
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .collect::<Vec<_>>();
                let url = url::Url::parse(&format!("http://localhost{}", first[1])).unwrap();
                assert_eq!(
                    url.query_pairs()
                        .find(|(key, _)| key == "directory")
                        .unwrap()
                        .1,
                    "/project 中文"
                );
                let length = lower
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map(|value| value.parse::<usize>().unwrap())
                    .unwrap_or(0);
                while bytes.len() < boundary + length {
                    let mut block = [0u8; 2048];
                    let count = socket.read(&mut block).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&block[..count]);
                }
                let response = if step == 1 {
                    assert_eq!(first[0], "POST");
                    assert_eq!(url.path(), "/opencode/session/s/prompt_async");
                    let body: Value =
                        serde_json::from_slice(&bytes[boundary..boundary + length]).unwrap();
                    assert_eq!(
                        body,
                        json!({"parts":[{"type":"text","text":"Route this 中文 "}],"model":{"providerID":"provider","modelID":"model"},"system":"Choose a session. Do not run tools.","format":{"type":"json_schema","schema":{"type":"object","additionalProperties":false},"retryCount":2}})
                    );
                    Value::Null
                } else {
                    assert_eq!(first[0], "GET");
                    assert_eq!(url.path(), "/opencode/session/s/message");
                    if step == 0 {
                        json!([])
                    } else {
                        json!([{"info":{"id":"server-user","role":"user"},"parts":[{"id":"server-part","type":"text","text":"Route this 中文 "}]}])
                    }
                };
                let body = serde_json::to_vec(&response).unwrap();
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        let api = Api::from_access_url(&format!(
            "http://{address}/#token=0123456789abcdef0123456789abcdef"
        ))
        .unwrap();
        let result=rpc(&api,&Shared::default(),"turn/start",json!({"threadId":"s","cwd":"/project 中文","model":"provider/model","input":[{"type":"text","text":"Route this 中文 "}],"developerInstructions":"Choose a session. Do not run tools.","outputSchema":{"type":"object","additionalProperties":false}})).await.unwrap();
        assert_eq!(result["turn"]["id"], "server-user");
        server.join().unwrap();
    }
    #[test]
    fn session_diff_uses_the_same_worker_projection_without_changing_activity() {
        let mut state = Projection::default();
        let mut values = normalize_event(
            &mut state,
            json!({"type":"session.diff","properties":{"sessionID":"s","diff":[{"file":"中文.md","before":"old","after":"new"}]}}),
        );
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["method"], "turn/diff/updated");
        let update = crate::command_data::prepare(&mut values[0]).unwrap();
        assert_eq!(update.thread, "s");
        let crate::command_data::Data::Diff(Ok(text)) = update.data else {
            panic!("Missing prepared diff")
        };
        assert!(text
            .document
            .window(1, None, false)
            .unwrap()
            .text
            .contains("中文.md"));
        assert!(state.status.is_empty());
    }
    #[test]
    fn image_only_prompts_match_server_identity_without_ignoring_attachments() {
        let parts =
            image_parts(json!([{"type":"image","url":"data:image/png;base64,aGVsbG8="}])).unwrap();
        assert_eq!(parts[0]["type"], "file");
        assert_eq!(parts[0]["mime"], "image/png");
        let expected = parts_fingerprint(&parts);
        let mut echoed = parts.clone();
        echoed[0]["id"] = json!("server-owned-part");
        echoed[0]["filename"] = json!("renamed.png");
        assert_eq!(expected, parts_fingerprint(&echoed));
        echoed[0]["url"] = json!("data:image/png;base64,d29ybGQ=");
        assert_ne!(expected, parts_fingerprint(&echoed));
        assert!(
            image_parts(json!([{"type":"image","url":"https://example.org/private.png"}])).is_err()
        );
        assert!(image_parts(json!([{"type":"skill","name":"unsupported"}])).is_err());
    }
    #[test]
    fn file_references_preserve_unicode_urls_and_reject_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("说明 #?.md"), "private").unwrap();
        let parts = image_parts(json!([{"type":"file","path":"说明 #?.md","root":root}])).unwrap();
        let url = url::Url::parse(parts[0]["url"].as_str().unwrap()).unwrap();
        assert_eq!(url.to_file_path().unwrap(), root.join("说明 #?.md"));
        assert_eq!(parts[0]["source"]["path"], "说明 #?.md");
        let outside = dir.path().join("private.md");
        std::fs::write(&outside, "outside").unwrap();
        assert!(image_parts(json!([{"type":"file","path":"../private.md","root":root}])).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("linked.md")).unwrap();
            assert!(image_parts(json!([{"type":"file","path":"linked.md","root":root}])).is_err());
        }
    }
    #[test]
    fn command_results_keep_server_user_identity_and_explicit_failure() {
        let result=command_result(json!({"info":{"role":"assistant","id":"assistant","parentID":"server-user"},"parts":[{"id":"reply-part","type":"text","text":"# Command result"}]}),json!([{"type":"skill","name":"build","path":"build"}])).unwrap();
        assert_eq!(result["turn"]["id"], "server-user");
        assert_eq!(result["turn"]["status"], "completed");
        assert_eq!(result["turn"]["items"][1]["id"], "reply-part");
        let failed = command_result(
            json!({"info":{"role":"assistant","parentID":"user","error":{"message":"failed"}}}),
            json!([]),
        )
        .unwrap();
        assert_eq!(failed["turn"]["status"], "failed");
        assert!(command_result(
            json!({"info":{"id":"assistant","role":"assistant"}}),
            json!([])
        )
        .is_err());
    }
    #[test]
    fn user_message_identity_and_parts_agree_between_history_events_and_readonly_pages() {
        let message = json!({"info":{"id":"user","role":"user"},"parts":[{"id":"p1","type":"text","text":"First 中文"},{"id":"p2","type":"text","text":"Second 😀"}]});
        let mut state = Projection::default();
        let history = history(
            &mut state,
            &json!({"id":"s"}),
            &json!([message]),
            &json!({"type":"idle"}),
        );
        let items = crate::model::history(&history);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "user");
        assert_eq!(items[0].body, "First 中文\nSecond 😀");
        let events = normalize_event(
            &mut state,
            json!({"type":"message.part.updated","properties":{"sessionID":"s","part":{"id":"p2","messageID":"user","type":"text","text":"Updated 😀"}}}),
        );
        let updated = crate::model::Item::from_value(&events[0]["params"]["item"]);
        assert_eq!(updated.id, "user");
        assert_eq!(updated.body, "First 中文\nUpdated 😀");
        let events = normalize_event(
            &mut state,
            json!({"type":"message.part.delta","properties":{"sessionID":"s","partID":"p2","messageID":"user","field":"text","delta":"!"}}),
        );
        assert_eq!(events[0]["params"]["item"]["id"], "user");
        assert_eq!(
            crate::model::Item::from_value(&events[0]["params"]["item"]).body,
            "First 中文\nUpdated 😀!"
        );
        let raw=message_items(&json!([{"info":{"id":"user","role":"user"},"parts":[{"id":"p1","type":"text","text":"First 中文"},{"id":"p2","type":"text","text":"Updated 😀!"}]}])).unwrap();
        assert_eq!(crate::model::Item::from_value(&raw[0]).id, "user");
        assert_eq!(
            crate::model::Item::from_value(&raw[0]).body,
            "First 中文\nUpdated 😀!"
        );
        let mut user = UserParts::default();
        user.set("large", &"中".repeat(100_000), false);
        assert!(crate::byte_cache::Weight::weight(&user) < 160 * 1024);
        assert!(user.truncated);
    }
    #[test]
    fn native_message_anchors_match_opencode_user_parent_ids() {
        let mut state = Projection::default();
        let thread = history(
            &mut state,
            &json!({"id":"s"}),
            &json!([
                {"info":{"id":"u1","role":"user"},"parts":[{"id":"pu1","type":"text","text":"first"}]},
                {"info":{"id":"a1","role":"assistant","parentID":"u1"},"parts":[{"id":"pa1","type":"text","text":"first answer"}]},
                {"info":{"id":"u2","role":"user"},"parts":[{"id":"pu2","type":"text","text":"second"}]},
                {"info":{"id":"a2","role":"assistant","parentID":"u2"},"parts":[{"id":"pa2","type":"text","text":"second answer"}]}
            ]),
            &json!({"type":"idle"}),
        );
        let items = crate::model::history(&thread);
        assert_eq!(
            items
                .iter()
                .map(|item| item.turn_id.as_str())
                .collect::<Vec<_>>(),
            vec!["u1", "u1", "u2", "u2"]
        );
        let events = normalize_event(
            &mut state,
            json!({"type":"message.part.updated","properties":{"sessionID":"s","part":{"id":"pa1","messageID":"a1","type":"text","text":"updated"}}}),
        );
        assert_eq!(events[0]["params"]["turnId"], "u1");
    }
    #[test]
    fn assistant_completion_does_not_invent_idle() {
        let mut state = Projection::default();
        let busy = normalize_event(
            &mut state,
            json!({"payload":{"type":"session.status","properties":{"sessionID":"s","status":{"type":"busy"}}}}),
        );
        assert_eq!(busy[0]["method"], "turn/started");
        let complete = normalize_event(
            &mut state,
            json!({"type":"message.updated","properties":{"info":{"id":"a","sessionID":"s","role":"assistant","time":{"completed":123}}}}),
        );
        assert!(complete.is_empty());
        let part = normalize_event(
            &mut state,
            json!({"type":"message.part.updated","properties":{"part":{"id":"p","sessionID":"s","messageID":"a","type":"text","text":"done"}}}),
        );
        assert_eq!(part[0]["method"], "item/completed");
        assert!(running(&state.status["s"].1));
        let idle = normalize_event(
            &mut state,
            json!({"type":"session.idle","properties":{"sessionID":"s"}}),
        );
        assert_eq!(idle[0]["method"], "turn/completed");
    }
    #[test]
    fn history_uses_authoritative_status_and_unicode_parts() {
        let mut state = Projection::default();
        let thread = history(
            &mut state,
            &json!({"id":"s","directory":"/中文/project","title":"Native"}),
            &json!([
                {"info":{"id":"u","role":"user"},"parts":[{"id":"up","type":"text","text":"你好"}]},
                {"info":{"id":"a","role":"assistant","time":{"completed":123}},"parts":[{"id":"ap","type":"text","text":"😀"}]}
            ]),
            &json!({"type":"busy"}),
        );
        assert_eq!(thread["turns"][0]["id"], "u");
        assert_eq!(crate::model::active_turn(&thread).as_deref(), Some("u"));
        let items = crate::model::history(&thread);
        assert_eq!(items[0].role, "You");
        assert_eq!(items[1].body, "😀");
        assert_eq!(thread["cwd"], "/中文/project");
    }
    #[test]
    fn large_user_history_preserves_full_source_while_live_assembly_is_bounded() {
        let text = "Original 中文😀\r\n".repeat(20000);
        let mut state = Projection::default();
        let messages = json!([{"info":{"id":"u","role":"user"},"parts":[{"id":"p","type":"text","text":text}]}]);
        let thread = history(
            &mut state,
            &json!({"id":"s","directory":"/project"}),
            &messages,
            &json!({"type":"idle"}),
        );
        assert_eq!(thread["turns"][0]["items"][0]["content"][0]["text"], text);
        let projected = crate::model::history(&thread);
        assert!(projected[0]
            .output
            .as_ref()
            .is_some_and(|r| r.unavailable.is_none()));
        let partial = state.users.update("u", "p", "more", true);
        let live = crate::model::Item::from_value(&partial);
        assert!(live.truncated);
        assert!(live
            .output
            .as_ref()
            .is_some_and(|r| r.unavailable.is_some()));
        assert!(live.body.len() < 129 * 1024);
    }
    #[test]
    fn paths_escape_ids_and_directories_without_query_injection() {
        let path = endpoint(
            &session_path("s/?#", "/message"),
            "/tmp/中文 &x=1",
            &[("limit", "200".into())],
        );
        let parsed = url::Url::parse(&format!("http://127.0.0.1:1{path}")).unwrap();
        assert_eq!(parsed.query_pairs().count(), 2);
        assert_eq!(parsed.query_pairs().next().unwrap().1, "/tmp/中文 &x=1");
        assert!(!parsed.path().contains('?'));
    }
    #[tokio::test]
    async fn maintained_sse_parser_handles_split_unicode_crlf_multiline_and_comments() {
        let bytes =
            "\u{feff}: heartbeat\r\ndata: {\"text\":\r\ndata: \"中文😀\"}\r\n\r\n".as_bytes();
        let chunks = bytes
            .chunks(2)
            .map(|chunk| Ok::<_, std::io::Error>(chunk))
            .collect::<Vec<_>>();
        let mut stream = SseByteStream::new(futures_util::stream::iter(chunks));
        let event = stream.next().await.unwrap().unwrap();
        let value: Value = serde_json::from_str(event.data.as_deref().unwrap()).unwrap();
        assert_eq!(value["text"], "中文😀");
        assert!(stream.next().await.is_none());
    }
}

#[cfg(test)]
mod native_image_tests {
    use super::*;
    #[test]
    fn history_paging_and_text_deltas_preserve_file_images_without_raw_urls() {
        let file = json!({"id":"file","type":"file","mime":"image/png","filename":"fixture.png","url":"data:image/png;base64,aGVsbG8="});
        let mut users = UserCache::default();
        let first = users.replace(
            "user",
            &json!([{"id":"text","type":"text","text":"Hello"},file]),
        );
        assert!(!serde_json::to_string(&first).unwrap().contains("base64"));
        assert_eq!(crate::model::Item::from_value(&first).images.len(), 1);
        let delta = users.update("user", "text", " world", true);
        let item = crate::model::Item::from_value(&delta);
        assert_eq!(item.images.len(), 1);
        assert!(item.body.contains("Hello world"));
        let pages =
            message_items(&json!([{"info":{"id":"user","role":"user"},"parts":[file]}])).unwrap();
        assert_eq!(crate::model::Item::from_value(&pages[0]).images.len(), 1);
        let mut state = Projection::default();
        state.roles.insert("user".into(), "user".into());
        let mut part = file.clone();
        part["messageID"] = json!("user");
        part["sessionID"] = json!("session");
        let events = normalize_event(
            &mut state,
            json!({"type":"message.part.updated","properties":{"part":part}}),
        );
        let mut event = events[0].clone();
        let presentation = crate::protocol::prepare_presentation(&mut event);
        assert_eq!(presentation.item.unwrap().images.len(), 1);
        assert!(!serde_json::to_string(&event).unwrap().contains("base64"));
        let events = normalize_event(
            &mut state,
            json!({"type":"message.part.delta","properties":{"sessionID":"session","messageID":"user","partID":"text","field":"text","delta":"Typed after image"}}),
        );
        let mut event = events[0].clone();
        let presentation = crate::protocol::prepare_presentation(&mut event);
        let item = presentation.item.unwrap();
        assert_eq!(item.images.len(), 1);
        assert!(item.body.contains("Typed after image"));
        let changed = users.file(
            "user",
            &json!({"id":"file","type":"file","mime":"text/plain","filename":"note.txt"}),
        );
        assert!(crate::model::Item::from_value(&changed).images.is_empty());
    }
}

/// Isolated, tool-denied utility session. Only the newly returned identity is
/// ever prompted, interrupted or deleted; the selected conversation is read-only.
pub(crate) async fn utility_translation(
    api: &Api,
    _ticket: u64,
    context: &crate::translation::Context,
    profile: &crate::translation::Profile,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<crate::translation::ResultValue, String> {
    let task = crate::translation::StructuredTask {
        instructions: crate::translation_text::INSTRUCTIONS.into(),
        input: crate::translation_text::input(&profile.source),
        schema: crate::translation_text::schema(),
        model: profile.model.clone(),
        effort: String::new(),
        service_tier: String::new(),
        max_result: crate::translation_text::MAX_RESULT,
        read_completed_thread: false,
    };
    let result = utility_structured(api, context, &task, cancel, "translation").await?;
    let content = crate::worker::compute(move || crate::translation_text::parse(&result.text))
        .await
        .map_err(|error| match &result.warning {
            Some(warning) => format!("{error}; {warning}"),
            None => error,
        })?;
    Ok(crate::translation::ResultValue {
        content: Arc::new(content),
        warning: result.warning,
        cached: false,
    })
}
/// One SDK-created identity, a bounded structured result, and receipt-verified
/// cleanup. Reused by translation and composer continuation, never source turns.
pub(crate) async fn utility_structured(
    api: &Api,
    context: &crate::translation::Context,
    task: &crate::translation::StructuredTask,
    cancel: &std::sync::atomic::AtomicBool,
    purpose: &str,
) -> Result<crate::translation::StructuredResult, String> {
    let check = || {
        if cancel.load(Ordering::Acquire) {
            Err("Utility task cancelled".to_string())
        } else {
            Ok(())
        }
    };
    check()?;
    if !context.ready
        || context.backend.kind != "opencode"
        || task.max_result == 0
        || task.max_result > 512 * 1024
        || task.input.len() > 1024 * 1024
        || !matches!(purpose, "translation" | "continuation")
    {
        return Err("Invalid structured utility task".into());
    }
    let receipt=api.command("/studio/native/utility-session",&json!({"operation":"create","backend":context.backend.id,"sourceThreadId":context.thread,"cwd":context.cwd,"purpose":purpose})).await?;
    let lease = receipt["lease"]
        .as_str()
        .filter(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("Invalid utility session receipt")?
        .to_owned();
    let created = &receipt["session"];
    let id = text(created, "id").to_owned();
    let state = Shared::default();
    let mut attempted = false;
    let result=async{
        check()?;
        if id.is_empty()||id.len()>256||id.chars().any(char::is_control)||id==context.thread||created["directory"].as_str()!=Some(context.cwd.as_str()){return Err("The backend did not return an isolated utility identity".into());}
        let mut params=json!({"threadId":id,"cwd":context.cwd,"input":[{"type":"text","text":task.input}],"developerInstructions":task.instructions,"outputSchema":task.schema});
        if !task.model.is_empty(){params["model"]=task.model.clone().into();}
        attempted=true;
        let turn=rpc(api,&state,"turn/start",params).await?;
        let turn_id=turn.pointer("/turn/id").and_then(Value::as_str).filter(|id|!id.is_empty()&&id.len()<=256&&!id.chars().any(char::is_control)).ok_or("OpenCode accepted a utility without a message identity")?.to_owned();
        tokio::time::timeout(Duration::from_secs(150),async{loop{
            check()?;
            let value=rpc(api,&state,"thread/read",json!({"threadId":id,"cwd":context.cwd,"includeTurns":true})).await?;
            if value.pointer("/thread/id").and_then(Value::as_str)!=Some(id.as_str()){return Err("Utility result belongs to a different conversation".into());}
            let turn=value.pointer("/thread/turns").and_then(Value::as_array).into_iter().flatten().find(|t|t["id"].as_str()==Some(turn_id.as_str()));
            if let Some(turn)=turn{
                if turn["status"]=="completed"{
                    let mut output=String::new();
                    for text in turn["items"].as_array().into_iter().flatten().filter(|i|i["type"]=="agentMessage").filter_map(|i|i["text"].as_str()){
                        if output.len().saturating_add(text.len()).saturating_add(1)>task.max_result{return Err("Utility task output exceeds its budget".into());}
                        if !output.is_empty(){output.push('\n');}output.push_str(text);
                    }
                    return Ok(output);
                }
                if matches!(turn["status"].as_str(),Some("failed"|"interrupted")){return Err("OpenCode utility failed or was interrupted".into());}
            }
            tokio::time::sleep(Duration::from_millis(350)).await;
        }}).await.map_err(|_|"Utility task timed out")?
    }.await;
    let cleanup = tokio::time::timeout(
        Duration::from_secs(15),
        api.command(
            "/studio/native/utility-session",
            &json!({"operation":"close","lease":lease,"interrupt":attempted&&result.is_err()}),
        ),
    )
    .await;
    let warning = match cleanup {
        Ok(Ok(_)) => None,
        _ => Some("Utility task cleanup was not confirmed".into()),
    };
    match result {
        Ok(text) => Ok(crate::translation::StructuredResult { text, warning }),
        Err(error) => Err(match warning {
            Some(warning) => format!("{error}; {warning}"),
            None => error,
        }),
    }
}
