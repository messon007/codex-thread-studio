use super::*;
use serde_json::json;
fn block(revision: u64) -> String {
    format!("{START}\n{{\"baseRevision\":{revision},\"operations\":[{{\"op\":\"setCurrent\",\"itemId\":\"root\"}}]}}\n{END}")
}
fn delta(text: &str) -> Value {
    json!({"method":"item/agentMessage/delta","params":{"threadId":"source","turnId":"turn","itemId":"message","delta":text}})
}
#[test]
fn every_marker_split_hides_private_stream_data_and_keeps_exact_visible_unicode() {
    let prefix = "Visible **answer** 中文😀\n\n";
    let private = block(7);
    for split in 0..=START.len() {
        let mut streams = Streams::default();
        let mut displayed = String::new();
        for chunk in [prefix, &private[..split], &private[split..]] {
            let mut payload = delta(chunk);
            streams.process(&mut payload);
            displayed.push_str(payload["params"]["delta"].as_str().unwrap());
        }
        assert_eq!(displayed.trim_end(), prefix.trim_end());
        assert!(!displayed.contains("baseRevision"));
        let raw = format!("{prefix}{private}");
        let mut payload = json!({"method":"item/completed","params":{"threadId":"source","turnId":"turn","item":{"id":"message","type":"agentMessage","text":raw}}});
        let notices = streams.process(&mut payload);
        assert!(matches!(
            &notices[0],
            Notice::Message {
                update: Some(Ok(Update { revision: 7, .. })),
                ..
            }
        ));
        let model = crate::model::Item::from_value(&payload["params"]["item"]);
        assert_eq!(model.body, prefix.trim_end());
        assert_eq!(
            crate::model::Item::source_content(&payload["params"]["item"]).1,
            prefix.trim_end()
        );
        assert_eq!(streams.bytes, 0);
    }
}
#[test]
fn malformed_stale_shaped_or_unsafe_suffixes_are_never_accepted_as_operations() {
    for text in [format!("{START}{{}}{END}"),format!("{START}invalid{END}"),format!("{START}{{\"baseRevision\":-1,\"operations\":[]}}{END}"),format!("{START}{{\"baseRevision\":1.5,\"operations\":[]}}{END}"),format!("{} trailing",block(1)),format!("{START}{{\"baseRevision\":1,\"operations\":[{{\"op\":\"setState\",\"itemId\":\"root\",\"state\":\"done\"}}]}}{END}"),format!("{START}{{\"baseRevision\":1,\"operations\":[]}}")]{assert!(parse(&text).unwrap().is_err(),"{text}");assert_eq!(visible(&format!("Visible\n{text}")),"Visible");}
    assert!(parse("Normal answer").is_none());
    assert!(parse(&format!(
        "{START}{}{END}",
        "x".repeat(MAX_MAP_BODY_BYTES + 1)
    ))
    .unwrap()
    .is_err());
}
#[test]
fn delta_only_and_turn_summary_providers_produce_one_ordered_final_update() {
    let mut streams = Streams::default();
    let mut p = delta(&format!("Answer\n{}", block(4)));
    streams.process(&mut p);
    let mut p = json!({"method":"turn/completed","params":{"threadId":"source","turn":{"id":"turn","status":"completed"}}});
    let notices = streams.process(&mut p);
    assert!(matches!(
        &notices[0],
        Notice::Message {
            update: Some(Ok(Update { revision: 4, .. })),
            ..
        }
    ));
    assert!(matches!(
        notices.last(),
        Some(Notice::Completed {
            completed: true,
            ..
        })
    ));
    assert_eq!(streams.bytes, 0);
    let mut p = json!({"method":"turn/completed","params":{"threadId":"source","turn":{"id":"summary-turn","status":"completed","items":[{"type":"agentMessage","text":format!("Final answer\n{}",block(8))}]}}});
    let notices = streams.process(&mut p);
    assert!(matches!(
        &notices[0],
        Notice::Message {
            update: Some(Ok(Update { revision: 8, .. })),
            ..
        }
    ));
    assert_eq!(p["params"]["turn"]["items"][0]["text"], "Final answer");
}
#[test]
fn filter_memory_is_bounded_even_for_unfinished_or_hostile_streams() {
    let mut streams = Streams::default();
    for i in 0..200 {
        let mut p = delta(&format!("{START}{}", "x".repeat(100_000)));
        p["params"]["itemId"] = format!("item-{i}").into();
        streams.process(&mut p);
        assert_eq!(p["params"]["delta"], "");
    }
    assert!(streams.bytes <= 4 * 1024 * 1024);
    assert!(streams.items.len() <= 128);
    let mut p = delta(&"x".repeat(MAX_MAP_BODY_BYTES + 1));
    p["params"]["itemId"] = "item-0".into();
    streams.process(&mut p);
    assert_eq!(p["params"]["delta"], "");
    let model = crate::model::Item::from_value(
        &json!({"type":"userMessage","content":[{"type":"text","text":format!("Literal {START}")}] }),
    );
    assert_eq!(model.body, format!("Literal {START}"));
}
#[test]
fn backend_restart_discards_partial_private_stream_state() {
    let mut streams = Streams::default();
    let mut ready =
        json!({"method":"studio/appServer/status","params":{"state":"ready","generation":1}});
    streams.process(&mut ready);
    let mut p = delta(START);
    streams.process(&mut p);
    assert!(!streams.items.is_empty());
    let mut status =
        json!({"method":"studio/appServer/status","params":{"state":"ready","generation":2}});
    streams.process(&mut status);
    assert!(streams.items.is_empty());
    assert_eq!(streams.bytes, 0);
    let mut p = delta("Fresh visible response");
    streams.process(&mut p);
    assert_eq!(p["params"]["delta"], "Fresh visible response");
}
