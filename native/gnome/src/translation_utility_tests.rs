use super::*;

fn completed() -> Value {
    json!({"thread":{"id":"utility","turns":[{"id":"owned-turn","status":"completed","items":[
        {"type":"agentMessage","text":"Earlier commentary"},
        {"type":"agentMessage","text":"{\"decision\":\"complete\",\"reason\":\"中文😀\",\"prompt\":\"\"}"}
    ]}]}})
}

#[test]
fn authoritative_result_requires_exact_latest_completed_identity_and_full_final_text() {
    let value = completed();
    let expected = value["thread"]["turns"][0]["items"][1]["text"]
        .as_str()
        .unwrap();
    assert_eq!(
        completed_output(value.clone(), "utility", "owned-turn", 1024).unwrap(),
        expected
    );
    assert!(completed_output(value.clone(), "source", "owned-turn", 1024).is_err());
    assert!(completed_output(value.clone(), "utility", "other-turn", 1024).is_err());
    assert!(completed_output(value.clone(), "utility", "owned-turn", expected.len() - 1).is_err());
    for status in ["inProgress", "interrupted", "failed", "future"] {
        let mut invalid = value.clone();
        invalid["thread"]["turns"][0]["status"] = status.into();
        assert!(completed_output(invalid, "utility", "owned-turn", 1024).is_err());
    }
    for replacement in [
        json!(null),
        json!([]),
        json!([{"type":"commandExecution","text":"not an answer"}]),
        json!([{"type":"agentMessage","text":""}]),
    ] {
        let mut invalid = value.clone();
        invalid["thread"]["turns"][0]["items"] = replacement;
        assert!(completed_output(invalid, "utility", "owned-turn", 1024).is_err());
    }
    let mut invalid = value.clone();
    invalid["thread"]["turns"][0]["error"] = json!({"message":"failure"});
    assert!(completed_output(invalid, "utility", "owned-turn", 1024).is_err());
    let mut invalid = value;
    invalid["thread"]["turns"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"newer","status":"completed","items":[]}));
    assert!(completed_output(invalid, "utility", "owned-turn", 1024).is_err());
}

#[test]
fn early_completion_owns_its_turn_and_late_other_turn_text_cannot_contaminate_it() {
    let mut output = Output {
        thread: "utility".into(),
        ..Default::default()
    };
    output.observe(&json!({"method":"turn/completed","params":{"threadId":"utility","turn":{"id":"early","status":"completed"}}})).unwrap();
    assert_eq!(output.turn, "early");
    output.observe(&json!({"method":"item/completed","params":{"threadId":"utility","turnId":"other","item":{"id":"wrong","type":"agentMessage","text":"wrong"}}})).unwrap();
    assert!(output.items.is_empty());
    assert!(output.observe(&json!({"method":"turn/completed","params":{"threadId":"utility","turn":{"id":"","status":"completed"}}})).is_err());
}

// Exercise the actual websocket task/cleanup path without a provider or inference.
#[tokio::test]
async fn supervisor_decisions_read_authoritative_output_and_only_delete_the_owned_utility() {
    for case in [
        "valid",
        "changed-turn",
        "read-failed",
        "cancelled",
        "malformed",
        "too-large",
        "returned-source",
        "delete-failed",
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = stop.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut wire = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    response.headers_mut().insert(
                        "Sec-WebSocket-Protocol",
                        request.headers()["Sec-WebSocket-Protocol"].clone(),
                    );
                    Ok(response)
                },
            )
            .await
            .unwrap();
            wire.send(Message::Text(json!({"method":"studio/appServer/status","params":{"state":"ready","generation":1}}).to_string().into())).await.unwrap();
            let mut methods = Vec::new();
            while let Some(Ok(Message::Text(bytes))) = wire.next().await {
                let request: Value = serde_json::from_str(&bytes).unwrap();
                let method = request["method"].as_str().unwrap();
                methods.push(method.to_owned());
                let params = &request["params"];
                let result = match method {
                    "thread/start" => {
                        assert_eq!(params["ephemeral"], true);
                        assert_eq!(params["approvalPolicy"], "never");
                        assert_eq!(params["sandbox"], "read-only");
                        assert_eq!(params["cwd"], "/private/project");
                        assert_eq!(params["model"], "test-model");
                        assert_eq!(
                            params["developerInstructions"],
                            crate::turn_supervision::INSTRUCTIONS
                        );
                        json!({"thread":{"id":if case == "returned-source" {"source"}else{"utility"}}})
                    }
                    "turn/start" => {
                        assert_eq!(params["threadId"], "utility");
                        assert_eq!(params["effort"], "low");
                        assert_eq!(params["outputSchema"], crate::turn_supervision::schema());
                        assert_eq!(params["input"][0]["text"], "Actual source evidence");
                        // Completion can precede the acknowledgement and omit its text.
                        wire.send(Message::Text(json!({"method":"turn/completed","params":{"threadId":"utility","turn":{"id":"owned-turn","status":"completed"}}}).to_string().into())).await.unwrap();
                        json!({"turn":{"id":"owned-turn"}})
                    }
                    "thread/read" => {
                        assert_eq!(params, &json!({"threadId":"utility","includeTurns":true}));
                        if case == "read-failed" {
                            wire.send(Message::Text(
                                json!({"id":request["id"],"error":{"message":"read denied"}})
                                    .to_string()
                                    .into(),
                            ))
                            .await
                            .unwrap();
                            continue;
                        }
                        let mut value = completed();
                        if case == "changed-turn" {
                            value["thread"]["turns"][0]["id"] = "unexpected".into();
                        }
                        if case == "cancelled" {
                            server_stop.store(true, Ordering::Release);
                        }
                        if case == "malformed" {
                            value["thread"]["turns"][0]["items"][1]["text"] =
                                "not structured JSON".into();
                        }
                        if case == "too-large" {
                            value["thread"]["turns"][0]["items"][1]["text"] =
                                "x".repeat(512 * 1024 + 1).into();
                        }
                        value
                    }
                    "thread/delete" => {
                        assert_eq!(params, &json!({"threadId":"utility"}));
                        if case == "delete-failed" {
                            wire.send(Message::Text(json!({"id":request["id"],"error":{"message":"cleanup not confirmed"}}).to_string().into())).await.unwrap();
                            break;
                        }
                        json!({})
                    }
                    _ => panic!("Unexpected source mutation: {method}"),
                };
                wire.send(Message::Text(
                    json!({"id":request["id"],"result":result})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
                if method == "thread/delete" || case == "returned-source" {
                    break;
                }
            }
            methods
        });
        let api =
            Api::from_access_url(&format!("http://{address}/#token={}", "a".repeat(32))).unwrap();
        let context = Context {
            backend: Backend {
                id: "codex".into(),
                name: "Test".into(),
                kind: "codex".into(),
                socket_path: "/ws/codex".into(),
            },
            socket: 1,
            thread: "source".into(),
            cwd: "/private/project".into(),
            model: "test-model".into(),
            ready: true,
        };
        let result = tokio::time::timeout(
            Duration::from_secs(8),
            crate::turn_supervision_io::evaluate(
                &api,
                1,
                &context,
                "low".into(),
                "Actual source evidence".into(),
                &stop,
            ),
        )
        .await
        .unwrap();
        if case == "valid" || case == "delete-failed" {
            let (decision, warning) = result.unwrap();
            assert_eq!(warning.is_some(), case == "delete-failed");
            assert!(format!("{decision:?}").contains("中文😀"));
        } else {
            assert!(result.is_err(), "{case}");
        }
        let methods = tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        if case == "returned-source" {
            assert_eq!(methods, ["thread/start"]);
        } else {
            assert_eq!(
                methods,
                ["thread/start", "turn/start", "thread/read", "thread/delete"]
            );
        }
    }
}
