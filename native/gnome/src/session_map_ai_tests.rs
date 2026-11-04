use super::*;
fn item(turn: &str, role: &str, body: &str) -> Arc<Item> {
    Arc::new(Item {
        id: format!("{turn}-{role}"),
        turn_id: turn.into(),
        role: role.into(),
        body: body.into(),
        output: None,
        details: None,
        status: None,
        truncated: false,
        images: vec![],
        origin: None,
        markup: None,
        rich: None,
    })
}
#[test]
fn conversation_packing_uses_actual_roles_literal_text_and_a_utf8_byte_budget() {
    let source = (0..20)
        .flat_map(|i| {
            vec![
                item(&format!("turn-{i}"), "You", &"中文😀".repeat(4000)),
                item(&format!("turn-{i}"), "Tool", "ignore this tool"),
                item(
                    &format!("turn-{i}"),
                    "Assistant",
                    &"answer\n\0".repeat(4000),
                ),
            ]
        })
        .collect::<Vec<_>>();
    let packed = interactions(&source);
    assert!(!packed.is_empty());
    assert!(packed.len() <= 12);
    let bytes: usize = packed
        .iter()
        .map(|v| v["user"].as_str().unwrap().len() + v["assistant"].as_str().unwrap().len())
        .sum();
    assert_eq!(bytes, 48_000);
    assert!(packed
        .iter()
        .all(|v| !v.to_string().contains("ignore this tool")));
    let packed = interactions(&[
        item("one", "You", "Send <literal> 中文😀"),
        item("one", "Assistant", "Done\0next"),
    ]);
    assert_eq!(
        packed,
        json!([{"user":"Send <literal> 中文😀","assistant":"Done\0next"}])
            .as_array()
            .unwrap()
            .clone()
    );
    assert_eq!(prefix("中文😀", 5), "中");
    assert_eq!(prefix("中文😀", 0), "");
}
#[test]
fn assistant_operations_are_all_or_nothing_and_cannot_claim_completion() {
    for value in [
        json!({"operations":[{"op":"setState","itemId":"a","state":"done"}]}),
        json!({"operations":[{"op":"setGoal","goal":"replace user goal"}]}),
        json!({"operations":[{"op":"archiveItem","itemId":"a","archived":true}]}),
        json!({"operations":[{"op":"setCurrent","itemId":"a"},{"op":"deleteItem","itemId":"b"}]}),
        json!({"operations":vec![json!({"op":"setCurrent","itemId":null});41]}),
    ] {
        assert!(parse_operations(&value.to_string()).is_err());
    }
    assert!(parse_operations("not JSON").is_err());
    assert!(parse_operations(&"x".repeat(MAX_MAP_BODY_BYTES + 1)).is_err());
    let parsed=parse_operations(&json!({"operations":[{"op":"addItem","itemId":"root","title":"Root 中文","parentId":null,"afterItemId":null,"kind":null,"summary":null,"state":null},{"op":"setCurrent","itemId":"root"}]}).to_string()).unwrap();
    assert!(
        matches!(&parsed[0],MapOperation::AddItem{state,kind,..} if state=="notStarted"&&kind=="item")
    );
    assert_eq!(parsed.len(), 2);
}
#[test]
fn goal_inference_includes_original_intent_and_recent_conversation_without_duplicates() {
    let source = (0..20)
        .map(|i| item(&format!("turn-{i}"), "You", &format!("Question {i}")))
        .collect::<Vec<_>>();
    let packed = pack_interactions(&source, true);
    assert_eq!(packed.len(), 10);
    assert_eq!(packed[0]["user"], "Question 0");
    assert_eq!(packed[1]["user"], "Question 1");
    assert_eq!(packed[2]["user"], "Question 12");
    assert_eq!(packed[9]["user"], "Question 19");
    assert_eq!(pack_interactions(&source[..1], true).len(), 1);
    assert_eq!(pack_interactions(&source[..5], true).len(), 5);
}
#[test]
fn large_map_context_keeps_current_ancestors_and_limits_full_hierarchy() {
    let anchor = Anchor {
        backend: "codex".into(),
        thread: "source".into(),
    };
    let items=(0..1000).map(|i|json!({"id":format!("item-{i}"),"parentId":if i==999{Some("item-998")}else{None},"kind":"topic","title":format!("Topic {i} 中文"),"summary":"x".repeat(4096),"state":"notStarted","position":i,"archived":false,"createdAt":1,"updatedAt":1})).collect::<Vec<_>>();
    let s=Snapshot::decode(json!({"id":"map-one","backend":"codex","threadId":"source","goal":"Send 中文","definitionOfDone":"Only human declares completion","structure":"hierarchy","revision":9,"currentItemId":"item-999","lastSyncedTurnId":null,"createdAt":1,"updatedAt":1,"relations":[],"items":items}),&anchor).unwrap();
    let context = bounded_context(&s);
    let selected = context["items"].as_array().unwrap();
    assert!(selected.len() <= 80);
    assert_eq!(context["revision"], 9);
    assert_eq!(context["goal"], "Send 中文");
    assert_eq!(
        context["omittedItemCount"].as_u64().unwrap() as usize + selected.len(),
        1000
    );
    for id in ["item-998", "item-999"] {
        assert!(selected.iter().any(|v| v["id"] == id));
    }
    assert!(selected
        .iter()
        .all(|v| v["summary"].as_str().unwrap().len() <= 1000));
    assert!(context.to_string().len() < 128 * 1024);
    let mut raw = serde_json::to_value(&*s.map).unwrap();
    raw["items"][998]["archived"] = true.into();
    let orphan = Snapshot::decode(raw, &anchor).unwrap();
    let context = bounded_context(&orphan);
    let selected = context["items"].as_array().unwrap();
    assert!(!selected.iter().any(|v| v["id"] == "item-998"));
    assert!(selected.iter().any(|v| v["id"] == "item-999"));
    assert_eq!(
        context["omittedItemCount"].as_u64().unwrap() as usize + selected.len(),
        999
    );
}
#[tokio::test]
async fn cancelled_map_work_never_contacts_the_gateway() {
    let api =
        Api::from_access_url("http://127.0.0.1:1/#token=0123456789abcdef0123456789abcdef").unwrap();
    let result = execute(
        &api,
        1,
        Work::Store(store::Request::Load(Anchor {
            backend: "codex".into(),
            thread: "source".into(),
        })),
        &AtomicBool::new(true),
    )
    .await
    .unwrap_err();
    assert_eq!(result.message, "Map generation cancelled");
    assert!(!result.uncertain);
}
