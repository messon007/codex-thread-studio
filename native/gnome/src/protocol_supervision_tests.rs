//! Actual socket-loop/environment/Map/wire acceptance, with controlled evidence
//! and evaluator services. Real utility transport has its own WebSocket tests.
use super::*;
use crate::{
    session_search::Context,
    translation,
    turn_supervision::{Decision, Snapshot, Stage, TurnState},
    turn_supervision_host::{Request, Update},
    turn_supervision_runtime::{Service, Start},
};
use std::sync::Mutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[derive(Default)]
struct Data {
    completed: AtomicBool,
    sent: AtomicBool,
    evaluations: std::sync::atomic::AtomicUsize,
    ledger: Mutex<Vec<String>>,
    params: Mutex<Vec<Value>>,
    notifications: Mutex<Option<mpsc::UnboundedSender<Value>>>,
    resume_gate: Mutex<Option<Arc<tokio::sync::Semaphore>>>,
    resume_seen: tokio::sync::Notify,
    ack_gate: Mutex<Option<Arc<tokio::sync::Semaphore>>>,
    ack_seen: tokio::sync::Notify,
    reject_readiness: AtomicBool,
    reject_handoff: AtomicBool,
    routed_source: AtomicBool,
}
#[derive(Clone)]
struct Evidence(Arc<Data>);
impl Service for Evidence {
    async fn read(
        &self,
        source: crate::conversation_pages::Source,
        stop: Arc<AtomicBool>,
    ) -> Result<Snapshot, String> {
        if stop.load(Ordering::Acquire) {
            return Err("Cancelled source read".into());
        }
        assert_eq!(source.context, context());
        assert_eq!(source.directory, "/private/project");
        let sent = self.0.sent.load(Ordering::Acquire);
        let complete = self.0.completed.load(Ordering::Acquire) && !sent;
        Ok(Snapshot {
            context: context(),
            latest_turn: if sent { "accepted-next" } else { "root" }.into(),
            state: if complete {
                TurnState::Completed
            } else {
                TurnState::InProgress
            },
            active_turn: (!complete).then(|| if sent { "accepted-next" } else { "root" }.into()),
            blocked: false,
            error: false,
            original: if sent {
                "[Studio turn supervisor]\nComplete B"
            } else {
                "Finish A and B"
            }
            .into(),
            reply: if complete {
                "A is done; B remains"
            } else {
                "Working"
            }
            .into(),
        })
    }
    async fn evaluate(
        &self,
        _id: u64,
        context: translation::Context,
        effort: String,
        input: String,
        stop: Arc<AtomicBool>,
    ) -> Result<(Decision, Option<String>), String> {
        assert!(!stop.load(Ordering::Acquire));
        assert_eq!(context.thread, "source");
        assert_eq!(context.model, "source-model");
        assert_eq!(effort, "medium");
        let input: Value = serde_json::from_str(&input).unwrap();
        assert_eq!(input["originalUserMessage"], "Finish A and B");
        assert_eq!(input["latestAssistantMessage"], "A is done; B remains");
        self.0.evaluations.fetch_add(1, Ordering::AcqRel);
        Ok((
            Decision::parse(
                &json!({"decision":"continue","reason":"B remains","prompt":"Complete B"})
                    .to_string(),
            )?,
            None,
        ))
    }
}
fn context() -> Context {
    Context {
        backend: "codex".into(),
        thread: "source".into(),
        generation: 7,
    }
}
fn backend() -> Backend {
    Backend {
        id: "codex".into(),
        name: "Private fake".into(),
        kind: "codex".into(),
        socket_path: "/ws/codex".into(),
    }
}
fn start() -> Start {
    Start {
        request: 1,
        context: translation::Context {
            backend: backend(),
            socket: 7,
            thread: "source".into(),
            cwd: "/private/project".into(),
            model: "source-model".into(),
            ready: true,
        },
        turn: "root".into(),
        effort: "medium".into(),
    }
}
fn map() -> Value {
    json!({"id":"map-one","backend":"codex","threadId":"source","goal":"Finish A and B","definitionOfDone":"Human confirms","structure":"hierarchy","revision":1,"currentItemId":null,"lastSyncedTurnId":null,"items":[],"relations":[],"createdAt":1,"updatedAt":1})
}
async fn connection(mut stream: tokio::net::TcpStream, data: Arc<Data>, mapped: bool) {
    let mut prefix = [0; 8192];
    loop {
        let n = stream.peek(&mut prefix).await.unwrap();
        assert!(n > 0);
        if prefix[..n].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        assert!(n < prefix.len());
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    if prefix.starts_with(b"GET /ws/codex ") {
        let mut wire=tokio_tungstenite::accept_hdr_async(stream,|request:&tokio_tungstenite::tungstenite::handshake::server::Request,mut response:tokio_tungstenite::tungstenite::handshake::server::Response|{
            response.headers_mut().insert("Sec-WebSocket-Protocol",request.headers()["Sec-WebSocket-Protocol"].clone());Ok(response)
        }).await.unwrap();
        let (notify, mut notifications) = mpsc::unbounded_channel();
        *data.notifications.lock().unwrap() = Some(notify);
        wire.send(Message::text(
            json!({"method":"studio/appServer/status","params":{"state":"ready","generation":1}})
                .to_string(),
        ))
        .await
        .unwrap();
        loop {
            tokio::select! {
                Some(value)=notifications.recv()=>{wire.send(Message::text(value.to_string())).await.unwrap();},
                value=wire.next()=>match value {
                    Some(Ok(Message::Text(raw)))=>{
                        let value:Value=serde_json::from_str(&raw).unwrap();let method=value["method"].as_str().unwrap();data.ledger.lock().unwrap().push(format!("rpc:{method}"));
                        let result=match method {
                        "thread/resume"=>{assert!(mapped);assert_eq!(value["params"]["threadId"],"source");assert_eq!(value["params"]["excludeTurns"],true);assert!(value["params"]["dynamicTools"].as_array().is_some_and(|a|!a.is_empty()));let gate=data.resume_gate.lock().unwrap().clone();data.resume_seen.notify_one();if let Some(gate)=gate{loop{tokio::select!{permit=gate.acquire()=>{permit.unwrap().forget();break;},Some(notification)=notifications.recv()=>{wire.send(Message::text(notification.to_string())).await.unwrap();}}}}json!({"thread":{"id":"source"}})},
                        "turn/interrupt"=>json!({}),
                            "turn/start"=>{
                                assert_eq!(value["params"]["threadId"],"source");data.params.lock().unwrap().push(value["params"].clone());data.sent.store(true,Ordering::Release);
                                // A source notification can beat its RPC receipt.
                                wire.send(Message::text(json!({"method":"turn/started","params":{"threadId":"source","turn":{"id":"accepted-next","status":"inProgress","items":[]}}}).to_string())).await.unwrap();
                                let gate=data.ack_gate.lock().unwrap().clone();data.ack_seen.notify_one();
                                if let Some(gate)=gate {let permit=gate.acquire().await.unwrap();permit.forget();}
                                json!({"turn":{"id":"accepted-next","status":"inProgress","items":[]}})
                            },
                            other=>panic!("Unexpected mutation: {other}"),
                        };
                        wire.send(Message::text(json!({"id":value["id"],"result":result}).to_string())).await.unwrap();
                    if method=="thread/resume" && data.resume_gate.lock().unwrap().is_some(){
                        wire.send(Message::text(json!({"method":"studio/privateFixture/barrier","params":{}}).to_string())).await.unwrap();
                    }
                    },
                    Some(Ok(Message::Close(_)))|None=>break,
                    Some(Ok(Message::Ping(bytes)))=>{wire.send(Message::Pong(bytes)).await.unwrap();},
                    other=>panic!("Unexpected fixture frame: {other:?}"),
                }
            }
        }
        return;
    }
    let mut header = Vec::new();
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).await.unwrap();
        header.push(byte[0]);
        assert!(header.len() < 8192);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = String::from_utf8(header).unwrap();
    assert!(header
        .to_ascii_lowercase()
        .contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
    let request = header.lines().next().unwrap().to_owned();
    data.ledger.lock().unwrap().push(request.clone());
    let size = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|n| n.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    assert!(size <= 32 * 1024);
    let mut content = vec![0; size];
    stream.read_exact(&mut content).await.unwrap();
    let input: Value = if size == 0 {
        Value::Null
    } else {
        serde_json::from_slice(&content).unwrap()
    };
    let value = if request.starts_with("GET /studio/environment?") {
        json!({"root":"/private/project","configured":false,"revision":"none","variables":{},"secretNames":[],"networkPolicy":"inherit","allowedHosts":[],"cacheVariables":{}})
    } else if request == "GET /studio/session-map/codex/source HTTP/1.1" {
        if mapped {
            map()
        } else {
            Value::Null
        }
    } else if request == "POST /studio/native/router/prepare-supervised HTTP/1.1" {
        assert_eq!(
            input,
            json!({"backend":"codex","threadId":"source","cwd":"/private/project","turnId":"root"})
        );
        json!({"ready":!data.reject_readiness.load(Ordering::Acquire),"routed":data.routed_source.load(Ordering::Acquire)})
    } else if request == "POST /studio/native/router/follow HTTP/1.1" {
        assert_eq!(
            input,
            json!({"backend":"codex","threadId":"source","cwd":"/private/project","previousTurnId":"root","turnId":"accepted-next","prompt":"[Studio turn supervisor]\nComplete B"})
        );
        json!({"followed":if data.reject_handoff.load(Ordering::Acquire){Value::Null}else{Value::Bool(false)}})
    } else {
        panic!("Unexpected HTTP operation: {request}")
    };
    let body = value.to_string();
    let status = if request.starts_with("GET /studio/session-map/") && !mapped {
        404
    } else {
        200
    };
    let reply=format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
    stream.write_all(reply.as_bytes()).await.unwrap();
}
async fn fixture(
    mapped: bool,
    data: Arc<Data>,
) -> (
    Api,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = Api::from_access_url(&format!(
        "http://{}/#token=0123456789abcdef0123456789abcdef",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut jobs = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _=&mut stopped=>break,
                incoming=listener.accept()=>{let(stream,_)=incoming.unwrap();jobs.spawn(connection(stream,data.clone(),mapped));},
                Some(result)=jobs.join_next(),if !jobs.is_empty()=>result.unwrap(),
            }
        }
        while let Some(result) = jobs.join_next().await {
            result.unwrap();
        }
    });
    (api, stop, server)
}
async fn event(events: &async_channel::Receiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(8), events.recv())
        .await
        .expect("No socket event")
        .unwrap()
}
async fn request(commands: &mpsc::Sender<Command>, request: Request) {
    commands
        .send(Command::Supervision {
            backend: "codex".into(),
            generation: 7,
            request,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn supervision_uses_actual_socket_environment_map_and_guarded_wire_path() {
    for mapped in [false, true] {
        let data = Arc::new(Data::default());
        let (api, stop, server) = fixture(mapped, data.clone()).await;
        let (commands, input) = mpsc::channel(32);
        let (events, output) = async_channel::bounded(64);
        let socket = tokio::spawn(codex_socket_with_service(
            api,
            backend(),
            7,
            input,
            events,
            Evidence(data.clone()),
            Arc::new(AtomicBool::new(false)),
        ));
        loop {
            if let Event::Wire { payload, .. } = event(&output).await {
                if payload["method"] == "studio/appServer/status" {
                    break;
                }
            }
        }
        request(&commands, Request::Start(start())).await;
        let mut waiting = 0;
        let mut id = None;
        loop {
            match event(&output).await {
                Event::Supervision {
                    backend,
                    generation,
                    update,
                } => {
                    assert_eq!(backend, "codex");
                    assert_eq!(generation, 7);
                    match update {
                        Update::Changed(view) => {
                            assert_ne!(
                                view.stage,
                                Stage::Stopped,
                                "{}; {:?}",
                                view.reason,
                                data.ledger.lock().unwrap()
                            );
                            assert_eq!(view.context, context());
                            if view.stage == Stage::Waiting
                                && !data.completed.load(Ordering::Acquire)
                            {
                                waiting += 1;
                                if waiting == 2 {
                                    data.completed.store(true, Ordering::Release);
                                    data.notifications.lock().unwrap().as_ref().unwrap().send(json!({"method":"turn/completed","params":{"threadId":"source","turn":{"id":"root","status":"completed","items":[]}}})).unwrap();
                                }
                            }
                        }
                        Update::Prepare {
                            id: source_id,
                            request: token,
                            context: scope,
                            root,
                            text,
                        } => {
                            assert_eq!(scope, context());
                            assert_eq!(token, 1);
                            assert_eq!(root, "/private/project");
                            assert_eq!(text, "[Studio turn supervisor]\nComplete B");
                            id = Some(source_id);
                            request(&commands,Request::Prepared{id:source_id,params:json!({"threadId":"source","input":[{"type":"text","text":text}],"model":"source-model","effort":"medium","serviceTier":"fast","approvalPolicy":"on-request","sandboxPolicy":{"type":"readOnly"}})}).await;
                        }
                        Update::Follow {
                            id: source_id,
                            receipt,
                            prompt,
                            routed,
                        } => {
                            assert_eq!(Some(source_id), id);
                            assert_eq!(receipt.context, context());
                            assert!(receipt.continuing);
                            assert_eq!(receipt.previous_turn, "root");
                            assert_eq!(receipt.turn, "accepted-next");
                            assert_eq!(prompt, "[Studio turn supervisor]\nComplete B");
                            assert_eq!(routed, Ok(false));
                            request(
                                &commands,
                                Request::Stop {
                                    thread: "source".into(),
                                },
                            )
                            .await;
                            break;
                        }
                        Update::Rejected { message, .. } => panic!("{message}"),
                    }
                }
                Event::Wire { .. } | Event::MapSync { .. } => {}
                other => panic!("Unexpected event: {other:?}"),
            }
        }
        commands.send(Command::Shutdown).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), socket)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        stop.send(()).unwrap();
        server.await.unwrap();
        let params = data.params.lock().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0]["serviceTier"], "fast");
        assert_eq!(params[0]["model"], "source-model");
        assert_eq!(params[0]["sandboxPolicy"]["type"], "readOnly");
        assert_eq!(data.evaluations.load(Ordering::Acquire), 1);
        let ledger = data.ledger.lock().unwrap();
        let environment = ledger
            .iter()
            .position(|s| s.starts_with("GET /studio/environment?"))
            .unwrap();
        let map = ledger
            .iter()
            .position(|s| s.starts_with("GET /studio/session-map/"))
            .unwrap();
        let send = ledger.iter().position(|s| s == "rpc:turn/start").unwrap();
        let ready = ledger
            .iter()
            .position(|s| s.starts_with("POST /studio/native/router/prepare-supervised "))
            .unwrap();
        let follow = ledger
            .iter()
            .position(|s| s.starts_with("POST /studio/native/router/follow "))
            .unwrap();
        assert!(ready < environment && environment < map && map < send && send < follow);
        assert_eq!(
            ledger.iter().filter(|s| *s == "rpc:thread/resume").count(),
            usize::from(mapped)
        );
    }
}

#[tokio::test]
async fn closing_the_window_drains_a_late_accepted_receipt_without_gui_or_source_replay() {
    for mapped in [false, true] {
        let data = Arc::new(Data::default());
        data.completed.store(true, Ordering::Release);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *data.ack_gate.lock().unwrap() = Some(gate.clone());
        let (api, stop, server) = fixture(mapped, data.clone()).await;
        let (commands, input) = mpsc::channel(32);
        let (events, output) = async_channel::bounded(64);
        let closing = Arc::new(AtomicBool::new(false));
        let socket = tokio::spawn(codex_socket_with_service(
            api,
            backend(),
            7,
            input,
            events,
            Evidence(data.clone()),
            closing.clone(),
        ));
        loop {
            if let Event::Wire { payload, .. } = event(&output).await {
                if payload["method"] == "studio/appServer/status" {
                    break;
                }
            }
        }
        request(&commands, Request::Start(start())).await;
        loop {
            match event(&output).await {
                Event::Supervision {
                    update: Update::Prepare { id, text, .. },
                    ..
                } => {
                    request(&commands,Request::Prepared{id,params:json!({"threadId":"source","input":[{"type":"text","text":text}]})}).await;
                    break;
                }
                Event::Supervision {
                    update: Update::Rejected { message, .. },
                    ..
                } => panic!("{message}"),
                _ => {}
            }
        }
        tokio::time::timeout(Duration::from_secs(3), data.ack_seen.notified())
            .await
            .unwrap();
        closing.store(true, Ordering::Release);
        // No GTK recipient remains. The socket must own durable Router handoff.
        drop(output);
        commands.send(Command::Shutdown).await.unwrap();
        gate.add_permits(1);
        tokio::time::timeout(Duration::from_secs(6), socket)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        stop.send(()).unwrap();
        server.await.unwrap();
        assert_eq!(data.params.lock().unwrap().len(), 1);
        assert_eq!(data.evaluations.load(Ordering::Acquire), 1);
        assert_eq!(
            data.ledger
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.starts_with("POST /studio/native/router/follow "))
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn unconfirmed_router_preparation_or_handoff_stops_without_a_source_retry() {
    for case in ["preparation", "handoff", "missing-route"] {
        let preparation = case == "preparation";
        let data = Arc::new(Data::default());
        data.completed.store(true, Ordering::Release);
        data.reject_readiness.store(preparation, Ordering::Release);
        data.reject_handoff
            .store(case == "handoff", Ordering::Release);
        data.routed_source
            .store(case == "missing-route", Ordering::Release);
        let (api, stop, server) = fixture(false, data.clone()).await;
        let (commands, input) = mpsc::channel(32);
        let (events, output) = async_channel::bounded(64);
        let socket = tokio::spawn(codex_socket_with_service(
            api,
            backend(),
            7,
            input,
            events,
            Evidence(data.clone()),
            Arc::new(AtomicBool::new(false)),
        ));
        loop {
            if let Event::Wire { payload, .. } = event(&output).await {
                if payload["method"] == "studio/appServer/status" {
                    break;
                }
            }
        }
        request(&commands, Request::Start(start())).await;
        let mut failed_receipt = false;
        loop {
            match event(&output).await {
                Event::Supervision {
                    update: Update::Prepare { id, text, .. },
                    ..
                } => {
                    request(&commands,Request::Prepared{id,params:json!({"threadId":"source","input":[{"type":"text","text":text}]})}).await;
                }
                Event::Supervision {
                    update:
                        Update::Follow {
                            receipt, routed, ..
                        },
                    ..
                } => {
                    assert!(!preparation);
                    assert!(routed.is_err());
                    assert_eq!(receipt.turn, "accepted-next");
                    failed_receipt = true;
                }
                Event::Supervision {
                    update: Update::Changed(view),
                    ..
                } if view.stage == Stage::Stopped => {
                    assert!(!view.holding);
                    break;
                }
                Event::Supervision {
                    update: Update::Rejected { message, .. },
                    ..
                } => panic!("{message}"),
                _ => {}
            }
        }
        commands.send(Command::Shutdown).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), socket)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        stop.send(()).unwrap();
        server.await.unwrap();
        assert_eq!(failed_receipt, !preparation);
        assert_eq!(data.params.lock().unwrap().len(), usize::from(!preparation));
        let ledger = data.ledger.lock().unwrap();
        assert_eq!(
            ledger
                .iter()
                .filter(|s| s.starts_with("GET /studio/environment?"))
                .count(),
            usize::from(!preparation)
        );
        assert_eq!(data.evaluations.load(Ordering::Acquire), 1);
    }
}

#[tokio::test]
async fn human_interrupt_or_approval_during_map_preparation_prevents_a_source_send() {
    for approval in [false, true] {
        let data = Arc::new(Data::default());
        data.completed.store(true, Ordering::Release);
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *data.resume_gate.lock().unwrap() = Some(gate.clone());
        let (api, stop, server) = fixture(true, data.clone()).await;
        let (commands, input) = mpsc::channel(32);
        let (events, output) = async_channel::bounded(64);
        let socket = tokio::spawn(codex_socket_with_service(
            api,
            backend(),
            7,
            input,
            events,
            Evidence(data.clone()),
            Arc::new(AtomicBool::new(false)),
        ));
        loop {
            if let Event::Wire { payload, .. } = event(&output).await {
                if payload["method"] == "studio/appServer/status" {
                    break;
                }
            }
        }
        request(&commands, Request::Start(start())).await;
        loop {
            match event(&output).await {
                Event::Supervision {
                    update: Update::Prepare { id, text, .. },
                    ..
                } => {
                    request(&commands,Request::Prepared{id,params:json!({"threadId":"source","input":[{"type":"text","text":text}]})}).await;
                    break;
                }
                Event::Supervision {
                    update: Update::Rejected { message, .. },
                    ..
                } => panic!("{message}"),
                Event::Supervision {
                    update: Update::Changed(view),
                    ..
                } => assert_ne!(view.stage, Stage::Stopped, "{}", view.reason),
                _ => {}
            }
        }
        tokio::time::timeout(Duration::from_secs(3), data.resume_seen.notified())
            .await
            .unwrap();
        if approval {
            data.notifications.lock().unwrap().as_ref().unwrap().send(json!({"id":"approval-owned","method":"item/commandExecution/requestApproval","params":{"threadId":"source","turnId":"root","itemId":"command","command":"read local input","cwd":"/private/project"}})).unwrap();
        } else {
            commands
                .send(Command::Rpc {
                    backend: "codex".into(),
                    generation: 7,
                    id: 91,
                    method: "turn/interrupt".into(),
                    params: json!({"threadId":"source","turnId":"root"}),
                    root: None,
                    images: Vec::new(),
                })
                .await
                .unwrap();
        }
        loop {
            match event(&output).await {
                Event::Supervision {
                    update: Update::Changed(view),
                    ..
                } if view.stage == Stage::Stopped => {
                    assert!(!view.holding);
                    break;
                }
                Event::Supervision {
                    update: Update::Follow { .. },
                    ..
                } => panic!("Cancelled preparation produced an accepted turn"),
                _ => {}
            }
        }
        gate.add_permits(1);
        loop {
            if let Event::Wire { payload, .. } = event(&output).await {
                if payload["method"] == "studio/privateFixture/barrier" {
                    break;
                }
            }
        }
        if !approval {
            loop {
                if let Event::Wire { payload, .. } = event(&output).await {
                    if payload["id"] == 91 {
                        break;
                    }
                }
            }
        }
        commands.send(Command::Shutdown).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), socket)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        stop.send(()).unwrap();
        server.await.unwrap();
        assert!(data.params.lock().unwrap().is_empty());
        assert!(!data.sent.load(Ordering::Acquire));
        assert_eq!(data.evaluations.load(Ordering::Acquire), 1);
    }
}

#[tokio::test]
async fn socket_rejects_foreign_generations_backends_and_evaluator_socket_paths_before_any_work() {
    let data = Arc::new(Data::default());
    let (api, stop, server) = fixture(false, data.clone()).await;
    let (commands, input) = mpsc::channel(32);
    let (events, output) = async_channel::bounded(64);
    let socket = tokio::spawn(codex_socket_with_service(
        api,
        backend(),
        7,
        input,
        events,
        Evidence(data.clone()),
        Arc::new(AtomicBool::new(false)),
    ));
    loop {
        if let Event::Wire { payload, .. } = event(&output).await {
            if payload["method"] == "studio/appServer/status" {
                break;
            }
        }
    }
    for kind in ["generation", "backend", "socket"] {
        let mut source = start();
        if kind == "socket" {
            source.context.backend.socket_path = "/ws/other-backend".into();
        }
        commands
            .send(Command::Supervision {
                backend: if kind == "backend" { "other" } else { "codex" }.into(),
                generation: if kind == "generation" { 8 } else { 7 },
                request: Request::Start(source),
            })
            .await
            .unwrap();
        match event(&output).await {
            Event::Supervision {
                update: Update::Rejected {
                    thread, message, ..
                },
                ..
            } => {
                assert_eq!(thread.as_deref(), Some("source"));
                assert!(message.contains("does not match"));
            }
            other => panic!("{other:?}"),
        }
    }
    commands.send(Command::Shutdown).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), socket)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    stop.send(()).unwrap();
    server.await.unwrap();
    assert!(data.ledger.lock().unwrap().is_empty());
    assert_eq!(data.evaluations.load(Ordering::Acquire), 0);
    assert!(data.params.lock().unwrap().is_empty());
}
