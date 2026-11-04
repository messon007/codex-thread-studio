use super::*;
use crate::session_search::Context;
use serde_json::json;
use std::collections::VecDeque;

fn source() -> Source {
    Source {
        context: Context {
            backend: "codex".into(),
            thread: "source".into(),
            generation: 7,
        },
        backend_kind: "codex".into(),
        directory: "/private/project".into(),
    }
}
fn summary() -> Value {
    json!({"thread":{"id":"source","cwd":"/private/project","status":{"type":"idle"}}})
}
fn states() -> Value {
    json!({"data":[{"id":"latest","status":"completed"},{"id":"old","status":"completed"}]})
}
fn entry(id: &str, kind: &str, text: &str) -> Value {
    json!({"turnId":"latest","item":{"id":id,"type":kind,"text":text}})
}
fn page() -> Value {
    json!({"data":[entry("answer","agentMessage","A is done; B remains 中文😀"),entry("question","userMessage","Finish A and B")]})
}
struct Script {
    expected: VecDeque<(String, Option<String>, Result<Value, String>)>,
    requests: Vec<String>,
}
impl Script {
    fn new(rows: Vec<(&str, Result<Value, String>)>) -> Self {
        Self {
            expected: rows
                .into_iter()
                .map(|(op, result)| (op.into(), None, result))
                .collect(),
            requests: Vec::new(),
        }
    }
    fn normal() -> Self {
        Self::new(vec![
            ("summary", Ok(summary())),
            ("states", Ok(states())),
            ("items", Ok(page())),
            ("states", Ok(states())),
            ("summary", Ok(summary())),
        ])
    }
}
impl History for Script {
    async fn read(&mut self, operation: &str, cursor: Option<String>) -> Result<Value, String> {
        let (expected, expected_cursor, result) =
            self.expected.pop_front().expect("Unexpected history read");
        assert_eq!(operation, expected);
        assert_eq!(cursor, expected_cursor);
        self.requests.push(operation.into());
        result
    }
}
#[tokio::test]
async fn authoritative_evidence_keeps_unshortened_user_and_latest_assistant_across_item_pages() {
    let mut first = vec![
        entry("empty-final", "agentMessage", ""),
        entry("answer", "agentMessage", &"中文😀".repeat(25_000)),
    ];
    first.extend((0..118).map(|i| {
        entry(
            &format!("tool-{i}"),
            "commandExecution",
            "tool text is not evidence",
        )
    }));
    let mut script = Script::new(vec![
        ("summary", Ok(summary())),
        ("states", Ok(states())),
        (
            "items",
            Ok(json!({"data":first,"nextCursor":"opaque:中文/+?"})),
        ),
        (
            "items",
            Ok(
                json!({"data":[entry("steer","userMessage","Also keep C"),{"turnId":"latest","item":{"id":"question","type":"userMessage","content":[{"type":"text","text":"Original <literal>"},{"type":"image","url":"not text"},{"type":"text","text":" 中文😀 "}]}},{"turnId":"old","item":{"id":"old-question","type":"userMessage","text":"DO NOT include"}}]}),
            ),
        ),
        ("states", Ok(states())),
        ("summary", Ok(summary())),
    ]);
    script.expected[3].1 = Some("opaque:中文/+?".into());
    let snapshot = evidence(&mut script, &source()).await.unwrap();
    assert_eq!(snapshot.context, source().context);
    assert_eq!(
        snapshot.original,
        "Original <literal>\n\n 中文😀 \nAlso keep C"
    );
    assert_eq!(snapshot.reply, "中文😀".repeat(25_000));
    assert!(
        snapshot.reply.len() > 128 * 1024,
        "must exceed the GTK item excerpt limit"
    );
    assert_eq!(snapshot.state, TurnState::Completed);
    assert!(!snapshot.blocked && !snapshot.error);
    assert!(snapshot.active_turn.is_none());
    assert!(script.expected.is_empty());
}
#[tokio::test]
async fn idle_is_not_completion_and_metadata_approval_or_active_state_remains_a_guard() {
    let mut running = states();
    running["data"][0]["status"] = "inProgress".into();
    let mut script = Script::normal();
    script.expected[1].2 = Ok(running.clone());
    script.expected[3].2 = Ok(running);
    let snapshot = evidence(&mut script, &source()).await.unwrap();
    assert_eq!(snapshot.state, TurnState::InProgress);
    assert_eq!(snapshot.active_turn.as_deref(), Some("latest"));
    for status in [
        json!({"type":"active","activeFlags":["waitingOnApproval"]}),
        json!({"type":"active","activeFlags":["waitingOnUserInput"]}),
        json!({"type":"systemError"}),
        json!({"type":"notLoaded"}),
        json!({"type":"active","activeFlags":["future-attention-state"]}),
    ] {
        let mut value = summary();
        value["thread"]["status"] = status;
        let mut script = Script::normal();
        script.expected[4].2 = Ok(value);
        assert!(evidence(&mut script, &source()).await.unwrap().blocked);
    }
    let mut value = summary();
    value["thread"]["status"] = json!({"type":"active","activeFlags":[]});
    let mut script = Script::normal();
    script.expected[4].2 = Ok(value);
    assert!(evidence(&mut script, &source())
        .await
        .unwrap()
        .active_turn
        .is_some());
}
#[tokio::test]
async fn changed_source_identity_project_and_latest_turn_fail_closed() {
    for (field, replacement) in [
        ("id", json!("other")),
        ("cwd", json!("/other/project")),
        ("ephemeral", json!(true)),
        ("archived", json!(true)),
    ] {
        let mut value = summary();
        value["thread"][field] = replacement;
        let mut script = Script::normal();
        script.expected[4].2 = Ok(value);
        assert!(evidence(&mut script, &source()).await.is_err(), "{field}");
    }
    for (field, replacement) in [
        ("id", json!("new-human-turn")),
        ("status", json!("inProgress")),
        ("error", json!({"message":"failure"})),
    ] {
        let mut value = states();
        value["data"][0][field] = replacement;
        let mut script = Script::normal();
        script.expected[3].2 = Ok(value);
        assert!(evidence(&mut script, &source()).await.is_err(), "{field}");
    }
}
#[tokio::test]
async fn provider_compatibility_reads_full_turns_and_legacy_identity_without_display_projection() {
    for legacy in [false, true] {
        let turns = json!([{"id":"latest","status":"completed","items":[{"id":"u","type":"userMessage","text":"Actual original"},{"id":"a","type":"agentMessage","text":"Actual answer"}]}]);
        let full = if legacy {
            json!({"thread":{"id":"source","turns":turns}})
        } else {
            json!({"data":turns})
        };
        let mut rows = vec![
            ("summary", Ok(summary())),
            ("states", Err("method not found".into())),
        ];
        if legacy {
            rows.push(("turns", Err("unknown method".into())));
        }
        rows.push((if legacy { "read" } else { "turns" }, Ok(full)));
        rows.push(("summary", Ok(summary())));
        let mut script = Script::new(rows);
        let snapshot = evidence(&mut script, &source()).await.unwrap();
        assert_eq!(snapshot.original, "Actual original");
        assert_eq!(snapshot.reply, "Actual answer");
        assert!(script.expected.is_empty());
    }
    let mut script = Script::new(vec![
        ("summary", Ok(summary())),
        ("states", Err("method not found".into())),
        ("turns", Err("method not found".into())),
        ("read", Ok(json!({"thread":{"id":"other","turns":[]}}))),
    ]);
    assert!(evidence(&mut script, &source())
        .await
        .unwrap_err()
        .contains("another session"));
}
#[tokio::test]
async fn item_method_fallback_revalidates_the_latest_turn_and_does_not_mask_real_errors() {
    let full = json!({"data":[{"id":"latest","status":"completed","items":[{"id":"u","type":"userMessage","text":"Original"},{"id":"a","type":"agentMessage","text":"Reply"}]}]});
    let mut script = Script::new(vec![
        ("summary", Ok(summary())),
        ("states", Ok(states())),
        ("items", Err("unsupported method".into())),
        ("turns", Ok(full)),
        ("states", Ok(states())),
        ("summary", Ok(summary())),
    ]);
    assert_eq!(
        evidence(&mut script, &source()).await.unwrap().reply,
        "Reply"
    );
    let mut script = Script::new(vec![
        ("summary", Ok(summary())),
        ("states", Ok(states())),
        ("items", Err("connection closed".into())),
    ]);
    assert_eq!(
        evidence(&mut script, &source()).await.unwrap_err(),
        "connection closed"
    );
    assert!(script.expected.is_empty());
}
#[test]
fn source_states_require_one_valid_latest_active_turn_and_known_statuses() {
    for value in [
        json!({"data":[]}),
        json!({"data":[{"id":"bad\n","status":"completed"}]}),
        json!({"data":[{"id":"t","status":"unknown"}]}),
        json!({"data":[{"id":"t","status":"completed"},{"id":"old","status":"inProgress"}]}),
        json!({"data":[{"id":"t","status":"inProgress"},{"id":"old","status":"inProgress"}]}),
        json!({"data":[{"id":"t","status":"completed"},{"id":"t","status":"completed"}]}),
    ] {
        assert!(Head::states(value).is_err());
    }
    let mut changed = source();
    changed.backend_kind = "opencode".into();
    assert!(check_source(&changed).is_err());
    changed = source();
    changed.context.thread = "".into();
    assert!(check_source(&changed).is_err());
}
#[test]
fn source_text_and_item_pages_are_bounded_and_never_truncated_or_silently_overlapped() {
    let mut texts = Texts::new("latest".into());
    assert!(texts
        .page(json!({"data":[],"nextCursor":"dangling"}))
        .is_err());
    assert!(texts.page(json!({"data":[],"nextCursor":17})).is_err());
    assert!(texts
        .page(json!({"data":vec![entry("a","userMessage","x");121]}))
        .is_err());
    assert!(texts
        .page(json!({"data":[entry("a","userMessage","x"),entry("a","userMessage","changed")]}))
        .is_err());
    let mut texts = Texts::new("latest".into());
    assert!(texts.page(json!({"data":[{"turnId":"newer","item":{"id":"new","type":"agentMessage","text":"wrong turn"}}]})).is_err());
    let mut texts = Texts::new("latest".into());
    assert!(texts
        .page(json!({"data":[entry("a","agentMessage",&"x".repeat(TEXT_BYTES+1))]}))
        .is_err());
    let mut texts = Texts::new("latest".into());
    assert!(texts.page(json!({"data":[entry("a","userMessage",&"x".repeat(TEXT_BYTES)),entry("b","userMessage","extra")]})).is_err());
    assert!(user_text(&json!({"content":vec![json!({});MAX_PARTS+1]})).is_err());
}
#[tokio::test]
async fn repeated_cursors_stop_reads_before_the_page_budget_and_no_partial_evidence_escapes() {
    let mut script = Script::new(vec![
        ("summary", Ok(summary())),
        ("states", Ok(states())),
        (
            "items",
            Ok(json!({"data":[entry("a","agentMessage","reply")],"nextCursor":"repeat"})),
        ),
        (
            "items",
            Ok(json!({"data":[entry("b","userMessage","original")],"nextCursor":"repeat"})),
        ),
    ]);
    script.expected[3].1 = Some("repeat".into());
    assert!(evidence(&mut script, &source())
        .await
        .unwrap_err()
        .contains("repeated a cursor"));
    assert!(script.expected.is_empty());
}

#[tokio::test]
async fn cancelled_reads_and_invalid_sources_are_rejected_before_transport() {
    let api =
        Api::from_access_url("http://127.0.0.1:1/#token=0123456789abcdef0123456789abcdef").unwrap();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        read(&api, &source(), &cancelled).await.unwrap_err(),
        "Supervision source read cancelled"
    );
    let mut invalid = source();
    invalid.backend_kind = "opencode".into();
    assert!(read(&api, &invalid, &AtomicBool::new(false))
        .await
        .unwrap_err()
        .contains("requires an identified Codex"));
}

#[tokio::test]
async fn evaluator_rejects_excess_context_before_starting_a_utility() {
    let api =
        Api::from_access_url("http://127.0.0.1:1/#token=0123456789abcdef0123456789abcdef").unwrap();
    let context = crate::translation::Context {
        backend: crate::model::Backend {
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
    for text in [
        "x".repeat(TEXT_BYTES + 1),
        "😀".repeat(MAX_CONTEXT_UNITS / 2 + 1),
    ] {
        assert!(evaluate(
            &api,
            1,
            &context,
            "low".into(),
            text,
            &AtomicBool::new(false)
        )
        .await
        .unwrap_err()
        .contains("context is too large"));
    }
}

#[test]
fn assistant_evidence_uses_shared_map_privacy_without_rewriting_user_instructions() {
    let private = format!(
        "{}{}{}",
        crate::session_map_text::START,
        "x".repeat(TEXT_BYTES + 1),
        crate::session_map_text::END
    );
    let original = format!(
        "Explain this literal marker: {}",
        crate::session_map_text::START
    );
    let mut texts = Texts::new("latest".into());
    texts.page(json!({"data":[entry("private-only","agentMessage",&private),entry("answer","agentMessage",&format!("B remains 中文😀\n\n{private}")),entry("question","userMessage",&original)]})).unwrap();
    let head = Head::states(states()).unwrap();
    let snapshot = texts.finish(
        &source(),
        head,
        Metadata {
            active: false,
            blocked: false,
        },
        Metadata {
            active: false,
            blocked: false,
        },
    );
    assert_eq!(snapshot.reply, "B remains 中文😀");
    assert_eq!(snapshot.original, original);
    assert!(!snapshot.reply.contains(crate::session_map_text::START));
    let mut texts = Texts::new("latest".into());
    texts
        .page(json!({"data":[entry("plain","agentMessage","Visible whitespace  \n")]}))
        .unwrap();
    assert_eq!(texts.reply, "Visible whitespace  \n");
}
