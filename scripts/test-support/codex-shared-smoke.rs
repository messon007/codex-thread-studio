//! Opt-in real CLI integration, compiled inside the actual gateway test crate.
use crate::backend_runtime::{BackendRuntime, WslSettings};
use crate::codex_app_server::CodexAppServer;
use crate::codex_connection::ConnectionMode;
use serde_json::{json, Value};
use tokio::sync::broadcast;

async fn ready(server: &CodexAppServer) {
    for _ in 0..100 {
        match server.request("thread/list", json!({"limit":1})).await {
            Ok(_) => return,
            Err(error) if error.contains("still initializing") => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(error) => panic!("{error}"),
        }
    }
    panic!("initialization timeout");
}

async fn completion(
    events: &mut broadcast::Receiver<String>,
    thread: &str,
    previous: &str,
) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let event: Value = serde_json::from_str(&events.recv().await.unwrap()).unwrap();
            if event["method"] == "turn/completed" && event["params"]["threadId"] == thread {
                let turn = &event["params"]["turn"];
                if turn["id"].as_str() == Some(previous) {
                    continue;
                }
                assert_eq!(turn["status"], "completed", "{event}");
                return turn["id"].as_str().unwrap().to_owned();
            }
        }
    })
    .await
    .expect("turn completion timeout")
}

pub async fn run() {
    let home = std::env::var("STUDIO_CODEX_SMOKE_HOME").expect("run through smoke-codex-shared.py");
    assert_eq!(std::env::var("CODEX_HOME").unwrap(), home);
    assert!(std::path::Path::new(&home)
        .join("studio-smoke-fixture")
        .is_file());
    let id = std::env::var("STUDIO_TEST_THREAD_ID").unwrap();
    let runtime = BackendRuntime::new(std::env::var_os("PATH").unwrap(), WslSettings::default());
    let a = CodexAppServer::new("codex".into(), runtime.clone());
    let b =
        CodexAppServer::new("codex".into(), runtime).with_connection_mode(ConnectionMode::Shared);
    ready(&a).await;
    ready(&b).await;
    a.request("thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    b.request("thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    a.request(
        "thread/name/set",
        json!({"threadId":id,"name":"from-studio"}),
    )
    .await
    .unwrap();
    b.request("thread/name/set", json!({"threadId":id,"name":"from-peer"}))
        .await
        .unwrap();
    let read = a
        .request("thread/read", json!({"threadId":id}))
        .await
        .unwrap();
    assert_eq!(read["thread"]["name"], "from-peer");
    let mut stream = a.map_events().subscribe();
    let private = a
        .request_private_read("thread/read", json!({"threadId":id,"includeTurns":true}))
        .await
        .unwrap();
    assert_eq!(private["thread"]["id"], id);
    while let Ok(message) = stream.try_recv() {
        let value: Value = serde_json::from_str(&message).unwrap();
        assert!(
            !value["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("studio-native-read-")),
            "Private history leaked into the shared client stream"
        );
    }
    println!("Native private history reads preserve the source without broadcast");
    println!("Two Studio connections resumed and wrote the same live thread");

    let mut first = a.subscribe_lifecycle();
    let mut peer = b.subscribe_lifecycle();
    let turn = a
        .request(
            "turn/start",
            json!({"threadId":id,"input":[{"type":"text","text":"from-studio-prompt"}]}),
        )
        .await
        .unwrap();
    let first_id = completion(&mut first, &id, "").await;
    assert_eq!(turn["turn"]["id"], first_id);
    assert_eq!(completion(&mut peer, &id, "").await, first_id);
    let command = a
        .run_workspace_command(
            "codex",
            &[
                "queue",
                "--thread",
                &id,
                "--message",
                "from-cli-prompt",
                "--resume",
            ],
            &[],
        )
        .await
        .unwrap();
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    let second_id = completion(&mut first, &id, &first_id).await;
    assert_eq!(completion(&mut peer, &id, &first_id).await, second_id);
    let read = a
        .request("thread/read", json!({"threadId":id,"includeTurns":true}))
        .await
        .unwrap();
    let history = serde_json::to_string(&read).unwrap();
    assert!(history.contains("from-studio-prompt"));
    assert!(history.contains("from-cli-prompt"));
    println!(
        "Studio and the actual Codex CLI both submitted turns; both clients received completions"
    );

    let mut lifecycle = a.subscribe_lifecycle();
    // This harness only runs in the private fixture CODEX_HOME checked above.
    let stopped = a
        .run_workspace_command("codex", &["app-server", "daemon", "stop"], &[])
        .await
        .unwrap();
    assert!(stopped.status.success());
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let raw = lifecycle.recv().await.unwrap();
            let event: Value = serde_json::from_str(&raw).unwrap();
            if event["method"] == "studio/appServer/status" && event["params"]["state"] == "stopped"
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    ready(&a).await;
    a.request("thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    println!("Studio reconnected after the private test daemon restarted");
}
