use super::*;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const BASE: &str = "# Private 😀\nkeep one\nkeep two\n";
type Audit = Arc<Mutex<Vec<(String, Value, String)>>>;
struct Server {
    api: Api,
    audit: Audit,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server(name: &'static str, revision: char, changed_page: bool, hold_save: bool) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = Api::from_access_url(&format!(
        "http://{}/#token=0123456789abcdef0123456789abcdef",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let audit: Audit = Arc::default();
    let log = audit.clone();
    let task = tokio::spawn(async move {
        let mut workers = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                connection = listener.accept() => {
                    let (mut socket, _) = connection.unwrap();
                    let log = log.clone();
                    workers.spawn(async move {
                        let mut bytes = Vec::new();
                        let mut chunk = [0; 4096];
                        let end = loop {
                            let count = socket.read(&mut chunk).await.unwrap();
                            if count == 0 { return; }
                            bytes.extend_from_slice(&chunk[..count]);
                            assert!(bytes.len() <= 1024 * 1024);
                            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") { break end + 4; }
                        };
                        let head = String::from_utf8_lossy(&bytes[..end]).to_string();
                        let route = head.lines().next().unwrap().split_whitespace().take(2).collect::<Vec<_>>().join(" ");
                        let header = |key: &str| head.lines().find_map(|line| line.split_once(':').filter(|(k,_)|k.eq_ignore_ascii_case(key)).map(|(_,v)|v.trim().to_string()));
                        let len = header("content-length").map_or(0, |s| s.parse::<usize>().unwrap());
                        assert!(len <= 1024 * 1024);
                        while bytes.len() < end + len {
                            let count = socket.read(&mut chunk).await.unwrap();
                            if count == 0 { return; }
                            bytes.extend_from_slice(&chunk[..count]);
                        }
                        let body: Value = if len == 0 { Value::Null } else { serde_json::from_slice(&bytes[end..end+len]).unwrap() };
                        log.lock().unwrap().push((route.clone(), body.clone(), header("x-reader-document").unwrap_or_default()));
                        let page = |text: &str, revision: char| json!({"documentId":format!("{name}-document"),"revision":revision.to_string().repeat(64),"sourceLines":3,"sourceBytes":BASE.len(),"window":{"start":0,"end":BASE.len(),"line":1,"endLine":3,"text":text}});
                        let result = if route.starts_with("POST /studio/native/markdown/open") {
                            json!({"readerId":format!("{name}-reader"),"readOnly":route.ends_with("open-readonly"),"root":body["root"],"path":if name=="retargeted" {json!("/private/different.md")} else {body["path"].clone()}})
                        } else if route.ends_with("/document") {
                            json!({"state":"ready","documentId":format!("{name}-document"),"index":{"name":"private.md","revision":revision.to_string().repeat(64),"sourceBytes":BASE.len(),"sourceLines":3,"pages":[{"start":0,"end":BASE.len(),"line":1,"rows":0,"rowStart":0,"table":null}],"tables":[],"toc":[]}})
                        } else if route.contains("/edit?") {
                            page(if changed_page { "# Inconsistent cached page\n" } else { BASE }, revision)
                        } else if route.starts_with("POST ") && route.ends_with("/edit") {
                            if hold_save { tokio::time::sleep(Duration::from_secs(30)).await; return; }
                            let mut units: Vec<u16> = BASE.encode_utf16().collect();
                            for change in body["changes"].as_array().unwrap().iter().rev() {
                                units.splice(change["from"].as_u64().unwrap() as usize..change["to"].as_u64().unwrap() as usize, change["insert"].as_str().unwrap().encode_utf16());
                            }
                            page(&String::from_utf16(&units).unwrap(), 'c')
                        } else if route == "POST /studio/markdown/close" { json!({"ok":true}) }
                        else { panic!("Unexpected private route {route}") };
                        let body = result.to_string();
                        let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                        let _ = socket.write_all(reply.as_bytes()).await;
                    });
                }
                _ = workers.join_next(), if !workers.is_empty() => {},
            }
        }
    });
    Server { api, audit, task }
}
fn message(generation: u64, request: Request) -> Message {
    Message {
        key: 1,
        generation,
        serial: 9,
        request,
    }
}
async fn response(output: &async_channel::Receiver<Event>, kind: &str) -> Result<Reply, String> {
    let Event::Document(result) = tokio::time::timeout(Duration::from_secs(3), output.recv())
        .await
        .unwrap()
        .unwrap()
    else {
        panic!("Unexpected event")
    };
    assert_eq!(result.kind, kind);
    result.result
}
async fn ask(
    input: &mpsc::Sender<Message>,
    output: &async_channel::Receiver<Event>,
    generation: u64,
    request: Request,
    kind: &str,
) -> Result<Reply, String> {
    input.send(message(generation, request)).await.unwrap();
    response(output, kind).await
}
async fn prepared(
    hold_save: bool,
) -> (
    Server,
    tokio::sync::watch::Sender<Option<Api>>,
    mpsc::Sender<Message>,
    async_channel::Receiver<Event>,
    tokio::task::JoinHandle<()>,
) {
    let original = server("original", 'a', false, hold_save).await;
    let (control, watch) = tokio::sync::watch::channel(Some(original.api.clone()));
    let (input, requests) = mpsc::channel(32);
    let (events, output) = async_channel::bounded(32);
    let task = tokio::spawn(run_recoverable(watch, requests, events));
    ask(
        &input,
        &output,
        1,
        Request::Open {
            root: "/private".into(),
            path: "/private/private.md".into(),
            read_only: false,
        },
        "open",
    )
    .await
    .unwrap();
    ask(&input, &output, 1, Request::Index, "index")
        .await
        .unwrap();
    ask(
        &input,
        &output,
        1,
        Request::Edit {
            line: 1,
            offset: None,
            previous: false,
        },
        "edit",
    )
    .await
    .unwrap();
    input
        .send(message(
            1,
            Request::Insert {
                offset: 0,
                text: "Before 中文\n".into(),
            },
        ))
        .await
        .unwrap();
    (original, control, input, output, task)
}
async fn offline(
    control: &tokio::sync::watch::Sender<Option<Api>>,
    input: &mpsc::Sender<Message>,
    output: &async_channel::Receiver<Event>,
) {
    control.send_replace(None);
    assert!(ask(
        input,
        output,
        1,
        Request::Source {
            line: 1,
            offset: None,
            previous: false
        },
        "source"
    )
    .await
    .unwrap_err()
    .contains("unavailable"));
    input
        .send(message(
            1,
            Request::Insert {
                offset: BASE.chars().count() + "Before 中文\n".chars().count(),
                text: "Offline 😀\n".into(),
            },
        ))
        .await
        .unwrap();
}
fn text() -> String {
    format!("Before 中文\n{BASE}Offline 😀\n")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_snapshot_keeps_offline_token_identities_and_only_explicit_save_writes() {
    let (old, control, input, output, task) = prepared(false).await;
    offline(&control, &input, &output).await;
    old.api.retire();
    let fresh = server("fresh", 'a', false, false).await;
    control.send_replace(Some(fresh.api.clone()));
    let resumed = ask(
        &input,
        &output,
        2,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some(text()),
        },
        "resume",
    )
    .await
    .unwrap();
    assert!(
        matches!(resumed,Reply::Resumed{reader_id,editor:Some(page)} if reader_id=="fresh-reader" && page.document_id=="fresh-document" && page.window.text==BASE)
    );
    assert!(!fresh
        .audit
        .lock()
        .unwrap()
        .iter()
        .any(|(route, _, _)| route.ends_with("/edit")));
    assert!(
        matches!(ask(&input,&output,2,Request::Save{text:text()},"save").await.unwrap(),Reply::Saved(page) if page.window.text==text())
    );
    let log = fresh.audit.lock().unwrap();
    let saves = log
        .iter()
        .filter(|(route, _, _)| route.ends_with("/edit"))
        .collect::<Vec<_>>();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].2, "fresh-document");
    let changes = saves[0].1["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(
        changes[0],
        json!({"from":0,"to":0,"insert":"Before 中文\n"})
    );
    assert_eq!(
        changes[1],
        json!({"from":BASE.encode_utf16().count(),"to":BASE.encode_utf16().count(),"insert":"Offline 😀\n"})
    );
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_file_conflict_keeps_typing_and_can_be_explicitly_revalidated_again() {
    let (_old, control, input, output, task) = prepared(false).await;
    offline(&control, &input, &output).await;
    let changed = server("changed", 'b', false, false).await;
    control.send_replace(Some(changed.api.clone()));
    assert!(ask(
        &input,
        &output,
        2,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some(text())
        },
        "resume"
    )
    .await
    .unwrap_err()
    .contains("file changed"));
    assert!(
        ask(&input, &output, 2, Request::Save { text: text() }, "save")
            .await
            .unwrap_err()
            .contains("Revalidate")
    );
    input
        .send(message(
            2,
            Request::Insert {
                offset: 0,
                text: "After conflict\n".into(),
            },
        ))
        .await
        .unwrap();
    let restored = server("restored", 'a', false, false).await;
    control.send_replace(Some(restored.api.clone()));
    ask(
        &input,
        &output,
        3,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some(format!("After conflict\n{}", text())),
        },
        "resume",
    )
    .await
    .unwrap();
    let log = changed.audit.lock().unwrap();
    assert_eq!(
        log.iter()
            .filter(|(r, _, _)| r == "POST /studio/markdown/close")
            .count(),
        1
    );
    assert!(!log.iter().any(|(r, _, _)| r.ends_with("/edit")));
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn matching_hash_with_inconsistent_page_and_mismatched_visible_text_fail_closed() {
    let (_old, control, input, output, task) = prepared(false).await;
    offline(&control, &input, &output).await;
    let fresh = server("inconsistent", 'a', true, false).await;
    control.send_replace(Some(fresh.api.clone()));
    assert!(ask(
        &input,
        &output,
        2,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some("wrong visible buffer".into())
        },
        "resume"
    )
    .await
    .unwrap_err()
    .contains("tracking differs"));
    assert!(fresh.audit.lock().unwrap().is_empty());
    assert!(ask(
        &input,
        &output,
        3,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some(text())
        },
        "resume"
    )
    .await
    .unwrap_err()
    .contains("page changed"));
    assert!(
        ask(&input, &output, 3, Request::Save { text: text() }, "save")
            .await
            .is_err()
    );
    assert!(!fresh
        .audit
        .lock()
        .unwrap()
        .iter()
        .any(|(r, _, _)| r.ends_with("/edit")));
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupted_save_is_not_replayed_and_changed_snapshot_requires_conflict_resolution() {
    let (old, control, input, output, task) = prepared(true).await;
    let edited = format!("Before 中文\n{BASE}");
    input
        .send(message(
            1,
            Request::Save {
                text: edited.clone(),
            },
        ))
        .await
        .unwrap();
    let start = std::time::Instant::now();
    while !old
        .audit
        .lock()
        .unwrap()
        .iter()
        .any(|(r, _, _)| r.ends_with("/edit"))
    {
        assert!(start.elapsed() < Duration::from_secs(3));
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    control.send_replace(None);
    assert!(response(&output, "save")
        .await
        .unwrap_err()
        .contains("not replayed"));
    old.api.retire();
    let fresh = server("after-uncertain-save", 'b', false, false).await;
    control.send_replace(Some(fresh.api.clone()));
    assert!(ask(
        &input,
        &output,
        2,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some(edited)
        },
        "resume"
    )
    .await
    .unwrap_err()
    .contains("file changed"));
    assert!(!fresh
        .audit
        .lock()
        .unwrap()
        .iter()
        .any(|(r, _, _)| r.ends_with("/edit")));
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_offline_close_cannot_remove_an_owner_and_readonly_rebind_keeps_source_identity() {
    let original = server("readonly-old", 'a', false, false).await;
    let (control, watch) = tokio::sync::watch::channel(Some(original.api.clone()));
    let (input, requests) = mpsc::channel(8);
    let (events, output) = async_channel::bounded(8);
    let task = tokio::spawn(run_recoverable(watch, requests, events));
    ask(
        &input,
        &output,
        4,
        Request::Open {
            root: "/private".into(),
            path: "/private/private.md".into(),
            read_only: true,
        },
        "open",
    )
    .await
    .unwrap();
    ask(&input, &output, 4, Request::Index, "index")
        .await
        .unwrap();
    control.send_replace(None);
    assert!(ask(
        &input,
        &output,
        4,
        Request::Source {
            line: 1,
            offset: None,
            previous: false
        },
        "source"
    )
    .await
    .is_err());
    ask(&input, &output, 3, Request::Close, "close")
        .await
        .unwrap();
    let fresh = server("readonly-new", 'a', false, false).await;
    control.send_replace(Some(fresh.api.clone()));
    assert!(
        matches!(ask(&input,&output,5,Request::Resume{revision:"a".repeat(64),text:None},"resume").await.unwrap(),Reply::Resumed{reader_id,editor:None} if reader_id=="readonly-new-reader")
    );
    assert!(fresh.audit.lock().unwrap().iter().any(|(r, body, _)| r
        == "POST /studio/native/markdown/open-readonly"
        && body["path"] == "/private/private.md"));
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retargeted_canonical_location_is_rejected_even_for_identical_source_bytes() {
    let (_old, control, input, output, task) = prepared(false).await;
    offline(&control, &input, &output).await;
    let retargeted = server("retargeted", 'a', false, false).await;
    control.send_replace(Some(retargeted.api.clone()));
    assert!(ask(
        &input,
        &output,
        2,
        Request::Resume {
            revision: "a".repeat(64),
            text: Some(text())
        },
        "resume"
    )
    .await
    .unwrap_err()
    .contains("canonical document location changed"));
    let log = retargeted.audit.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[1].0, "POST /studio/markdown/close");
    task.abort();
}
