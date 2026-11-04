use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn map(revision: u64, id: &str, current: Option<&str>) -> Value {
    json!({"id":id,"backend":"codex","threadId":"source","goal":"User goal 中文😀","definitionOfDone":"Human confirms","structure":"hierarchy","revision":revision,"currentItemId":current,"lastSyncedTurnId":null,"items":[{"id":"root","parentId":null,"kind":"topic","title":"Root","summary":"","state":"notStarted","position":0,"archived":false,"createdAt":1,"updatedAt":1},{"id":"child","parentId":"root","kind":"topic","title":"Child","summary":"","state":"notStarted","position":0,"archived":false,"createdAt":1,"updatedAt":1}],"relations":[],"createdAt":1,"updatedAt":1})
}
async fn server(
    replies: Vec<(u16, Value)>,
) -> (Api, tokio::task::JoinHandle<Vec<(String, Value)>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = Api::from_access_url(&format!(
        "http://{}/#token=0123456789abcdef0123456789abcdef",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (status, body) in replies {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut request = Vec::new();
            let split = loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
                if request.ends_with(b"\r\n\r\n") {
                    break request.len();
                }
            };
            let headers = String::from_utf8(request[..split].to_vec()).unwrap();
            assert!(headers
                .to_ascii_lowercase()
                .contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
            let len = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(len <= 512 * 1024);
            let mut bytes = vec![0; len];
            socket.read_exact(&mut bytes).await.unwrap();
            requests.push((
                headers.lines().next().unwrap().into(),
                if len == 0 {
                    Value::Null
                } else {
                    serde_json::from_slice(&bytes).unwrap()
                },
            ));
            if status == 0 {
                continue;
            } // accepted request, connection closed without a reply
            let body = body.to_string();
            let reply=format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
        requests
    });
    (api, task)
}
async fn next(r: &mut Runtime) -> Effect {
    tokio::time::timeout(Duration::from_secs(5), r.next())
        .await
        .unwrap()
}

#[tokio::test]
async fn guarded_map_configuration_does_not_register_a_turn_until_single_use_release() {
    let (api, server) = server(vec![(200, map(1, "map-one", None))]).await;
    let mut r = Runtime::new(api, "codex".into());
    let params = json!({"threadId":"source","input":[{"type":"text","text":"Continuation"}],"serviceTier":"fast"});
    r.start_guarded(51, params.clone());
    let Effect::Send(resume) = next(&mut r).await else {
        panic!("Missing Map resume")
    };
    assert!(incoming(
        &mut r,
        json!({"id":resume["id"],"result":{"thread":{"id":"source"}}})
    ));
    let Effect::GuardedStart {
        id,
        params: prepared,
    } = next(&mut r).await
    else {
        panic!("Guarded preparation sent an ordinary turn")
    };
    assert_eq!(id, 51);
    assert_eq!(prepared, params);
    assert!(!r.turns.contains_key("source"));
    assert!(!r.sent.contains_key(&51));
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "old".into(),
        completed: true,
    });
    assert!(!r.turns.contains_key("source"));
    assert_eq!(r.release_guarded(51).unwrap(), params);
    assert!(r.release_guarded(51).is_err());
    assert_eq!(r.sent[&51], "source");
    assert!(!r.turns["source"].completed);
    assert!(!r
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::Start { .. })));
    server.await.unwrap();
}

#[tokio::test]
async fn cancelled_guarded_configuration_is_cleared_before_a_later_no_map_turn() {
    let (api, server) = server(vec![
        (200, map(1, "map-one", None)),
        (404, json!({"error":"Map not found"})),
    ])
    .await;
    let mut r = Runtime::new(api, "codex".into());
    r.start_guarded(52, json!({"threadId":"source","input":[]}));
    let Effect::Send(resume) = next(&mut r).await else {
        panic!()
    };
    incoming(
        &mut r,
        json!({"id":resume["id"],"result":{"thread":{"id":"source"}}}),
    );
    assert!(matches!(next(&mut r).await, Effect::GuardedStart { .. }));
    r.cancel_guarded("source");
    assert!(r.release_guarded(52).is_err());
    assert!(!r.turns.contains_key("source"));
    r.start(53, json!({"threadId":"source","input":[]}));
    let Effect::Send(clear) = next(&mut r).await else {
        panic!("Cancelled configuration was forgotten")
    };
    assert_eq!(clear["params"]["dynamicTools"], json!([]));
    assert_eq!(clear["params"]["developerInstructions"], Value::Null);
    incoming(
        &mut r,
        json!({"id":clear["id"],"result":{"thread":{"id":"source"}}}),
    );
    assert!(matches!(next(&mut r).await, Effect::Start { id: 53, .. }));
    server.await.unwrap();
}

#[tokio::test]
async fn cancellation_before_map_resume_reply_consumes_the_late_reply_without_sending() {
    let (api, server) = server(vec![
        (200, map(1, "map-one", None)),
        (404, json!({"error":"Map not found"})),
    ])
    .await;
    let mut r = Runtime::new(api, "codex".into());
    r.start_guarded(54, json!({"threadId":"source","input":[]}));
    let Effect::Send(resume) = next(&mut r).await else {
        panic!()
    };
    r.cancel_guarded("source");
    assert!(incoming(
        &mut r,
        json!({"id":resume["id"],"result":{"thread":{"id":"source"}}})
    ));
    assert!(r.release_guarded(54).is_err());
    assert!(r.effects.is_empty());
    assert!(r.turns.is_empty());
    r.start(55, json!({"threadId":"source","input":[]}));
    let Effect::Send(clear) = next(&mut r).await else {
        panic!("Uncertain resume was forgotten")
    };
    assert_eq!(clear["params"]["dynamicTools"], json!([]));
    incoming(
        &mut r,
        json!({"id":clear["id"],"result":{"thread":{"id":"source"}}}),
    );
    assert!(matches!(next(&mut r).await, Effect::Start { id: 55, .. }));
    server.await.unwrap();
}
fn incoming(r: &mut Runtime, p: Value) -> bool {
    let tool = prepare_tool(&p);
    r.incoming(&p, tool)
}
async fn configured(r: &mut Runtime, id: u64, turn: &str) {
    let Effect::Send(resume) = next(r).await else {
        panic!("Expected context resume")
    };
    assert_eq!(resume["method"], "thread/resume");
    assert_eq!(resume["params"]["excludeTurns"], true);
    assert!(incoming(
        r,
        json!({"id":resume["id"],"result":{"thread":{"id":"source"}}})
    ));
    let Effect::Start { id: sent, .. } = next(r).await else {
        panic!("Turn was not released")
    };
    assert_eq!(sent, id);
    incoming(r, json!({"id":id,"result":{"turn":{"id":turn}}}));
}
fn tool(id: &str, call: &str, turn: &str, operations: Value) -> Value {
    json!({"id":id,"method":"item/tool/call","params":{"threadId":"source","turnId":turn,"callId":call,"tool":"update_session_map","arguments":{"operations":operations}}})
}
#[tokio::test]
async fn context_falls_back_without_resending_the_turn_and_duplicate_tools_apply_once() {
    let (api, server) = server(vec![
        (200, map(3, "map-one", None)),
        (200, map(4, "map-one", Some("root"))),
    ])
    .await;
    let mut r = Runtime::new(api, "codex".into());
    let original =
        json!({"threadId":"source","input":[{"type":"text","text":"User text unchanged 中文😀"}]});
    r.start(7, original.clone());
    let Effect::Send(resume) = next(&mut r).await else {
        panic!()
    };
    assert_eq!(
        resume["params"]["dynamicTools"][0]["name"],
        "update_session_map"
    );
    assert!(resume["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains("User goal 中文😀"));
    incoming(
        &mut r,
        json!({"id":resume["id"],"error":{"code":-32602,"message":"unknown dynamicTools field"}}),
    );
    let Effect::Send(resume) = next(&mut r).await else {
        panic!()
    };
    assert!(resume["params"].get("dynamicTools").is_none());
    incoming(
        &mut r,
        json!({"id":resume["id"],"result":{"thread":{"id":"source"}}}),
    );
    let Effect::Start { id, params } = next(&mut r).await else {
        panic!()
    };
    assert_eq!(id, 7);
    assert_eq!(params, original);
    incoming(&mut r, json!({"id":7,"result":{"turn":{"id":"turn-one"}}}));
    let ops = json!([{"op":"setCurrent","itemId":"root"}]);
    assert!(incoming(
        &mut r,
        tool("rpc-one", "call-one", "turn-one", ops.clone())
    ));
    assert!(incoming(
        &mut r,
        tool("rpc-two", "call-one", "turn-one", ops.clone())
    ));
    for expected in ["rpc-one", "rpc-two"] {
        let Effect::Send(response) = next(&mut r).await else {
            panic!()
        };
        assert_eq!(response["id"], expected);
        assert_eq!(response["result"]["success"], true);
    }
    let Effect::Changed { result, .. } = next(&mut r).await else {
        panic!()
    };
    assert_eq!(result.unwrap().unwrap().map.revision, 4);
    incoming(&mut r, tool("rpc-three", "call-one", "turn-one", ops));
    let Effect::Send(response) = next(&mut r).await else {
        panic!()
    };
    assert_eq!(response["result"]["success"], true);
    incoming(
        &mut r,
        tool(
            "rpc-four",
            "call-one",
            "turn-one",
            json!([{"op":"setCurrent","itemId":"child"}]),
        ),
    );
    let Effect::Send(response) = next(&mut r).await else {
        panic!()
    };
    assert_eq!(response["result"]["success"], false);
    r.notice(Notice::Message {
        thread: "source".into(),
        turn: "turn-one".into(),
        update: Some(Ok(Update {
            revision: 3,
            operations: vec![crate::session_map::types::MapOperation::SetCurrent {
                item_id: Some("child".into()),
            }],
        })),
    });
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    assert!(!r.has_work());
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].1["mapId"], "map-one");
    assert_eq!(requests[1].1["baseRevision"], 3);
    assert_eq!(requests[1].1["actor"], "assistant");
    assert_eq!(requests[1].1["sourceTurnId"], "turn-one");
}
#[tokio::test]
async fn fallback_finishes_before_the_next_queued_turn_captures_context() {
    let (api, server) = server(vec![
        (200, map(3, "map-one", None)),
        (200, map(4, "map-one", Some("child"))),
        (200, map(4, "map-one", Some("child"))),
    ])
    .await;
    let mut r = Runtime::new(api, "codex".into());
    r.start(1, json!({"threadId":"source","input":[]}));
    configured(&mut r, 1, "turn-one").await;
    r.notice(Notice::Message {
        thread: "source".into(),
        turn: "turn-one".into(),
        update: Some(Ok(Update {
            revision: 3,
            operations: vec![crate::session_map::types::MapOperation::SetCurrent {
                item_id: Some("child".into()),
            }],
        })),
    });
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    r.start(2, json!({"threadId":"source","input":[]}));
    assert_eq!(
        r.jobs.len(),
        1,
        "The next context must wait for the prior write"
    );
    let Effect::Changed { result, .. } = next(&mut r).await else {
        panic!()
    };
    assert_eq!(result.unwrap().unwrap().map.revision, 4);
    let Effect::Send(resume) = next(&mut r).await else {
        panic!()
    };
    assert!(resume["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains("\"revision\":4"));
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    assert_eq!(server.await.unwrap().len(), 3);
}
#[tokio::test]
async fn wrong_turn_unsafe_tool_and_stale_fallback_never_write() {
    let (api, server) = server(vec![(200, map(3, "map-one", None))]).await;
    let mut r = Runtime::new(api, "codex".into());
    r.start(1, json!({"threadId":"source"}));
    configured(&mut r, 1, "turn-one").await;
    for p in [
        tool(
            "foreign",
            "one",
            "other-turn",
            json!([{"op":"setCurrent","itemId":"root"}]),
        ),
        tool(
            "unsafe",
            "two",
            "turn-one",
            json!([{"op":"setState","itemId":"root","state":"done"}]),
        ),
    ] {
        incoming(&mut r, p);
        let Effect::Send(response) = next(&mut r).await else {
            panic!()
        };
        assert_eq!(response["result"]["success"], false);
    }
    r.notice(Notice::Message {
        thread: "source".into(),
        turn: "turn-one".into(),
        update: Some(Ok(Update {
            revision: 2,
            operations: vec![],
        })),
    });
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    let Effect::Changed { result, .. } = next(&mut r).await else {
        panic!()
    };
    assert!(result.unwrap_err().message.contains("stale"));
    assert!(!r.has_work());
    assert_eq!(server.await.unwrap().len(), 1);
}
#[tokio::test]
async fn conflicts_never_rebase_or_replay_a_frozen_update() {
    let (api, server) = server(vec![
        (200, map(3, "map-one", None)),
        (409, json!({"error":"map identity conflict: replaced"})),
    ])
    .await;
    let mut r = Runtime::new(api, "codex".into());
    r.start(1, json!({"threadId":"source"}));
    configured(&mut r, 1, "turn-one").await;
    r.notice(Notice::Message {
        thread: "source".into(),
        turn: "turn-one".into(),
        update: Some(Ok(Update {
            revision: 3,
            operations: vec![crate::session_map::types::MapOperation::SetCurrent {
                item_id: Some("child".into()),
            }],
        })),
    });
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    let Effect::Changed { result, .. } = next(&mut r).await else {
        panic!()
    };
    assert!(result.unwrap_err().conflict);
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    assert!(!r.has_work());
    assert_eq!(server.await.unwrap().len(), 2);
}
#[tokio::test]
async fn deleting_a_map_clears_its_thread_configuration_before_a_normal_turn() {
    let (api, server) = server(vec![(404, json!({"error":"Map not found"}))]).await;
    let mut r = Runtime::new(api, "codex".into());
    r.map_deleted("source".into());
    let params =
        json!({"threadId":"source","input":[{"type":"text","text":"ordinary conversation"}]});
    r.start(5, params.clone());
    let Effect::Send(resume) = next(&mut r).await else {
        panic!()
    };
    assert_eq!(resume["params"]["developerInstructions"], Value::Null);
    assert_eq!(resume["params"]["dynamicTools"], json!([]));
    incoming(
        &mut r,
        json!({"id":resume["id"],"result":{"thread":{"id":"source"}}}),
    );
    let Effect::Start { id, params: sent } = next(&mut r).await else {
        panic!()
    };
    assert_eq!(id, 5);
    assert_eq!(sent, params);
    assert!(!r.turns.contains_key("source"));
    assert_eq!(server.await.unwrap().len(), 1);
}
#[tokio::test]
async fn restarted_backend_cannot_reuse_an_old_turn_context_or_block_new_turns() {
    let (api, server) = server(vec![
        (200, map(3, "map-one", None)),
        (200, map(6, "replacement-map", None)),
    ])
    .await;
    let mut r = Runtime::new(api, "codex".into());
    incoming(
        &mut r,
        json!({"method":"studio/appServer/status","params":{"state":"ready","generation":1}}),
    );
    r.start(1, json!({"threadId":"source"}));
    configured(&mut r, 1, "old-turn").await;
    incoming(
        &mut r,
        json!({"method":"studio/appServer/status","params":{"state":"ready","generation":2}}),
    );
    assert!(r.turns.is_empty());
    incoming(
        &mut r,
        tool(
            "old-tool",
            "old-call",
            "old-turn",
            json!([{"op":"setCurrent","itemId":"root"}]),
        ),
    );
    let Effect::Send(reply) = next(&mut r).await else {
        panic!()
    };
    assert_eq!(reply["result"]["success"], false);
    r.start(2, json!({"threadId":"source"}));
    configured(&mut r, 2, "new-turn").await;
    assert_eq!(r.turns["source"].version.id, "replacement-map");
    assert_eq!(r.turns["source"].version.revision, 6);
    assert_eq!(server.await.unwrap().len(), 2);
}
#[tokio::test]
async fn an_uncertain_tool_write_cannot_be_replayed_by_the_trailing_fallback() {
    let (api, server) = server(vec![(200, map(3, "map-one", None)), (0, Value::Null)]).await;
    let mut r = Runtime::new(api, "codex".into());
    r.start(1, json!({"threadId":"source"}));
    configured(&mut r, 1, "turn-one").await;
    incoming(
        &mut r,
        tool(
            "tool-one",
            "call-one",
            "turn-one",
            json!([{"op":"setCurrent","itemId":"child"}]),
        ),
    );
    let Effect::Send(reply) = next(&mut r).await else {
        panic!()
    };
    assert_eq!(reply["result"]["success"], false);
    let Effect::Changed { result, .. } = next(&mut r).await else {
        panic!()
    };
    assert!(result.unwrap_err().uncertain);
    r.notice(Notice::Message {
        thread: "source".into(),
        turn: "turn-one".into(),
        update: Some(Ok(Update {
            revision: 3,
            operations: vec![crate::session_map::types::MapOperation::SetCurrent {
                item_id: Some("child".into()),
            }],
        })),
    });
    r.notice(Notice::Completed {
        thread: "source".into(),
        turn: "turn-one".into(),
        completed: true,
    });
    assert!(!r.has_work());
    assert_eq!(server.await.unwrap().len(), 2);
}
