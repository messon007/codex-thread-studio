use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn anchor() -> Anchor {
    Anchor {
        backend: "ept-codex".into(),
        thread: "thread/中文:1".into(),
    }
}
fn value() -> Value {
    let item = |id: &str, parent: Option<&str>, state: &str, position: i64, archived: bool| json!({"id":id,"parentId":parent,"kind":"topic","title":format!("Send {id} 中文"),"summary":"<literal> 😀","state":state,"position":position,"archived":archived,"createdAt":1,"updatedAt":1});
    json!({"id":"map-one","backend":anchor().backend,"threadId":anchor().thread,"goal":"Literal Send goal 中文","definitionOfDone":"Keep exact text","structure":"hierarchy","revision":3,"currentItemId":"child","lastSyncedTurnId":"turn-one","createdAt":1,"updatedAt":1,"relations":[],"items":[
        item("root",None,"notStarted",0,false),item("done",Some("root"),"done",2,false),
        item("child",Some("root"),"active",1,false),item("hidden",None,"done",1,true),item("orphan",Some("hidden"),"notStarted",0,false)
    ]})
}
#[test]
fn outline_visibility_order_trail_and_literal_values_match_shared_maps() {
    let raw = value();
    let snapshot = Snapshot::decode(raw.clone(), &anchor()).unwrap();
    assert_eq!(
        snapshot.rows.as_ref(),
        &[
            Row { index: 0, depth: 0 },
            Row { index: 2, depth: 1 },
            Row { index: 1, depth: 1 },
            Row { index: 4, depth: 0 }
        ]
    );
    assert_eq!(snapshot.trail.as_ref(), &[0, 2]);
    assert_eq!(snapshot.done, 1);
    assert_eq!(serde_json::to_value(&*snapshot.map).unwrap(), raw);
}
#[test]
fn corrupt_or_foreign_maps_never_become_native_rows() {
    let mut raw = value();
    raw["backend"] = "another".into();
    assert!(Snapshot::decode(raw, &anchor()).is_err());
    let mut raw = value();
    raw["items"][2]["parentId"] = "child".into();
    assert!(Snapshot::decode(raw, &anchor()).is_err());
    let mut raw = value();
    raw["items"][2]["id"] = "root".into();
    assert!(Snapshot::decode(raw, &anchor()).is_err());
    let mut raw = value();
    raw["revision"] = 0.into();
    assert!(Snapshot::decode(raw, &anchor()).is_err());
    let mut raw = value();
    raw["items"][0]["summary"] = "x".repeat(4097).into();
    assert!(Snapshot::decode(raw, &anchor()).is_err());
}
#[test]
fn request_identity_query_revisions_and_body_bounds_are_frozen() {
    assert_eq!(
        anchor().path().unwrap(),
        "/studio/session-map/ept%2Dcodex/thread%2F%E4%B8%AD%E6%96%87%3A1"
    );
    for backend in ["../codex", "Codex", "a/b", ""] {
        assert!(Anchor {
            backend: backend.into(),
            ..anchor()
        }
        .path()
        .is_err());
    }
    let request = prepare(Request::Undo {
        anchor: anchor(),
        map_id: "map-one".into(),
        revision: 41,
    })
    .unwrap();
    assert!(request.path.ends_with("/undo?revision=41&mapId=map%2Done"));
    assert_eq!(request.expected, Some(42));
    assert!(request.body.is_none());
    let request = prepare(Request::Delete {
        anchor: anchor(),
        map_id: "map-one".into(),
        revision: 41,
    })
    .unwrap();
    assert!(request.path.ends_with("?revision=41&mapId=map%2Done"));
    assert_eq!(request.expected, Some(41));
    assert!(request.delete);
    let huge = Request::Apply {
        anchor: anchor(),
        map_id: "map-one".into(),
        revision: 3,
        actor: Actor::User,
        source_turn: None,
        operations: vec![MapOperation::SetGoal {
            goal: "x".repeat(513 * 1024),
            definition_of_done: None,
        }],
    };
    assert!(prepare(huge).unwrap_err().contains("512 KiB"));
}
async fn receive(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
        assert!(bytes.len() < 1024 * 1024);
        if let Some(header) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header]).to_ascii_lowercase();
            let length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .map_or(0, |s| s.parse::<usize>().unwrap());
            if bytes.len() >= header + 4 + length {
                return String::from_utf8(bytes).unwrap();
            }
        }
    }
}
async fn server(responses: Vec<(u16, Value)>) -> (Api, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = Api::from_access_url(&format!(
        "http://{}/#token=0123456789abcdef0123456789abcdef",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (status, value) in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(receive(&mut socket).await);
            let body = serde_json::to_vec(&value).unwrap();
            let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
            socket.write_all(header.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
        }
        requests
    });
    (api, task)
}
#[tokio::test]
async fn absence_conflicts_and_wrong_mutation_revisions_stay_distinct() {
    let mut replaced = value();
    replaced["id"] = "replacement-map".into();
    replaced["revision"] = 4.into();
    let (api, task) = server(vec![
        (404, json!({"error":"session map not found"})),
        (409, json!({"error":{"message":"map revision conflict"}})),
        (200, value()),
        (200, replaced),
    ])
    .await;
    assert!(execute(&api, Request::Load(anchor()))
        .await
        .unwrap()
        .is_none());
    let failure = execute(
        &api,
        Request::Undo {
            anchor: anchor(),
            map_id: "map-one".into(),
            revision: 3,
        },
    )
    .await
    .unwrap_err();
    assert!(failure.conflict && !failure.uncertain);
    assert_eq!(failure.message, "map revision conflict");
    let failure = execute(
        &api,
        Request::Apply {
            anchor: anchor(),
            map_id: "map-one".into(),
            revision: 3,
            actor: Actor::User,
            source_turn: None,
            operations: vec![MapOperation::SetCurrent {
                item_id: Some("root".into()),
            }],
        },
    )
    .await
    .unwrap_err();
    assert!(!failure.conflict && failure.uncertain);
    let failure = execute(
        &api,
        Request::Apply {
            anchor: anchor(),
            map_id: "map-one".into(),
            revision: 3,
            actor: Actor::User,
            source_turn: None,
            operations: vec![MapOperation::SetCurrent {
                item_id: Some("root".into()),
            }],
        },
    )
    .await
    .unwrap_err();
    assert!(failure.uncertain && failure.message.contains("replacement map"));
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[0].starts_with(
        "GET /studio/session-map/ept%2Dcodex/thread%2F%E4%B8%AD%E6%96%87%3A1 HTTP/1.1\r\n"
    ));
    assert!(requests[0].ends_with("\r\n\r\n"));
    assert!(requests[1].starts_with(
        "POST /studio/session-map/ept%2Dcodex/thread%2F%E4%B8%AD%E6%96%87%3A1/undo?revision=3&mapId=map%2Done "
    ));
}
#[tokio::test]
async fn accepted_mutation_with_a_lost_reply_is_not_replayed() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = Api::from_access_url(&format!(
        "http://{}/#token=0123456789abcdef0123456789abcdef",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = receive(&mut socket).await;
        assert!(request.starts_with("DELETE "));
        drop(socket);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(300), listener.accept())
                .await
                .is_err(),
            "A mutation was replayed"
        );
    });
    let error = execute(
        &api,
        Request::Delete {
            anchor: anchor(),
            map_id: "map-one".into(),
            revision: 3,
        },
    )
    .await
    .unwrap_err();
    assert!(error.uncertain && !error.conflict);
    task.await.unwrap();
}

#[tokio::test]
async fn successful_create_update_undo_and_delete_use_the_shared_wire_contract() {
    let mut created = value();
    created["revision"] = 1.into();
    let mut updated = created.clone();
    updated["revision"] = 2.into();
    let mut undone = created.clone();
    undone["revision"] = 3.into();
    let (api, task) = server(vec![
        (201, created),
        (200, updated),
        (200, undone.clone()),
        (200, undone),
    ])
    .await;
    let create = types::CreateMapRequest {
        backend: anchor().backend,
        thread_id: anchor().thread,
        goal: "Literal Send goal 中文".into(),
        definition_of_done: "Keep exact text".into(),
        structure: "hierarchy".into(),
        items: Vec::new(),
    };
    assert_eq!(
        execute(&api, Request::Create(create))
            .await
            .unwrap()
            .unwrap()
            .map
            .revision,
        1
    );
    assert_eq!(
        execute(
            &api,
            Request::Apply {
                anchor: anchor(),
                map_id: "map-one".into(),
                revision: 1,
                actor: Actor::Assistant,
                source_turn: Some("turn-two".into()),
                operations: vec![MapOperation::SetCurrent {
                    item_id: Some("root".into())
                }]
            }
        )
        .await
        .unwrap()
        .unwrap()
        .map
        .revision,
        2
    );
    assert_eq!(
        execute(
            &api,
            Request::Undo {
                anchor: anchor(),
                map_id: "map-one".into(),
                revision: 2
            }
        )
        .await
        .unwrap()
        .unwrap()
        .map
        .revision,
        3
    );
    assert!(execute(
        &api,
        Request::Delete {
            anchor: anchor(),
            map_id: "map-one".into(),
            revision: 3
        }
    )
    .await
    .unwrap()
    .is_none());
    let requests = task.await.unwrap();
    let create: Value =
        serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(create["threadId"], anchor().thread);
    assert_eq!(create["items"], json!([]));
    let update: Value =
        serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        update,
        json!({"mapId":"map-one","baseRevision":1,"actor":"assistant","sourceTurnId":"turn-two","operations":[{"op":"setCurrent","itemId":"root"}]})
    );
}

#[test]
fn the_complete_thousand_item_map_is_prepared_without_silently_dropping_rows() {
    let mut raw = value();
    raw["currentItemId"] = Value::Null;
    raw["items"]=Value::Array((0..1000).map(|i|json!({"id":format!("node-{i}"),"parentId":null,"kind":"item","title":format!("Topic {i} 中文😀"),"summary":"literal text","state":if i%3==0{"done"}else{"notStarted"},"position":1000-i,"archived":false,"createdAt":1,"updatedAt":1})).collect());
    let snapshot = Snapshot::decode(raw.clone(), &anchor()).unwrap();
    assert_eq!(snapshot.rows.len(), 1000);
    assert_eq!(snapshot.rows[0].index, 999);
    assert_eq!(snapshot.done, 334);
    let extra = raw["items"][0].clone();
    raw["items"].as_array_mut().unwrap().push(extra);
    assert!(Snapshot::decode(raw, &anchor())
        .unwrap_err()
        .contains("too many items"));
}
