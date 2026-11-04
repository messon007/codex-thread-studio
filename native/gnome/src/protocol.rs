use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};

use crate::{gateway::Api, model::Backend};
#[path = "protocol_reject.rs"]
mod reject;
#[cfg(test)]
#[path = "protocol_supervision_tests.rs"]
mod supervision_tests;

#[derive(Debug, Default)]
pub struct Prepared {
    pub catalog: Option<Result<crate::agents::Page, String>>,
    pub thread_summary: Option<Result<Arc<crate::agents::Agent>, String>>,
    pub mcp_page: Option<Result<crate::mcp::Page, String>>,
    pub command_data: Option<crate::command_data::Update>,
    pub active_shell: bool,
    pub shell_item: bool,
    pub reply_items: Vec<Arc<crate::model::Item>>,
    pub tools: Option<Arc<Vec<Arc<crate::composer_refs::Suggestion>>>>,
    pub interaction_details: Option<String>,
    pub interaction: Option<Result<Arc<crate::interaction::Request>, String>>,
    pub approval: Option<Result<Arc<crate::approval::Request>, String>>,
    pub history: Option<Vec<Arc<crate::model::Item>>>,
    pub active_turn: Option<String>,
    pub item: Option<Arc<crate::model::Item>>,
}

#[derive(Debug)]
pub struct ReplyError {
    pub code: i32,
    pub message: String,
}
impl From<String> for ReplyError {
    fn from(message: String) -> Self {
        Self {
            code: -32601,
            message,
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Continue {
        ticket: u64,
        request: crate::continuation::Request,
        cancel: Arc<AtomicBool>,
    },
    MessageFlow {
        serial: u64,
        request: crate::message_flow::Request,
    },
    Supervision {
        backend: String,
        generation: u64,
        request: crate::turn_supervision_host::Request,
    },
    MapDeleted {
        thread: String,
    },
    SessionMap {
        serial: u64,
        work: crate::session_map_ai::Work,
        stop: Arc<std::sync::atomic::AtomicBool>,
    },
    Speech {
        id: u64,
        text: String,
        language: crate::speech::Language,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    },
    Translate {
        ticket: u64,
        context: crate::translation::Context,
        text: String,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    },
    TranslationSettings {
        dialog: u64,
        serial: u64,
        request: crate::translation_settings::Request,
    },
    BackendConfig {
        dialog: u64,
        serial: u64,
        request: crate::backend_settings::Request,
    },
    Connections {
        dialog: u64,
        serial: u64,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    },
    Streaming(Option<crate::streaming::Request>),
    ConversationOutput(Option<crate::conversation_output::Request>),
    OutputExport {
        dialog: u64,
        serial: u64,
        source: crate::conversation_output::Reference,
        path: std::path::PathBuf,
    },
    ConversationPage(Option<crate::conversation_pages::Request>),
    NotificationSettings {
        serial: u64,
        request: crate::notifications::Request,
    },
    Language {
        serial: u64,
        request: crate::language::Request,
    },
    SessionVisibility {
        dialog: u64,
        serial: u64,
        request: crate::session_visibility::Request,
    },
    Annotations {
        dialog: u64,
        serial: u64,
        request: crate::annotations::Request,
    },
    Epub {
        generation: u64,
        serial: u64,
        kind: &'static str,
        body: Value,
    },
    Pdf {
        generation: u64,
        serial: u64,
        kind: &'static str,
        body: Value,
    },
    NativeImage {
        id: u64,
        source: crate::images::Source,
        cancel: Arc<tokio::sync::Notify>,
    },
    ImageExport {
        id: u64,
        source: crate::images::Source,
        identity: crate::images::Identity,
        path: std::path::PathBuf,
    },
    Table {
        generation: u64,
        serial: u64,
        kind: &'static str,
        body: Value,
    },
    Router {
        serial: u64,
        request: crate::router::Request,
    },
    RouterConfigCancel {
        dialog: u64,
    },
    RouterConfig {
        dialog: u64,
        serial: u64,
        request: crate::router_config::Request,
    },
    AgentTree {
        dialog: u64,
        revision: u64,
        discovery: crate::agents::Discovery,
    },
    CommandPalette {
        dialog: u64,
        serial: u64,
        query: String,
        opencode: bool,
    },
    FilterTools {
        serial: u64,
        sources: Vec<Arc<Vec<Arc<crate::composer_refs::Suggestion>>>>,
        query: String,
    },
    Images {
        key: String,
        serial: u64,
        source: crate::attachments::Source,
    },
    Queue {
        key: String,
        serial: u64,
        request: crate::message_queue::Request,
    },
    Favorites {
        dialog: u64,
        serial: u64,
        request: crate::favorites::Request,
    },
    ConversationFocus(Option<crate::conversation_focus::Request>),
    SessionSearch(Option<crate::session_search::Request>),
    SessionResources(Option<crate::session_resources::Request>),
    SessionInfo {
        dialog: u64,
        serial: u64,
        request: crate::session_info::Request,
    },
    Preferences(Value),
    BrowserTabs {
        serial: u64,
        request: crate::browser_tabs::Request,
    },
    BrowserSettings {
        dialog: u64,
        serial: u64,
        request: crate::browser_settings::Request,
    },
    Appearance {
        dialog: u64,
        serial: u64,
        request: crate::appearance::Request,
    },
    SharedDirectories {
        dialog: u64,
        serial: u64,
        request: crate::shared_directories::Request,
    },
    SessionOptions {
        key: String,
        request: u64,
        options: Option<crate::turn_options::Options>,
    },
    SupervisionOptions {
        serial: u64,
        key: String,
    },
    Workspace {
        panel: u8,
        generation: u64,
        serial: u64,
        request: crate::workspace::Request,
    },
    Terminal(crate::terminal::Control),
    FilesWatch(Option<crate::files_watch::Subscription>),
    Connect {
        backend: Backend,
        generation: u64,
    },
    Rpc {
        images: Vec<Arc<crate::attachments::Image>>,
        backend: String,
        generation: u64,
        id: u64,
        method: String,
        params: Value,
        root: Option<String>,
    },
    Reply {
        id: Value,
        result: Result<Value, ReplyError>,
        backend: String,
        generation: u64,
    },
    Http {
        tag: String,
        method: reqwest::Method,
        path: String,
        body: Option<Value>,
        document: Option<String>,
    },
    Discover {
        generation: u64,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum Event {
    TurnNavigation(crate::turn_navigation::Response),
    Continuation {
        ticket: u64,
        result: Result<crate::continuation::Outcome, String>,
    },
    MessageFlow {
        serial: u64,
        result: Result<crate::message_flow::Snapshot, String>,
    },
    Supervision {
        backend: String,
        generation: u64,
        update: crate::turn_supervision_host::Update,
    },
    GatewayAvailable {
        generation: u64,
    },
    GatewayStopped(String),
    Startup {
        generation: u64,
        update: crate::startup::Update,
    },
    MapStoreChanged {
        backend: String,
        generation: u64,
        thread: String,
        message: String,
    },
    MapSync {
        backend: String,
        generation: u64,
        thread: String,
        result: Result<Option<Arc<crate::session_map::Snapshot>>, crate::session_map::Failure>,
        message: String,
    },
    SessionMap {
        serial: u64,
        result: Result<crate::session_map_ai::Reply, crate::session_map::Failure>,
    },
    Speech {
        id: u64,
        result: Result<crate::speech::Outcome, String>,
    },
    Translation {
        ticket: u64,
        update: crate::translation::Update,
    },
    TranslationSettings {
        dialog: u64,
        serial: u64,
        result: Result<crate::translation_settings::Reply, String>,
    },
    BackendConfig {
        dialog: u64,
        serial: u64,
        result: Result<crate::backend_settings::Reply, String>,
    },
    Language {
        serial: u64,
        result: Result<crate::language::Snapshot, String>,
    },
    Streaming(crate::streaming::Response),
    BrowserTabs {
        serial: u64,
        result: Result<crate::browser_tabs::Snapshot, String>,
    },
    BrowserSettings {
        dialog: u64,
        serial: u64,
        result: Result<crate::browser_settings::Snapshot, String>,
    },
    ConversationOutput(crate::conversation_output::Response),
    OutputExport {
        dialog: u64,
        serial: u64,
        result: Result<(), String>,
    },
    ConversationPage(crate::conversation_pages::Response),
    NotificationSettings {
        serial: u64,
        result: Result<crate::notifications::Snapshot, String>,
    },
    SessionRows {
        serial: u64,
        result: Result<Vec<crate::model::Thread>, String>,
    },
    SessionVisibility {
        dialog: u64,
        serial: u64,
        result: Result<crate::session_visibility::Reply, String>,
    },
    SharedDirectories {
        dialog: u64,
        serial: u64,
        result: Result<crate::shared_directories::Snapshot, String>,
    },
    Appearance {
        dialog: u64,
        serial: u64,
        result: Result<crate::appearance::Reply, String>,
    },
    Annotations {
        dialog: u64,
        serial: u64,
        result: Result<crate::annotations::Reply, String>,
    },
    Epub(crate::epub::Response),
    Pdf(crate::pdf::Response),
    NativeImage {
        id: u64,
        result: Result<crate::images::Frame, String>,
    },
    ImageExport {
        id: u64,
        result: Result<(), String>,
    },
    Table(crate::tables::Response),
    Router {
        serial: u64,
        result: Result<crate::router::Reply, String>,
    },
    RouterConfig {
        dialog: u64,
        serial: u64,
        result: Result<crate::router_config::Reply, String>,
    },
    AgentTree {
        dialog: u64,
        revision: u64,
        result: Result<crate::agents::Tree, String>,
    },
    CommandPalette {
        dialog: u64,
        serial: u64,
        result: Result<Vec<Arc<crate::composer_refs::Suggestion>>, String>,
    },
    FilterTools {
        serial: u64,
        result: Result<Vec<Arc<crate::composer_refs::Suggestion>>, String>,
    },
    Images {
        key: String,
        serial: u64,
        result: Result<Vec<Arc<crate::attachments::Image>>, String>,
    },
    Queue {
        key: String,
        serial: u64,
        result: Result<crate::message_queue::Reply, String>,
    },
    Favorites {
        dialog: u64,
        serial: u64,
        result: Result<crate::favorites::Reply, String>,
    },
    ConversationFocus(crate::conversation_focus::Response),
    SessionSearch(crate::session_search::Response),
    SessionResources(crate::session_resources::Response),
    SessionInfo {
        dialog: u64,
        serial: u64,
        result: Result<crate::session_info::Reply, crate::session_info::Failure>,
    },
    Connections {
        dialog: u64,
        serial: u64,
        result: Result<crate::connections::Snapshot, String>,
    },
    Environment {
        tag: String,
        result: Result<crate::environment::Profile, String>,
    },
    Workspace(crate::workspace::Response),
    FilesWatch {
        generation: u64,
        update: crate::files_watch::Update,
    },
    Terminal {
        generation: u64,
        update: crate::terminal::Update,
    },
    Document(crate::document::Response),
    Backends(Vec<Backend>),
    Wire {
        backend: String,
        generation: u64,
        payload: Value,
        prepared: Prepared,
    },
    Http {
        tag: String,
        result: Result<Value, String>,
    },
    Error(String),
    BackendDisconnected {
        backend: String,
        generation: u64,
        error: String,
    },
    BackendError {
        backend: String,
        generation: u64,
        error: String,
    },
}

struct BackendConnection {
    kind: String,
    generation: u64,
    sender: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<()>,
    closing: Arc<AtomicBool>,
}

struct SettingsRequest {
    tag: String,
    method: reqwest::Method,
    path: String,
    body: Option<Value>,
}
fn settings_event(tag: String, result: Result<Value, String>) -> Event {
    if let Some((serial, key)) = tag
        .strip_prefix("native-queue-prepare:")
        .and_then(|value| value.split_once(':'))
    {
        let serial = serial.parse().unwrap_or(0);
        Event::Queue {
            key: key.into(),
            serial,
            result: result.and_then(|value| {
                serde_json::from_value(value)
                    .map(crate::message_queue::Reply::Prepared)
                    .map_err(|_| "Invalid queued turn options".into())
            }),
        }
    } else {
        Event::Http { tag, result }
    }
}
async fn queue_settings(
    sender: &mpsc::Sender<SettingsRequest>,
    events: &async_channel::Sender<Event>,
    request: SettingsRequest,
) {
    if let Err(error) = sender.try_send(request) {
        let request = error.into_inner();
        let _ = events
            .send(settings_event(
                request.tag,
                Err("Native settings queue is full; the change was not saved".into()),
            ))
            .await;
    }
}

pub fn start(
    runtime: &tokio::runtime::Runtime,
) -> (
    mpsc::Sender<Command>,
    mpsc::Sender<crate::document::Message>,
    async_channel::Receiver<Event>,
    tokio::sync::watch::Sender<Option<crate::session_list::Request>>,
    tokio::sync::watch::Sender<crate::gateway_launch::Control>,
    tokio::sync::watch::Sender<Option<crate::turn_navigation::Request>>,
) {
    let (commands, rx) = mpsc::channel(64);
    let (documents, document_rx) = mpsc::channel(256);
    let (events, output) = async_channel::bounded(256);
    let (session_rows, session_row_requests) = tokio::sync::watch::channel(None);
    let (turn_rows, turn_requests) = tokio::sync::watch::channel(None);
    runtime.spawn(crate::turn_navigation::run(turn_requests, events.clone()));
    let (launch_control, launches) =
        tokio::sync::watch::channel(crate::gateway_launch::Control::Attempt(0));
    runtime.spawn(async move {
        if let Err(error) = run(
            rx,
            document_rx,
            events.clone(),
            session_row_requests,
            launches,
        )
        .await
        {
            let _ = events.send(Event::Error(error)).await;
        }
    });
    (
        commands,
        documents,
        output,
        session_rows,
        launch_control,
        turn_rows,
    )
}

async fn run(
    mut commands: mpsc::Receiver<Command>,
    documents: mpsc::Receiver<crate::document::Message>,
    events: async_channel::Sender<Event>,
    session_row_requests: tokio::sync::watch::Receiver<Option<crate::session_list::Request>>,
    mut launch_control: tokio::sync::watch::Receiver<crate::gateway_launch::Control>,
) -> Result<(), String> {
    let Some((gateway, startup_generation)) =
        crate::gateway_launch::run(&mut launch_control, &events).await
    else {
        return Ok(());
    };
    let (document_api, document_control) = tokio::sync::watch::channel(None);
    let document_task = tokio::spawn(crate::document::run_recoverable(
        document_control,
        documents,
        events.clone(),
    ));
    let mut next = Some((gateway, startup_generation));
    let mut replacement = false;
    let result = async {
        while let Some((gateway, generation)) = next.take() {
            if replacement && !reject_launch_backlog(&mut commands, &events).await {
                gateway.shutdown().await;
                break;
            }
            document_api.send_replace(Some(gateway.api.clone()));
            let _ = events.send(Event::GatewayAvailable { generation }).await;
            if !run_gateway(
                gateway,
                generation,
                &mut commands,
                &events,
                session_row_requests.clone(),
                &mut launch_control,
                &document_api,
            )
            .await?
            {
                break;
            }
            // Reap and settle the departed generation before accepting an
            // explicit replacement. Ordinary offline work is rejected, never
            // retained for replay against new credentials.
            if !wait_for_gateway_restart(&mut commands, &mut launch_control, &events).await {
                break;
            }
            next = crate::gateway_launch::run(&mut launch_control, &events).await;
            replacement = true;
        }
        Ok(())
    }
    .await;
    document_task.abort();
    let _ = document_task.await;
    result
}

async fn reject_launch_backlog(
    commands: &mut mpsc::Receiver<Command>,
    events: &async_channel::Sender<Event>,
) -> bool {
    while let Ok(command) = commands.try_recv() {
        if matches!(command, Command::Shutdown) {
            return false;
        }
        if let Some(event) = reject::command(
            command,
            "Gateway was unavailable when this request was queued; it was not replayed",
        ) {
            let _ = events.send(event).await;
        }
    }
    !commands.is_closed()
}

async fn wait_for_gateway_restart(
    commands: &mut mpsc::Receiver<Command>,
    control: &mut tokio::sync::watch::Receiver<crate::gateway_launch::Control>,
    events: &async_channel::Sender<Event>,
) -> bool {
    loop {
        if matches!(*control.borrow(), crate::gateway_launch::Control::Close) {
            return false;
        }
        if control.has_changed().unwrap_or(false) {
            return true;
        }
        tokio::select! {
            changed = control.changed() => return changed.is_ok() && !matches!(*control.borrow(), crate::gateway_launch::Control::Close),
            command = commands.recv() => match command {
                None | Some(Command::Shutdown) => return false,
                Some(command) => if let Some(event) = reject::command(command, "Gateway stopped; use Reconnect to start a replacement") { let _ = events.send(event).await; },
            }
        }
    }
}

#[cfg(test)]
mod gateway_restart_tests {
    use super::*;
    #[tokio::test]
    async fn offline_admission_rejects_rpc_without_replay_and_waits_for_explicit_attempt() {
        let (sender, mut commands) = mpsc::channel(8);
        let (control, mut attempts) =
            tokio::sync::watch::channel(crate::gateway_launch::Control::Attempt(0));
        let (events, output) = async_channel::bounded(8);
        let task = tokio::spawn(async move {
            let restart = wait_for_gateway_restart(&mut commands, &mut attempts, &events).await;
            (restart, commands)
        });
        sender
            .send(Command::Rpc {
                backend: "old".into(),
                generation: 7,
                id: 19,
                method: "turn/start".into(),
                params: json!({"text":"never replay"}),
                root: None,
                images: Vec::new(),
            })
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), output.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(event,Event::Wire{backend,generation:7,payload,..} if backend=="old" && payload["id"]==19 && payload.get("error").is_some())
        );
        assert!(!task.is_finished());
        control.send_replace(crate::gateway_launch::Control::Attempt(1));
        let (restart, mut commands) = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(restart);
        assert!(matches!(
            commands.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }
    #[tokio::test]
    async fn close_does_not_depend_on_space_in_the_offline_command_queue() {
        let (sender, mut commands) = mpsc::channel(1);
        sender.try_send(Command::Shutdown).unwrap();
        let (control, mut attempts) =
            tokio::sync::watch::channel(crate::gateway_launch::Control::Attempt(0));
        control.send_replace(crate::gateway_launch::Control::Close);
        let (events, _) = async_channel::bounded(1);
        assert!(!tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_gateway_restart(&mut commands, &mut attempts, &events)
        )
        .await
        .unwrap());
    }

    #[tokio::test]
    async fn requests_queued_during_replacement_launch_are_rejected_before_admission() {
        let (sender, mut commands) = mpsc::channel(2);
        sender
            .try_send(Command::Http {
                tag: "captured-old-dialog".into(),
                method: reqwest::Method::PATCH,
                path: "/studio/environment".into(),
                body: Some(json!({"never":"replay"})),
                document: None,
            })
            .unwrap();
        let (events, output) = async_channel::bounded(2);
        assert!(reject_launch_backlog(&mut commands, &events).await);
        assert!(
            matches!(output.recv().await.unwrap(),Event::Http{tag,result:Err(error)} if tag=="captured-old-dialog" && error.contains("not replayed"))
        );
        assert!(matches!(
            commands.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        sender.try_send(Command::Shutdown).unwrap();
        assert!(!reject_launch_backlog(&mut commands, &events).await);
    }
}

async fn run_gateway(
    mut gateway: crate::gateway::ManagedGateway,
    startup_generation: u64,
    commands: &mut mpsc::Receiver<Command>,
    events: &async_channel::Sender<Event>,
    session_row_requests: tokio::sync::watch::Receiver<Option<crate::session_list::Request>>,
    launch_control: &mut tokio::sync::watch::Receiver<crate::gateway_launch::Control>,
    document_api: &tokio::sync::watch::Sender<Option<Api>>,
) -> Result<bool, String> {
    let session_rows_task = tokio::spawn(crate::session_list::run(
        session_row_requests,
        events.clone(),
    ));
    let (preferences, mut preference_commands) = mpsc::channel::<SettingsRequest>(32);
    let api = gateway.api.clone();
    let preference_events = events.clone();
    let mut preference_task = tokio::spawn(async move {
        while let Some(request) = preference_commands.recv().await {
            let result = api
                .json(request.method, &request.path, request.body.as_ref())
                .await;
            let _ = preference_events
                .send(settings_event(request.tag, result))
                .await;
        }
    });
    let (search, search_commands) = tokio::sync::watch::channel(None);
    let search_task = tokio::spawn(crate::session_search::run(
        gateway.api.clone(),
        search_commands,
        events.clone(),
    ));
    let (focus, focus_commands) = tokio::sync::watch::channel(None);
    let (resources, resource_commands) = tokio::sync::watch::channel(None);
    let resources_task = tokio::spawn(crate::session_resources::run(
        gateway.api.clone(),
        resource_commands,
        events.clone(),
    ));
    let focus_task = tokio::spawn(crate::conversation_focus::run(
        gateway.api.clone(),
        focus_commands,
        events.clone(),
    ));
    let (history_pages, history_page_commands) = tokio::sync::watch::channel(None);
    let (outputs, output_commands) = tokio::sync::watch::channel(None);
    let (streaming, streaming_commands) = tokio::sync::watch::channel(None);
    let streaming_task = tokio::spawn(crate::streaming::run(streaming_commands, events.clone()));
    let output_task = tokio::spawn(crate::conversation_output::run(
        output_commands,
        events.clone(),
    ));
    let history_pages_task = tokio::spawn(crate::conversation_pages::run(
        gateway.api.clone(),
        history_page_commands,
        events.clone(),
    ));
    let mut gateway_stopped = false;
    let result = async {
        let mut startup_task = tokio::spawn(crate::startup::run(gateway.api.clone(), events.clone(), startup_generation));
        let mut connections:HashMap<String,BackendConnection>=HashMap::new();
        let mut terminal_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut files_watch_task:Option<tokio::task::JoinHandle<()>>=None;
        let mut terminal_sender: Option<mpsc::Sender<crate::terminal::Control>> = None;
        let mut jobs = tokio::task::JoinSet::new();
        let mut translations=tokio::task::JoinSet::new();
        let mut speech_jobs=tokio::task::JoinSet::new();
        let mut speech_cancel:Option<(u64,Arc<std::sync::atomic::AtomicBool>)>=None;
        let speech_busy=Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut translation_cancels:HashMap<u64,Arc<std::sync::atomic::AtomicBool>>=HashMap::new();
        let mut router_jobs:HashMap<u64,Vec<tokio::task::AbortHandle>>=HashMap::new();
        let mut launch_controls_open = true;
        loop {
            let command = tokio::select! {
                result = gateway.wait() => {
                    gateway_stopped = true;
                    gateway.api.retire();
                    document_api.send_replace(None);
                    launch_control.borrow_and_update();
                    let error = match result {
                        Ok(status) => format!("Gateway process exited ({status}); unfinished requests were not replayed"),
                        Err(error) => format!("Gateway process wait failed: {error}"),
                    };
                    let _ = events.send(Event::GatewayStopped(error.clone())).await;
                    while let Ok(command) = commands.try_recv() {
                        if let Some(event) = reject::command(command, &error) { let _ = events.send(event).await; }
                    }
                    break;
                }
                changed = launch_control.changed(), if launch_controls_open => {
                    if changed.is_err() { launch_controls_open = false; continue; }
                    match *launch_control.borrow_and_update() {
                        crate::gateway_launch::Control::Attempt(generation) => Command::Discover { generation },
                        // Once running, retain ordered preference writes before
                        // the normal Shutdown command. The independent Close
                        // signal is only needed before a gateway exists.
                        crate::gateway_launch::Control::Close => continue,
                    }
                }
                command = commands.recv() => match command { Some(command) => command, None => break },
            };
            match command {
                Command::MapDeleted{..}=>{}, // internal backend-socket control only
                Command::Speech{id,text,language,cancel}=>{
                    while let Some(Ok(finished))=speech_jobs.try_join_next() { if speech_cancel.as_ref().is_some_and(|(id,_)|*id==finished){speech_cancel=None;} }
                    if speech_busy.load(std::sync::atomic::Ordering::Acquire){let _=events.send(Event::Speech{id,result:Err("Speech playback is already active; stop it first".into())}).await;}
                    else{speech_cancel=Some((id,cancel.clone()));speech_busy.store(true,std::sync::atomic::Ordering::Release);let busy=speech_busy.clone();let events=events.clone();speech_jobs.spawn(async move{let result=crate::speech::run(text,language,cancel).await;busy.store(false,std::sync::atomic::Ordering::Release);let _=events.send(Event::Speech{id,result}).await;id});}
                }
                Command::Translate{ticket,context,text,cancel}=>{
                    while let Some(Ok(ticket))=translations.try_join_next(){translation_cancels.remove(&ticket);}
                    if translations.len()>=2 || translation_cancels.contains_key(&ticket){let _=events.send(Event::Translation{ticket,update:crate::translation::Update::Done(Err("Two translation tasks are already active; close one and wait for cleanup".into()))}).await;}
                    else{translation_cancels.insert(ticket,cancel.clone());let api=gateway.api.clone();let events=events.clone();translations.spawn(async move{let result=crate::translation::execute(api,ticket,context,text,cancel,events.clone()).await;let _=events.send(Event::Translation{ticket,update:crate::translation::Update::Done(result)}).await;ticket});}
                }
                Command::Continue{ticket,request,cancel}=>{
                    while let Some(Ok(ticket))=translations.try_join_next(){translation_cancels.remove(&ticket);}
                    if translations.len()>=2 || translation_cancels.contains_key(&ticket){let _=events.send(Event::Continuation{ticket,result:Err("Two utility tasks are already active; wait for cleanup".into())}).await;}
                    else{translation_cancels.insert(ticket,cancel.clone());let api=gateway.api.clone();let events=events.clone();translations.spawn(async move{let result=crate::continuation::execute(&api,ticket,request,&cancel).await;let _=events.send(Event::Continuation{ticket,result}).await;ticket});}
                }
                Command::TranslationSettings{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::TranslationSettings{dialog,serial,result:Err("Native settings workers are busy; retry translation settings".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::translation_settings::execute(&api,request).await;let _=events.send(Event::TranslationSettings{dialog,serial,result}).await;});}
                }
                Command::MessageFlow{serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::MessageFlow{serial,result:Err("Native settings workers are busy; retry message flow".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::message_flow::execute(&api,request).await;let _=events.send(Event::MessageFlow{serial,result}).await;});}
                }

                Command::Streaming(request)=>{streaming.send_replace(request);}
                Command::Router{serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8 {let _=events.send(Event::Router{serial,result:Err("Native workers are busy; try again".into())}).await;}
                    else {let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move {let result=crate::router::execute(&api,request).await;let _=events.send(Event::Router{serial,result}).await;});}
                }
                Command::RouterConfigCancel{dialog}=>{if let Some(handles)=router_jobs.remove(&dialog){for handle in handles{handle.abort();}}}
                Command::RouterConfig{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::RouterConfig{dialog,serial,result:Err("Native workers are busy; try again".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();let handle=jobs.spawn(async move{let result=crate::router_config::execute(&api,request).await;let _=events.send(Event::RouterConfig{dialog,serial,result}).await;});let handles=router_jobs.entry(dialog).or_default();handles.retain(|handle|!handle.is_finished());handles.push(handle);}
                }
                Command::AgentTree{dialog,revision,discovery}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8 {let _=events.send(Event::AgentTree{dialog,revision,result:Err("Native workers are busy; reload the agent tree".into())}).await;}
                    else{let events=events.clone();jobs.spawn(async move{let result=crate::worker::compute(move||discovery.tree()).await;let _=events.send(Event::AgentTree{dialog,revision,result}).await;});}
                }
                Command::CommandPalette{dialog,serial,query,opencode}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::CommandPalette{dialog,serial,result:Err("Native workers are busy; try again".into())}).await;}else{
                        let events=events.clone();jobs.spawn(async move{let result=crate::worker::compute(move||Ok(crate::composer_refs::filter(vec![crate::command_tools::catalog(opencode)],&query))).await;let _=events.send(Event::CommandPalette{dialog,serial,result}).await;});
                    }
                }
                Command::FilterTools{serial,sources,query}=> {
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8 {let _=events.send(Event::FilterTools{serial,result:Err("Search workers are busy; try again".into())}).await;} else {
                        let events=events.clone();jobs.spawn(async move {let result=crate::worker::compute(move||Ok(crate::composer_refs::filter(sources,&query))).await;let _=events.send(Event::FilterTools{serial,result}).await;});
                    }
                }
                Command::Images {key,serial,source} => {
                    while jobs.try_join_next().is_some() {}
                    if jobs.len() >= 8 {
                        let _=events.send(Event::Images{key,serial,result:Err("Native workers are busy; try attaching again".into())}).await;
                    } else {
                        let events=events.clone();
                        jobs.spawn(async move {
                            let result=crate::worker::compute(move || crate::attachments::prepare(source,serial)).await;
                            let _=events.send(Event::Images{key,serial,result}).await;
                        });
                    }
                }
                Command::ConversationFocus(request) => { focus.send_replace(request); }
                Command::ConversationPage(request) => { history_pages.send_replace(request); }
                Command::ConversationOutput(request) => {outputs.send_replace(request);}
                Command::OutputExport{dialog,serial,source,path}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8 {let _=events.send(Event::OutputExport{dialog,serial,result:Err("Native workers are busy; try exporting again".into())}).await;}
                    else {let plan=crate::conversation_output::prepare_export(source,path.clone());let events=events.clone();jobs.spawn(async move{#[cfg(debug_assertions)] crate::gateway_test_fault::export_latch(&path).await;let result=crate::worker::compute(move||plan?.write()).await;let _=events.send(Event::OutputExport{dialog,serial,result}).await;});}
                }
                Command::NotificationSettings{serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::NotificationSettings{serial,result:Err("Too many requests; reload notification settings".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::notifications::execute(&api,request).await;let _=events.send(Event::NotificationSettings{serial,result}).await;});}
                }
                Command::Language{serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::Language{serial,result:Err("Language settings are unavailable".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::language::execute(&api,request).await;let _=events.send(Event::Language{serial,result}).await;});}
                }
                Command::BackendConfig{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::BackendConfig{dialog,serial,result:Err("Backend configuration workers are busy; retry".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::backend_settings::execute(&api,request).await;let _=events.send(Event::BackendConfig{dialog,serial,result}).await;});}
                }
                Command::SessionVisibility{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8 {let _=events.send(Event::SessionVisibility{dialog,serial,result:Err("Too many requests; retry session visibility settings".into())}).await;}
                    else {let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::session_visibility::execute(&api,request).await;let _=events.send(Event::SessionVisibility{dialog,serial,result}).await;});}
                }
                Command::SessionSearch(request) => { search.send_replace(request); }
                Command::SessionResources(request) => { resources.send_replace(request); }
                Command::SessionInfo {dialog,serial,request} => {
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8 {let _=events.send(Event::SessionInfo {dialog,serial,result:Err(crate::session_info::Failure {message:"Session information workers are busy; retry".into(),reload_required:false,http_status:None})}).await;}
                    else {let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::session_info::execute(&api,request).await;let _=events.send(Event::SessionInfo {dialog,serial,result}).await;});}
                }

                Command::Connections{dialog,serial,cancel} => {
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::Connections{dialog,serial,result:Err("Connection information workers are busy; retry".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::connections::read(&api,cancel).await;let _=events.send(Event::Connections{dialog,serial,result}).await;});}
                }
                Command::Preferences(patch) => {
                    queue_settings(&preferences,&events,SettingsRequest{tag:"native-preferences-save".into(),method:reqwest::Method::PATCH,path:"/studio/native/preferences".into(),body:Some(patch)}).await;
                }
                Command::Appearance{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::Appearance{dialog,serial,result:Err("Too many requests; retry appearance settings".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::appearance::execute(&api,request).await;let _=events.send(Event::Appearance{dialog,serial,result}).await;});}
                }
                Command::BrowserTabs{serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::BrowserTabs{serial,result:Err("Too many requests; retry opening Browser".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::browser_tabs::execute(&api,request).await;let _=events.send(Event::BrowserTabs{serial,result}).await;});}
                }
                Command::BrowserSettings{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::BrowserSettings{dialog,serial,result:Err("Too many requests; retry browser settings".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::browser_settings::execute(&api,request).await;let _=events.send(Event::BrowserSettings{dialog,serial,result}).await;});}
                }
                Command::SharedDirectories{dialog,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::SharedDirectories{dialog,serial,result:Err("Too many requests; retry shared-folder settings".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::shared_directories::execute(&api,request).await;let _=events.send(Event::SharedDirectories{dialog,serial,result}).await;});}
                }
                Command::SessionOptions{key,request,options}=>{
                    let request=if let Some(options)=options {SettingsRequest{tag:"turn-options-save".into(),method:reqwest::Method::PUT,path:"/studio/session-state/turn-options".into(),body:Some(json!({"sessionKey":key,"model":options.model,"effort":options.effort,"serviceTier":options.service_tier}))}}
                    else {let query=url::form_urlencoded::Serializer::new(String::new()).append_pair("sessionKey",&key).finish();SettingsRequest{tag:format!("turn-options-load:{request}"),method:reqwest::Method::GET,path:format!("/studio/session-state/turn-options?{query}"),body:None}};
                    queue_settings(&preferences,&events,request).await;
                }
                Command::SupervisionOptions{serial,key}=>{
                    let query=url::form_urlencoded::Serializer::new(String::new()).append_pair("sessionKey",&key).finish();
                    queue_settings(&preferences,&events,SettingsRequest{tag:format!("native-supervision-options:{serial}"),method:reqwest::Method::GET,path:format!("/studio/session-state/turn-options?{query}"),body:None}).await;
                }
                Command::Queue{key,serial,request:crate::message_queue::Request::Prepare}=>{
                    let query=url::form_urlencoded::Serializer::new(String::new()).append_pair("sessionKey",&key).finish();
                    queue_settings(&preferences,&events,SettingsRequest{tag:format!("native-queue-prepare:{serial}:{key}"),method:reqwest::Method::GET,path:format!("/studio/session-state/turn-options?{query}"),body:None}).await;
                }
                Command::Queue{key,serial,request}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=8{let _=events.send(Event::Queue{key,serial,result:Err("Too many pending requests; try again".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::message_queue::execute(&api,&key,request).await;let _=events.send(Event::Queue{key,serial,result}).await;});}
                }
                Command::Favorites { dialog, serial, request } => {
                    while jobs.try_join_next().is_some() {}
                    if jobs.len() >= 8 {
                        let _ = events.send(Event::Favorites { dialog, serial, result: Err("Too many pending requests; try again".into()) }).await;
                    } else {
                        let api = gateway.api.clone();
                        let events = events.clone();
                        jobs.spawn(async move {
                            let result = crate::favorites::execute(&api, request).await;
                            let _ = events.send(Event::Favorites { dialog, serial, result }).await;
                        });
                    }
                }
                Command::Epub {generation,serial,kind,body}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=if kind=="close"{12}else{8}{let _=events.send(Event::Epub(crate::epub::Response{generation,serial,kind,result:Err("Native workers are busy; try again".into())})).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::epub::execute(&api,kind,body).await;let _=events.send(Event::Epub(crate::epub::Response{generation,serial,kind,result})).await;});}
                }
                Command::Pdf {generation,serial,kind,body}=>{
                    while jobs.try_join_next().is_some(){}
                    if jobs.len()>=if kind=="close"{12}else{8}{let _=events.send(Event::Pdf(crate::pdf::Response{generation,serial,kind,result:Err("Native workers are busy; try again".into())})).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::pdf::execute(&api,kind,body).await;let _=events.send(Event::Pdf(crate::pdf::Response{generation,serial,kind,result})).await;});}
                }
                Command::ImageExport{id,source,identity,path}=>{
                    if jobs.len()>=8 {let _=events.send(Event::ImageExport{id,result:Err("Native workers are busy; try again".into())}).await;}
                    else {
                        let plan=crate::images::prepare_export(source,identity,path);let api=gateway.api.clone();let events=events.clone();
                        jobs.spawn(async move {let result=match plan{Ok(plan)=>crate::images::export(&api,plan).await,Err(error)=>Err(error)};
                            let _=events.send(Event::ImageExport{id,result}).await;
                        });
                    }
                }
                Command::NativeImage {id,source,cancel} => {
                    while jobs.try_join_next().is_some() {}
                    if jobs.len()>=8 {let _=events.send(Event::NativeImage{id,result:Err("Native workers are busy; try again".into())}).await;}
                    else {let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{
                        let result=tokio::select! {biased;
                            _=cancel.notified()=>Err("Image request cancelled".into()),
                            result=crate::images::load_source(&api,source)=>result,
                        };
                        let _=events.send(Event::NativeImage{id,result}).await;
                    });}
                }
                Command::Annotations {dialog,serial,request}=>{
                    while jobs.try_join_next().is_some() {}
                    if jobs.len()>=8{let _=events.send(Event::Annotations{dialog,serial,result:Err("Native workers are busy; try again".into())}).await;}
                    else{let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::annotations::execute(&api,request).await;let _=events.send(Event::Annotations{dialog,serial,result}).await;});}
                }
                Command::SessionMap {serial,work,stop}=>{
                    while jobs.try_join_next().is_some() {}
                    if jobs.len()>=8{let _=events.send(Event::SessionMap{serial,result:Err(crate::session_map::Failure{message:"Native workers are busy; try again".into(),conflict:false,uncertain:false})}).await;}
                    else{
                        let deleted=if let crate::session_map_ai::Work::Store(crate::session_map::Request::Delete{anchor,..})=&work{connections.get(&anchor.backend).map(|c|(c.sender.clone(),anchor.thread.clone()))}else{None};
                        let api=gateway.api.clone();let events=events.clone();jobs.spawn(async move{let result=crate::session_map_ai::execute(&api,serial,work,&stop).await;if result.as_ref().is_ok_and(|r|r.snapshot.is_none()){if let Some((sender,thread))=deleted{let _=sender.try_send(Command::MapDeleted{thread});}}let _=events.send(Event::SessionMap{serial,result}).await;});
                    }
                }
                Command::Table {generation,serial,kind,body}=> {
                    while jobs.try_join_next().is_some() {}
                    if jobs.len()>=if kind=="close"{12}else{8} {let _=events.send(Event::Table(crate::tables::Response{generation,serial,kind,result:Err("Native workers are busy; try again".into())})).await;} else {
                        let api=gateway.api.clone();let events=events.clone();
                        jobs.spawn(async move {let result=crate::tables::execute(&api,kind,body).await;let _=events.send(Event::Table(crate::tables::Response{generation,serial,kind,result})).await;});
                    }
                }
                Command::FilesWatch(subscription)=>{
                    if let Some(task)=files_watch_task.take(){task.abort();let _=task.await;}
                    if let Some(subscription)=subscription{
                        files_watch_task=Some(tokio::spawn(crate::files_watch::run(gateway.api.clone(),subscription,events.clone())));
                    }
                }
                Command::Workspace {
                    panel,
                    generation,
                    serial,
                    request,
                } => {
                    while jobs.try_join_next().is_some() {}
                    let kind = request.kind();
                    if jobs.len() >= 8 {
                        let _ = events
                            .send(Event::Workspace(crate::workspace::Response {
                                panel,
                                generation,
                                serial,
                                kind,
                                result: Err("The workspace is busy; try again".into()),
                            }))
                            .await;
                    } else {
                        let api = gateway.api.clone();
                        let events = events.clone();
                        jobs.spawn(async move {
                            let result = crate::workspace::execute(&api, request).await;
                            let _ = events
                                .send(Event::Workspace(crate::workspace::Response {
                                    panel,
                                    generation,
                                    serial,
                                    kind,
                                    result,
                                }))
                                .await;
                        });
                    }
                }
                Command::Terminal(control) => match control {
                    crate::terminal::Control::Open {
                        root,
                        generation,
                        cols,
                        rows,
                    } => {
                        if let Some(task) = terminal_task.take() {
                            task.abort();
                            let _ = task.await;
                        }
                        let (sender, receiver) = mpsc::channel(64);
                        terminal_sender = Some(sender);
                        let api = gateway.api.clone();
                        let events = events.clone();
                        terminal_task = Some(tokio::spawn(async move {
                            if let Err(error) = crate::terminal::connect(
                                api,
                                root,
                                generation,
                                cols,
                                rows,
                                receiver,
                                events.clone(),
                            )
                            .await
                            {
                                let _ = events
                                    .send(Event::Terminal {
                                        generation,
                                        update: crate::terminal::Update::Error(error),
                                    })
                                    .await;
                            }
                        }));
                    }
                    crate::terminal::Control::Close => {
                        terminal_sender = None;
                        if let Some(task) = terminal_task.take() {
                            task.abort();
                            let _ = task.await;
                        }
                    }
                    control => {
                        if let Some(sender) = &terminal_sender {
                            let generation = match &control {
                                crate::terminal::Control::Input { generation, .. }
                                | crate::terminal::Control::Resize { generation, .. } => {
                                    *generation
                                }
                                _ => 0,
                            };
                            if let Err(error) = sender.try_send(control) {
                                let update = match error {
                                    mpsc::error::TrySendError::Full(_) => {
                                        crate::terminal::Update::Warning(
                                            "Terminal input queue is full; input was not sent"
                                                .into(),
                                        )
                                    }
                                    mpsc::error::TrySendError::Closed(_) => {
                                        crate::terminal::Update::Error(
                                            "Terminal disconnected; input was not sent".into(),
                                        )
                                    }
                                };
                                let _ = events.send(Event::Terminal { generation, update }).await;
                            }
                        }
                    }
                },
                Command::Connect {
                    backend,
                    generation,
                } => {
                    if let Some(connection) = connections.remove(&backend.id) {
                        connection.task.abort();
                        let _=connection.task.await;
                    }
                    let (tx, rx) = mpsc::channel(64);
                    let api = gateway.api.clone();
                    let events = events.clone();
                    let backend_id = backend.id.clone();
                    let connection_id=backend.id.clone();
                    let connection_kind=backend.kind.clone();
                    let closing=Arc::new(AtomicBool::new(false));let socket_closing=closing.clone();
                    let task=tokio::spawn(async move {
                        if let Err(error) =
                            backend_connection(api, backend, generation, rx, events.clone(),socket_closing).await
                        {
                            let _ = events
                                .send(Event::BackendDisconnected {
                                    backend: backend_id,
                                    generation,
                                    error,
                                })
                                .await;
                        }
                    });
                    connections.insert(connection_id,BackendConnection{kind:connection_kind,generation,sender:tx,task,closing});
                }
                Command::Rpc {backend,generation,id,method,mut params,root,images} if !images.is_empty() => {
                    // Codex admits human intervention before potentially costly
                    // attachment preparation, in its connection-owned loop.
                    if let Some(connection)=connections.get(&backend).filter(|c|c.generation==generation&&c.kind=="codex") {
                        if let Err(error)=connection.sender.try_send(Command::Rpc{backend:backend.clone(),generation,id,method,params,root,images}) {
                            let _=events.send(Event::Wire{backend,generation,payload:json!({"id":id,"error":{"message":format!("Backend request was not sent: {error}")}}),prepared:Prepared::default()}).await;
                        }
                        continue;
                    }
                    while jobs.try_join_next().is_some() {}
                    let target=connections.get(&backend).filter(|connection| connection.generation==generation).map(|connection|connection.sender.clone());
                    if jobs.len()>=8 || target.is_none() {
                        let _=events.send(Event::Wire{backend,generation,payload:json!({"id":id,"error":{"message":"Backend is disconnected or busy; attachments were not sent"}}),prepared:Prepared::default()}).await;
                    } else {
                        let target=target.unwrap();let events=events.clone();
                        jobs.spawn(async move {
                            let result=crate::worker::compute(move||{crate::attachments::append(&mut params,images)?;Ok(params)}).await;
                            let error=match result {
                                Ok(params)=>target.try_send(Command::Rpc{backend:backend.clone(),generation,id,method,params,root,images:Vec::new()}).err().map(|_|"Backend changed or its request queue is full; attachments were not sent".to_owned()),
                                Err(error)=>Some(error),
                            };
                            if let Some(error)=error {let _=events.send(Event::Wire{backend,generation,payload:json!({"id":id,"error":{"message":error}}),prepared:Prepared::default()}).await;}
                        });
                    }
                }
                Command::Supervision {backend,generation,request}=>{
                    let command=Command::Supervision{backend:backend.clone(),generation,request};
                    let error=if let Some(connection)=connections.get(&backend).filter(|c|c.generation==generation){
                        connection.sender.try_send(command).err().map(|error|error.into_inner())
                    }else{Some(command)};
                    if let Some(command)=error {if let Some(event)=reject::command(command,"Backend changed or is unavailable; supervision was not started"){let _=events.send(event).await;}}
                }
                Command::Rpc { .. } | Command::Reply { .. } => {
                    let (target,expected)=match &command{Command::Reply{backend,generation,..}|Command::Rpc{backend,generation,..}=>(backend.clone(),*generation),_=>unreachable!()};
                    if let Some(connection)=connections.get(&target){
                        if expected!=connection.generation {
                            if let Command::Rpc{id,..}=command{let _=events.send(Event::Wire{backend:target,generation:expected,payload:json!({"id":id,"error":{"message":"Backend connection changed; request was not sent"}}),prepared:Prepared::default()}).await;}
                            continue;
                        }
                        if let Err(error)=connection.sender.try_send(command){
                            let message="Backend request queue is full or disconnected. Reconnect before retrying.";
                            let event=match error.into_inner(){
                                Command::Rpc{id,..}=>Event::Wire{backend:target.clone(),generation:connection.generation,payload:json!({"id":id,"error":{"message":message}}),prepared:Prepared::default()},
                                _=>Event::BackendError{backend:target.clone(),generation:connection.generation,error:message.into()},
                            };
                            let _=events.send(event).await;
                        }
                    } else if let Command::Rpc{id,..}=command {
                        let _=events.send(Event::Wire{backend:target,generation:expected,payload:json!({"id":id,"error":{"message":"Backend is disconnected; request was not sent"}}),prepared:Prepared::default()}).await;

                }
                    }
                Command::Http {
                    tag,
                    method,
                    path,
                    body,
                    document,
                } => {
                    while jobs.try_join_next().is_some() {}
                    if jobs.len() >= 8 {
                        let _ = events
                            .send(Event::Http {
                                tag,
                                result: Err("Too many pending requests; try again".into()),
                            })
                            .await;
                        continue;
                    }
                    let api = gateway.api.clone();
                    let events = events.clone();
                    jobs.spawn(async move {
                        let result = api
                            .json_document(method, &path, body.as_ref(), document.as_deref())
                            .await;
                        let event = if tag.starts_with("settings:") && !tag.ends_with(":backends") {
                            let result = match result { Ok(value) => crate::worker::compute(move || serde_json::from_value(value).map_err(|_| "The server returned an invalid environment profile".into())).await, Err(error) => Err(error) };
                            Event::Environment { tag, result }
                        } else { Event::Http { tag, result } };
                        let _ = events.send(event).await;
                    });
                }
                Command::Discover { generation } => {
                    startup_task.abort();
                    let _ = (&mut startup_task).await;
                    // Reconnect can advance the UI generation just after
                    // readiness won the launch race. Confirm this already-owned
                    // API for that generation before publishing discovery.
                    let _ = events.send(Event::GatewayAvailable { generation }).await;
                    startup_task = tokio::spawn(crate::startup::run(gateway.api.clone(), events.clone(), generation));
                }
                Command::Shutdown => break,
            }
        }
        startup_task.abort();
        let _ = startup_task.await;
        for connection in connections.values(){connection.closing.store(true,Ordering::Release);let _=connection.sender.try_send(Command::Shutdown);}
        if let Some((_,cancel))=speech_cancel {cancel.store(true,std::sync::atomic::Ordering::Release);}
        for cancel in translation_cancels.values(){cancel.store(true,std::sync::atomic::Ordering::Release);}
        let _=tokio::time::timeout(Duration::from_secs(15),async{while speech_jobs.join_next().await.is_some(){}}).await;
        speech_jobs.abort_all();
        let _=tokio::time::timeout(Duration::from_secs(45),async{while translations.join_next().await.is_some(){}}).await;
        translations.abort_all();
        for mut connection in connections.into_values() {
            if tokio::time::timeout(Duration::from_secs(45),&mut connection.task).await.is_err(){connection.task.abort();let _=connection.task.await;}
        }
        if gateway_stopped {
            // Admitted exports must report their actual outcome. HTTP jobs
            // remain pinned to the departed endpoint; none is replayed.
            while jobs.join_next().await.is_some() {}
        } else { jobs.abort_all(); }
        if let Some(task)=files_watch_task {task.abort();let _=task.await;}
        if let Some(task) = terminal_task {
            task.abort();
            let _ = task.await;
        }
        Ok(())
    }
    .await;
    session_rows_task.abort();
    let _ = session_rows_task.await;
    search_task.abort();
    let _ = search_task.await;
    resources_task.abort();
    let _ = resources_task.await;
    focus_task.abort();
    let _ = focus_task.await;
    history_pages_task.abort();
    let _ = history_pages_task.await;
    output_task.abort();
    let _ = output_task.await;
    streaming_task.abort();
    let _ = streaming_task.await;
    drop(preferences);
    if tokio::time::timeout(Duration::from_secs(3), &mut preference_task)
        .await
        .is_err()
    {
        preference_task.abort();
        eprintln!("Native settings did not finish saving before shutdown");
    }
    gateway.shutdown().await;
    result.map(|()| gateway_stopped)
}

async fn codex_socket(
    api: Api,
    backend: Backend,
    generation: u64,
    commands: mpsc::Receiver<Command>,
    events: async_channel::Sender<Event>,
    closing: Arc<AtomicBool>,
) -> Result<(), String> {
    let service = crate::turn_supervision_runtime::Worker::new(api.clone());
    codex_socket_with_service(api, backend, generation, commands, events, service, closing).await
}
async fn codex_socket_with_service<S: crate::turn_supervision_runtime::Service>(
    api: Api,
    backend: Backend,
    generation: u64,
    mut commands: mpsc::Receiver<Command>,
    events: async_channel::Sender<Event>,
    service: S,
    closing: Arc<AtomicBool>,
) -> Result<(), String> {
    if backend.kind != "codex" {
        return Err("A Codex WebSocket requires a Codex backend".into());
    }
    let request = api.socket_request(&backend.socket_path)?;
    let config = WebSocketConfig::default().max_message_size(Some(32 * 1024 * 1024));
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async_with_config(request, Some(config), false),
    )
    .await
    .map_err(|_| "Backend connection timed out")?
    .map_err(|e| e.to_string())?;
    let (mut output, mut input) = socket.split();
    let mut pending: HashMap<u64, Instant> = HashMap::new();
    let mut selections = HashMap::<
        u64,
        (
            crate::session_hydration::Selection,
            Option<tokio::task::AbortHandle>,
        ),
    >::new();
    let mut histories = tokio::task::JoinSet::new();
    let applied_environments: crate::environment::Applied = Arc::default();
    let mut supervisor = crate::turn_supervision_host::Host::new(
        api.clone(),
        service,
        backend.id.clone(),
        generation,
        applied_environments.clone(),
    );
    let mut maps = crate::session_map_sync::Runtime::new(api.clone(), backend.id.clone());
    let mut map_streams = crate::session_map_text::Streams::default();
    let mut lifecycle = crate::backend_lifecycle::Codex::default();
    let mut expiry = tokio::time::interval(Duration::from_secs(1));
    let mut commands_open = true;
    let result=async { loop {
        if closing.load(Ordering::Acquire){supervisor.begin_shutdown();if !supervisor.runtime.awaiting_ack(){let _=output.close().await;return Ok(());}}
        tokio::select! {
            command = commands.recv(),if commands_open => match command {
                Some(Command::Rpc { id, method, mut params,root,images,.. }) => {
                    if closing.load(Ordering::Acquire){continue;}
                    if matches!(method.as_str(),"turn/start"|"turn/steer"|"turn/interrupt"|"thread/archive"|"thread/delete"|"thread/rollback"|"thread/compact/start"|"thread/shellCommand"|"review/start") {
                        let thread=crate::model::text(&params,"threadId");
                        maps.cancel_guarded(thread);
                        if let Some(id)=supervisor.runtime.unsent(thread){maps.cancel_start(id);}
                        if method=="review/start"{supervisor.runtime.stop(thread,"Stopped by user activity");}
                        else{supervisor.runtime.human_action(thread,&method,&params["input"]);}
                    }
                    if !images.is_empty() {
                        let thread=crate::model::text(&params,"threadId").to_owned();
                        match crate::worker::compute(move||{crate::attachments::append(&mut params,images)?;Ok(params)}).await {
                            Ok(prepared)=>params=prepared,
                            Err(error)=>{supervisor.runtime.stop(&thread,"The human message was not sent");let _=events.send(Event::Wire{backend:backend.id.clone(),generation,payload:json!({"id":id,"error":{"message":error}}),prepared:Prepared::default()}).await;continue;}
                        }
                    }
                    if let Some(selection) = crate::session_hydration::Selection::new(&method,&params) {
                        // A new selection of the same thread supersedes the old
                        // read. Other cached conversations may finish offscreen.
                        let old=selections.iter().filter(|(_, (old,_))|old.thread==selection.thread).map(|(id,_)|*id).collect::<Vec<_>>();
                        for old in old {
                            if let Some((_,Some(task)))=selections.remove(&old) {task.abort();}
                            pending.remove(&old);
                            let _=events.send(Event::Wire{backend:backend.id.clone(),generation,payload:json!({"id":old,"error":{"message":"Session load superseded by a newer read"}}),prepared:Prepared::default()}).await;
                        }
                        if selections.len()>=8 {
                            let _=events.send(Event::Wire{backend:backend.id.clone(),generation,payload:json!({"id":id,"error":{"message":"Too many sessions loading; refresh this session"}}),prepared:Prepared::default()}).await;
                            continue;
                        }
                        params=selection.metadata();
                        selections.insert(id,(selection,None));
                    }
                    if matches!(method.as_str(),"turn/start"|"thread/shellCommand"|"review/start") {
                        if let Some(root)=root {
                            if let Err(error)=crate::environment::prepare(&api,&backend.id,crate::model::text(&params,"threadId"),&root,&mut *applied_environments.lock().await).await {
                                let _=events.send(Event::Wire{backend:backend.id.clone(),generation,payload:json!({"id":id,"error":{"message":error}}),prepared:Prepared::default()}).await;continue;
                            }
                        }
                    }
                    if closing.load(Ordering::Acquire){continue;}
                    if method=="turn/start" { maps.start(id,params);continue; }
                    pending.insert(id, Instant::now());
                    let body=crate::worker::compute(move||serde_json::to_string(&json!({"id":id,"method":method,"params":params})).map_err(|error|error.to_string())).await?;
                    output.send(Message::text(body)).await.map_err(|e| e.to_string())?;
                }
                Some(Command::Reply { id, result,.. }) => {
                    if closing.load(Ordering::Acquire){continue;}
                    let response=match result {
                        Ok(result)=>json!({"id":id,"result":result}),
                        Err(error)=>json!({"id":id,"error":{"code":error.code,"message":error.message}}),
                    };
                    output.send(Message::text(response.to_string())).await.map_err(|e| e.to_string())?;
                }
                Some(Command::MapDeleted{thread})=>maps.map_deleted(thread),
                Some(Command::Supervision{backend:target,generation:expected,request})=>{
                    let valid=target==backend.id&&expected==generation&&match &request{
                        crate::turn_supervision_host::Request::Start(start)=>start.context.backend==backend,
                        _=>true,
                    };
                    if valid {supervisor.request(request);}else{
                        let _=events.send(Event::Supervision{backend:backend.id.clone(),generation,update:crate::turn_supervision_host::Update::Rejected{request:request.request(),thread:request.thread().map(str::to_owned),message:"Supervision context does not match this backend connection".into()}}).await;
                    }
                },
                None | Some(Command::Shutdown) => {commands_open=false;closing.store(true,Ordering::Release);supervisor.begin_shutdown();}
                _ => {}
            },
            incoming = input.next() => match incoming {
                Some(Ok(Message::Text(raw))) => {
                    let mut streams=std::mem::take(&mut map_streams);
                    let (payload,prepared,notices,streams,tool)=crate::worker::compute(move ||{
                        let mut payload:Value=serde_json::from_str(&raw).map_err(|_|"Backend sent invalid JSON".to_owned())?;
                        let notices=streams.process(&mut payload);
                        let tool=crate::session_map_sync::prepare_tool(&payload);
                        let prepared=if tool.is_some(){Prepared::default()}else{prepare_presentation(&mut payload)};
                        Ok((payload,prepared,notices,streams,tool))
                    }).await?;
                    map_streams=streams;
                    match lifecycle.observe(&payload) {
                        crate::backend_lifecycle::Notice::Ignore => continue,
                        crate::backend_lifecycle::Notice::Failed(error) => return Err(error),
                        crate::backend_lifecycle::Notice::Gap(skipped) => {
                            let count=skipped.map(|n|n.to_string()).unwrap_or_else(||"?".into());
                            return Err(crate::language::format("The UI missed {count} App Server events", &[("count", &count)]));
                        }
                        crate::backend_lifecycle::Notice::Forward => {}
                    }
                    if payload["method"]=="studio/sessionMap/changed"{
                        if payload["params"]["backend"].as_str()==Some(backend.id.as_str()){
                            if let (Some(thread),Some(message))=(payload["params"]["threadId"].as_str().filter(|s|!s.is_empty()&&s.len()<=256),payload["params"]["message"].as_str().filter(|s|s.len()<=4096)){
                                if !closing.load(Ordering::Acquire){let _=events.send(Event::MapStoreChanged{backend:backend.id.clone(),generation,thread:thread.into(),message:message.into()}).await;}
                            }
                        }continue;
                    }
                    if maps.incoming(&payload,tool){continue;}
                    for notice in notices{maps.notice(notice);}
                    let attention=prepared.approval.is_some()||prepared.interaction.is_some();
                    let owned=supervisor.observe(&payload,attention);
                    if owned {if let Some(id)=payload["id"].as_u64(){pending.remove(&id);}continue;}
                    if closing.load(Ordering::Acquire){continue;}
                    if let Some(id) = payload["id"].as_u64() {
                        if payload.get("method").is_none() {
                            if let Some((selection,task))=selections.get_mut(&id) {
                                // Late replies after cancellation must not start
                                // another read; only one reply belongs to a load.
                                if task.is_some() {continue;}
                                if let Some(error)=payload.get("error") {
                                    if selection.retry(error) {
                                        let response=json!({"id":id,"method":selection.method,"params":selection.original}).to_string();
                                        output.send(Message::text(response)).await.map_err(|e|e.to_string())?;
                                        continue;
                                    }
                                    selections.remove(&id);
                                } else {
                                    let api=api.clone();
                                    let context=crate::session_search::Context{backend:backend.id.clone(),thread:selection.thread.clone(),generation};
                                    *task=Some(histories.spawn(async move {
                                        (id,crate::session_hydration::load(&api,context,payload,prepared).await)
                                    }));
                                    continue;
                                }
                            }
                            pending.remove(&id);
                        }
                    }
                    if !closing.load(Ordering::Acquire)&&events.send(Event::Wire { backend: backend.id.clone(), generation, payload, prepared }).await.is_err() { return Ok(()); }
                }
                Some(Ok(Message::Ping(bytes))) => { output.send(Message::Pong(bytes)).await.map_err(|e| e.to_string())?; }
                Some(Ok(Message::Close(_))) | None => return Err(format!("{} disconnected", backend.name)),
                Some(Err(error)) => return Err(error.to_string()),
                _ => {}
            },
            loaded = histories.join_next(), if !histories.is_empty() => {
                if let Some(Ok((id,result)))=loaded {
                    if selections.remove(&id).is_none() || pending.remove(&id).is_none() {continue;}
                    let (payload,prepared)=match result {
                        Ok(result)=>result,
                        Err(error)=>(json!({"id":id,"error":{"message":error}}),Prepared::default()),
                    };
                    if !closing.load(Ordering::Acquire)&&events.send(Event::Wire{backend:backend.id.clone(),generation,payload,prepared}).await.is_err() {return Ok(());}
                }
            },
            effect = maps.next(), if maps.has_work() => {
                use crate::session_map_sync::Effect;
                match effect {
                    Effect::Send(payload)=>{if closing.load(Ordering::Acquire)&&payload.get("method").is_some(){continue;}let body=crate::worker::compute(move||serde_json::to_string(&payload).map_err(|e|e.to_string())).await?;output.send(Message::text(body)).await.map_err(|e|e.to_string())?;},
                    Effect::Start{id,params}=>{if closing.load(Ordering::Acquire){maps.cancel_start(id);continue;}pending.insert(id,Instant::now());let body=crate::worker::compute(move||serde_json::to_string(&json!({"id":id,"method":"turn/start","params":params})).map_err(|e|e.to_string())).await?;output.send(Message::text(body)).await.map_err(|e|e.to_string())?;},
                    Effect::GuardedStart{id,params}=>{if !supervisor.map_ready(id,params){maps.cancel_start(id);}},
                    Effect::Failed{id,message}=>{if !supervisor.failed(id,message.clone())&&!closing.load(Ordering::Acquire){let _=events.send(Event::Wire{backend:backend.id.clone(),generation,payload:json!({"id":id,"error":{"message":message}}),prepared:Prepared::default()}).await;}},
                    Effect::Changed{thread,result,message}=>{if !closing.load(Ordering::Acquire){let _=events.send(Event::MapSync{backend:backend.id.clone(),generation,thread,result,message}).await;}},
                }
            },
            action=supervisor.next(),if supervisor.has_work()=>{
                use crate::turn_supervision_host::Action;
                match action {
                    Action::Update(update)=>{if !closing.load(Ordering::Acquire)&&events.send(Event::Supervision{backend:backend.id.clone(),generation,update}).await.is_err(){return Ok(());}},
                    Action::Map{id,params}=>{if closing.load(Ordering::Acquire){maps.cancel_start(id);}else{maps.start_guarded(id,params);}},
                    Action::Abandon{id,reason:_}=>maps.cancel_start(id),
                    Action::Ready{id}=>{
                        if let Some(params)=supervisor.runtime.dispatch(id){
                            match maps.release_guarded(id){
                                Ok(prepared) if prepared==params=>{
                                    pending.insert(id,Instant::now());
                                    let body=crate::worker::compute(move||serde_json::to_string(&json!({"id":id,"method":"turn/start","params":params})).map_err(|e|e.to_string())).await?;
                                    if closing.load(Ordering::Acquire){let error="Studio closed before dispatch; the continuation was not sent";pending.remove(&id);maps.incoming(&json!({"id":id,"error":{"message":error}}),None);supervisor.runtime.acknowledge(id,Err(error.into()));continue;}
                                    output.send(Message::text(body)).await.map_err(|e|e.to_string())?;
                                },
                                Ok(_)=>{let error="Prepared source parameters changed; no continuation was sent";maps.incoming(&json!({"id":id,"error":{"message":error}}),None);supervisor.runtime.acknowledge(id,Err(error.into()));},
                                Err(error)=>{supervisor.runtime.acknowledge(id,Err(error));},
                            }
                        }else{maps.cancel_start(id);}
                    }
                }
            },
            _ = expiry.tick() => {
                maps.expire();
                supervisor.runtime.tick();
                let expired = pending.iter().filter(|(_, start)| start.elapsed() > Duration::from_secs(35)).map(|(id,_)| *id).collect::<Vec<_>>();
                for id in expired {
                    pending.remove(&id);
                    let timeout=json!({"id":id,"error":{"message":"Source acknowledgement timed out; inspect the conversation before continuing"}});
                    maps.incoming(&timeout,None);
                    if supervisor.observe(&timeout,false){continue;}
                    if let Some((_,Some(task)))=selections.remove(&id) {task.abort();}
                    let _ = events.send(Event::Wire { backend: backend.id.clone(), generation, prepared:Prepared::default(), payload: json!({"id":id,"error":{"message":"Request timed out. Refresh the session before retrying."}}) }).await;
                }
            }
        }
    }}.await;
    supervisor.shutdown().await;
    result
}

async fn backend_connection(
    api: Api,
    backend: Backend,
    generation: u64,
    commands: mpsc::Receiver<Command>,
    events: async_channel::Sender<Event>,
    closing: Arc<AtomicBool>,
) -> Result<(), String> {
    match backend.kind.as_str() {
        "codex" => codex_socket(api, backend, generation, commands, events, closing).await,
        "opencode" => crate::opencode::connect(api, backend, generation, commands, events).await,
        _ => Err("Unsupported backend protocol".into()),
    }
}

pub(crate) fn prepare_presentation(payload: &mut Value) -> Prepared {
    if let Some(turns) = payload.pointer("/result/thread/turns") {
        let validation = turns
            .as_array()
            .ok_or_else(|| "Backend returned invalid conversation turns".to_owned())
            .and_then(|turns| crate::history_state::lifecycle(turns).map(|_| ()));
        if let Err(error) = validation {
            // Legacy servers may ignore metadata-only loading. Their full
            // response needs the same lifecycle validation as paged reads.
            let id = payload["id"].take();
            *payload = json!({"id":id,"error":{"message":error}});
            return Prepared::default();
        }
    }
    // The dedicated history reader owns paging. Do not carry a redundant,
    // potentially large experimental initial page into GTK or a waiting job.
    if payload.get("method").is_none() {
        if let Some(result) = payload.get_mut("result").and_then(Value::as_object_mut) {
            result.remove("initialTurnsPage");
        }
    }
    fn prepare(item: &mut crate::model::Item) {
        if item.role == "Assistant" {
            item.rich = crate::chat_content::Content::parse(&item.body).ok();
            if item.rich.is_none() {
                item.markup = Some(crate::markdown::markup(&item.body));
            }
        }
    }
    let mut prepared = Prepared::default();
    prepared.command_data = crate::command_data::prepare(payload);
    prepared.mcp_page = crate::mcp::prepare(payload);
    prepared.catalog = crate::agents::prepare_page(payload);
    prepared.thread_summary = payload
        .pointer("/result/thread")
        .filter(|thread| thread.is_object())
        .map(crate::agents::Agent::parse);
    if let Some(turn) = payload
        .pointer_mut("/result/turn")
        .filter(|turn| turn["items"].is_array())
    {
        let source = json!({"turns":[{"id":turn["id"],"items":turn["items"].take()}]});
        let mut items = crate::model::history(&source)
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let mut bytes = 0;
        let mut count = 0;
        for item in &mut items {
            prepare(item);
            bytes += item.weight();
            if count > 0 && bytes > crate::model::MAX_VISIBLE_BYTES {
                break;
            }
            count += 1;
        }
        items.truncate(count);
        items.reverse();
        prepared.reply_items = items.into_iter().map(Arc::new).collect();
    }
    if payload.get("id").is_some() && payload.get("method").is_none() {
        if let Some(tools) = crate::composer_refs::catalog(&payload["result"]) {
            prepared.tools = Some(tools);
            payload["result"] = Value::Null;
        }
    }
    if payload.get("id").is_some() && payload.get("method").is_some() {
        if matches!(
            payload["method"].as_str(),
            Some(
                "item/tool/requestUserInput"
                    | "mcpServer/elicitation/request"
                    | "opencode/question"
            )
        ) {
            let project = payload["params"]["cwd"].as_str().map(str::to_owned);
            prepared.interaction = Some(
                crate::interaction::parse(payload["method"].as_str().unwrap(), &payload["params"])
                    .map(Arc::new),
            );
            // Schemas, defaults and question choices stay on the CPU worker;
            // GTK receives the bounded typed model and routing identity only.
            payload["params"] = json!({"threadId":payload["params"]["threadId"].as_str().or(payload["params"]["sessionID"].as_str()),"turnId":payload["params"]["turnId"]});
            if payload["method"] == "opencode/question" {
                if let Some(project) = project {
                    payload["params"]["cwd"] = project.into();
                }
            }
        } else {
            prepared.approval = Some(
                crate::approval::parse(
                    payload["method"].as_str().unwrap_or(""),
                    &payload["params"],
                )
                .map(Arc::new),
            );
            if prepared.approval.as_ref().is_some_and(Result::is_err) {
                let mut details =
                    serde_json::to_string_pretty(&payload["params"]).unwrap_or_default();
                if details.len() > 16384 {
                    let mut end = 16384;
                    while !details.is_char_boundary(end) {
                        end -= 1;
                    }
                    details.truncate(end);
                    details.push_str("\n… Details truncated; no approval actions are available.");
                }
                prepared.interaction_details = Some(details);
            }
            // Full policy/permission values stay in the bounded worker model.
            // GTK only needs the source identity for notices and routing.
            payload["params"] = json!({"threadId":payload["params"]["threadId"].as_str().or(payload["params"]["sessionID"].as_str()),"turnId":payload["params"]["turnId"],"cwd":payload["params"]["cwd"]});
        }
    }
    if let Some(thread) = payload.pointer_mut("/result/thread") {
        if thread.get("turns").is_some() {
            prepared.active_shell = thread["turns"]
                .as_array()
                .into_iter()
                .flatten()
                .rev()
                .find(|turn| turn["status"] == "inProgress")
                .is_some_and(|turn| {
                    turn["items"].as_array().into_iter().flatten().any(|item| {
                        item["type"] == "commandExecution" && item["source"] == "userShell"
                    })
                });
            let mut items = crate::model::history(thread);
            let mut bytes = 0usize;
            let mut retained = 0;
            for item in items.iter_mut().rev() {
                prepare(item);
                bytes = bytes.saturating_add(item.weight());
                if bytes > crate::model::MAX_VISIBLE_BYTES && retained > 0 {
                    break;
                }
                retained += 1;
            }
            if retained < items.len() {
                items.drain(..items.len() - retained);
            }
            prepared.active_turn = crate::model::active_turn(thread);
            prepared.history = Some(items.into_iter().map(Arc::new).collect());
            // Drop the unbounded JSON history on the worker. Only its bounded
            // presentation and small metadata cross into GTK's main loop.
            if let Some(thread) = thread.as_object_mut() {
                thread.remove("turns");
            }
        }
    }
    if crate::model::text(payload, "method") == "turn/plan/updated" {
        use std::hash::{Hash, Hasher};
        let thread = crate::model::text(&payload["params"], "threadId").to_owned();
        let turn = crate::model::text(&payload["params"], "turnId").to_owned();
        let valid =
            |id: &str| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control);
        if valid(&thread) && valid(&turn) {
            let mut value = payload["params"].take();
            let mut hash = std::hash::DefaultHasher::new();
            turn.hash(&mut hash);
            value["id"] = format!("studio-plan:{:016x}", hash.finish()).into();
            value["type"] = "plan".into();
            let mut item = crate::model::Item::from_value(&value);
            prepare(&mut item);
            prepared.item = Some(Arc::new(item));
        }
        payload["params"] = json!({"threadId":thread,"turnId":turn});
        return prepared;
    }
    if matches!(
        crate::model::text(payload, "method"),
        "item/started" | "item/completed"
    ) {
        if let Some(value) = payload.pointer_mut("/params/item") {
            prepared.shell_item =
                value["type"] == "commandExecution" && value["source"] == "userShell";
            let mut item = crate::model::Item::from_value(value);
            prepare(&mut item);
            *value = json!({"id":item.id});
            prepared.item = Some(Arc::new(item));
        }
        if let Some(item) = prepared
            .item
            .as_mut()
            .filter(|item| item.turn_id.is_empty())
        {
            Arc::make_mut(item).turn_id = payload
                .pointer("/params/turnId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
        }
    }
    prepared
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_notifications_are_worker_projected_and_replace_one_row_without_changing_activity() {
        let mut items = Vec::new();
        let mut active = Some("active-turn".into());
        let mut changes = crate::history_state::Changes::default();
        let mut identity = None;
        for status in ["inProgress", "completed"] {
            let mut payload = json!({"method":"turn/plan/updated","params":{"threadId":"thread","turnId":"active-turn","explanation":"Plan 中文😀","plan":[{"step":"Inspect <literal>","status":status}]}});
            let prepared = prepare_presentation(&mut payload);
            let item = prepared.item.as_ref().unwrap();
            assert_eq!(item.role, "Plan");
            assert!(item.body.contains(status));
            assert!(item.details.is_some());
            if let Some(previous) = &identity {
                assert_eq!(previous, &item.id);
            } else {
                identity = Some(item.id.clone());
            }
            assert!(payload.pointer("/params/plan").is_none());
            assert!(crate::history_state::observe(
                &mut items,
                &mut active,
                &mut changes,
                false,
                "turn/plan/updated",
                &payload["params"],
                prepared.item.as_ref()
            ));
        }
        assert_eq!(items.len(), 1);
        assert!(items[0].body.contains("completed"));
        assert_eq!(active.as_deref(), Some("active-turn"));
        let mut malformed = json!({"method":"turn/plan/updated","params":{"threadId":"thread","turnId":"","plan":[{"step":"must not cross into GTK"}]}});
        assert!(prepare_presentation(&mut malformed).item.is_none());
        assert!(malformed.pointer("/params/plan").is_none());
    }
    #[test]
    fn permission_choices_are_worker_prepared_without_raw_profile_in_gtk_payload() {
        let mut payload = json!({"id":"permission","method":"item/permissions/requestApproval","params":{"threadId":"t","turnId":"turn","cwd":"/project","permissions":{"network":{"enabled":true},"fileSystem":{"write":["/a","/b"],"entries":[{"access":"write","path":{"type":"path","path":"/a"}}]}}}});
        let prepared = prepare_presentation(&mut payload);
        let r = prepared.approval.unwrap().unwrap();
        assert_eq!(r.permissions.as_ref().unwrap().grants.len(), 3);
        assert!(r.actions.iter().any(|a| a.id == "review"));
        assert!(!r.details.contains("Permissions requested"));
        assert_eq!(
            payload["params"],
            json!({"threadId":"t","turnId":"turn","cwd":"/project"})
        );
        assert!(!payload.to_string().contains("fileSystem"));
    }
    #[test]
    fn worker_bounds_approval_decisions_and_strips_raw_permission_values() {
        let mut payload = json!({"id":"approval","method":"item/commandExecution/requestApproval","params":{"threadId":"t","turnId":"turn","cwd":"/private/project","command":"x".repeat(20000),"availableDecisions":["cancel",{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["git","status"]}}]}});
        let prepared = prepare_presentation(&mut payload);
        let model = prepared.approval.unwrap().unwrap();
        assert_eq!(model.actions.len(), 2);
        assert!(model.details.contains(&"x".repeat(20000)));
        assert!(!model.details.contains("truncated"));
        assert!(prepared.interaction_details.is_none());
        assert_eq!(
            payload["params"],
            json!({"threadId":"t","turnId":"turn","cwd":"/private/project"})
        );
        assert!(!payload.to_string().contains("execpolicy"));
        let mut malformed = json!({"id":1,"method":"item/commandExecution/requestApproval","params":{"command":"x".repeat(65536)}});
        let prepared = prepare_presentation(&mut malformed);
        assert!(prepared.approval.unwrap().is_err());
        assert!(prepared.interaction_details.unwrap().len() < 16500);
        assert!(malformed["params"].get("command").is_none());
    }
    #[test]
    fn worker_projects_input_forms_without_forwarding_raw_schemas_or_defaults_to_gtk() {
        let mut payload = json!({"id":"request","method":"mcpServer/elicitation/request","params":{"threadId":"thread","turnId":"turn","serverName":"test MCP","mode":"form","requestedSchema":{"type":"object","properties":{"value":{"type":"string","default":"private default"}}}}});
        let prepared = prepare_presentation(&mut payload);
        assert!(prepared.interaction.unwrap().is_ok());
        assert_eq!(
            payload["params"],
            json!({"threadId":"thread","turnId":"turn"})
        );
        assert!(!payload.to_string().contains("private default"));
        let mut malformed = json!({"id":17,"method":"item/tool/requestUserInput","params":{"threadId":"t","questions":[{"id":"q","question":"x".repeat(8193)}]}});
        assert!(prepare_presentation(&mut malformed)
            .interaction
            .unwrap()
            .is_err());
        assert!(malformed["params"].get("questions").is_none());
    }
    #[test]
    fn shell_identity_survives_worker_projection_and_only_tracks_the_active_turn() {
        let mut response = json!({"result":{"thread":{"id":"thread","turns":[{"id":"shell","status":"inProgress","items":[{"id":"command","type":"commandExecution","source":"userShell","command":"printf test","aggregatedOutput":"test"}]}]}}});
        let prepared = prepare_presentation(&mut response);
        assert!(prepared.active_shell);
        assert_eq!(prepared.active_turn.as_deref(), Some("shell"));
        assert!(response.pointer("/result/thread/turns").is_none());
        let mut response = json!({"result":{"thread":{"id":"thread","turns":[{"id":"old-shell","status":"completed","items":[{"type":"commandExecution","source":"userShell"}]},{"id":"model","status":"inProgress","items":[]}]}}});
        assert!(!prepare_presentation(&mut response).active_shell);
        let mut event = json!({"method":"item/started","params":{"threadId":"thread","turnId":"shell","item":{"id":"command","type":"commandExecution","source":"userShell","command":"printf test"}}});
        assert!(prepare_presentation(&mut event).shell_item);
    }
    #[test]
    fn worker_prepares_bounded_history_and_drops_raw_turns() {
        let items=(0..450).map(|id|json!({"id":id.to_string(),"type":"agentMessage","text":"**safe** <b>literal</b>"})).collect::<Vec<_>>();
        let mut payload = json!({"result":{"thread":{"id":"thread","turns":[{"id":"turn","status":"inProgress","items":items}]}}});
        let prepared = prepare_presentation(&mut payload);
        let history = prepared.history.unwrap();
        assert_eq!(history.len(), crate::model::MAX_VISIBLE_ITEMS);
        assert_eq!(history[0].id, "50");
        assert_eq!(prepared.active_turn.as_deref(), Some("turn"));
        assert!(history.iter().all(|item| item.turn_id == "turn"));
        let content = history[0].rich.as_ref().unwrap();
        let studio_markdown_native::BlockKind::Paragraph(text) = &content.rows[0].block.kind else {
            panic!("paragraph missing");
        };
        assert!(text.markup.contains("<b>safe</b>"));
        assert!(!text.markup.contains("<b>literal</b>"));
        assert!(payload.pointer("/result/thread/turns").is_none());
        assert_eq!(payload.pointer("/result/thread/id").unwrap(), "thread");
    }
    #[test]
    fn streaming_items_keep_their_backend_turn_anchor() {
        let mut payload = json!({"method":"item/completed","params":{"threadId":"thread","turnId":"turn-one","item":{"id":"message","type":"agentMessage","text":"done"}}});
        let prepared = prepare_presentation(&mut payload);
        assert_eq!(prepared.item.unwrap().turn_id, "turn-one");
    }
    #[test]
    fn malformed_legacy_lifecycle_fails_without_retaining_raw_history() {
        let mut payload = json!({"id":17,"result":{"thread":{"id":"thread","turns":[{"id":"t","status":"unknown-status","items":[{"id":"message","type":"agentMessage","text":"unverified"}]}]}}});
        let prepared = prepare_presentation(&mut payload);
        assert_eq!(payload["id"], 17);
        assert!(payload["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid turn state"));
        assert!(payload.get("result").is_none());
        assert!(prepared.history.is_none());
    }
}
