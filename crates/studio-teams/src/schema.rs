use serde_json::{json, Map, Value};
use std::sync::OnceLock;

fn object(properties: &[(&str, Value)], required: &[&str]) -> Value {
    let fields: Map<String, Value> = properties
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
fn string(max: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max})
}
fn id() -> Value {
    string(128)
}
fn options(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn array(item: Value, limit: usize) -> Value {
    json!({"type":"array","items":item,"maxItems":limit})
}
fn revision() -> Value {
    json!({"type":"integer","minimum":0})
}
fn actor() -> Value {
    json!({"oneOf":[object(&[("kind",options(&["user"]))],&["kind"]),
        object(&[("kind",options(&["member"])),("member_id",id())],&["kind","member_id"])]})
}
fn continuation() -> Value {
    object(
        &[("summary", string(4096)), ("next_action", string(4096))],
        &["summary", "next_action"],
    )
}
fn evidence() -> Value {
    object(
        &[
            ("kind", options(&["commit", "url", "file", "test", "note"])),
            ("locator", string(2048)),
            ("description", string(2048)),
        ],
        &["kind", "locator", "description"],
    )
}
fn criteria() -> Value {
    array(
        object(
            &[("criterion_id", id()), ("text", string(2048))],
            &["criterion_id", "text"],
        ),
        20,
    )
}
fn assessment() -> Value {
    let item = object(
        &[
            ("criterion_id", id()),
            ("verdict", options(&["pass", "fail", "not_checked"])),
            ("explanation", string(2048)),
            ("evidence", array(evidence(), 20)),
        ],
        &["criterion_id", "verdict", "explanation", "evidence"],
    );
    object(
        &[("summary", string(32768)), ("criteria", array(item, 20))],
        &["summary", "criteria"],
    )
}
fn tool(
    name: &str,
    description: &str,
    write: bool,
    version: bool,
    fields: &[(&str, Value)],
    required: &[&str],
) -> Value {
    let mut properties = vec![("group_id", id())];
    // Native team_context can discover the single group bound to its authenticated session.
    let mut keys = if name == "team_context" {
        vec![]
    } else {
        vec!["group_id"]
    };
    if write {
        properties.push(("request_id", id()));
        keys.push("request_id");
    }
    if version {
        properties.extend([
            ("task_id", id()),
            ("attempt_id", id()),
            ("expected_revision", revision()),
        ]);
        keys.extend(["task_id", "attempt_id", "expected_revision"]);
    }
    properties.extend_from_slice(fields);
    keys.extend_from_slice(required);
    json!({"name":name,"description":description,"inputSchema":object(&properties,&keys),"annotations":{"readOnlyHint":!write}})
}
fn catalog() -> Value {
    let page = [
        ("cursor", string(16384)),
        (
            "limit",
            json!({"type":"integer","minimum":1,"maximum":100,"default":20}),
        ),
    ];
    let result_item = object(
        &[
            ("criterion_id", id()),
            ("status", options(&["met", "not_met", "not_checked"])),
            ("explanation", string(2048)),
            ("evidence_indexes", array(revision(), 20)),
        ],
        &["criterion_id", "status", "explanation", "evidence_indexes"],
    );
    let statuses = [
        "pending",
        "working",
        "waiting",
        "submitted",
        "completed",
        "cancelled",
    ];
    let tools=vec![
        tool("team_context","If you do not know your group ID, call this tool with {}. It discovers the group bound to your current session and returns your identity, roster, responsibilities and coordinator.",false,false,&[],&[]),
        tool("team_task_get","Read the current authoritative task and claim before deciding what to do.",false,false,&[("task_id",id())],&["task_id"]),
        tool("team_task_history","Page through this task's recorded history, including prior submissions and decisions.",false,false,&[("task_id",id()),page[0].clone(),page[1].clone()],&["task_id"]),
        tool("team_task_list","List authorized team tasks; optional filters compose by intersection.",false,false,&[page[0].clone(),page[1].clone(),("status",array(options(&statuses),6)),("assignee_member_id",id()),("requester",actor())],&[]),
        tool("team_command_get","Look up only your own prior write by its request ID.",false,false,&[("request_id",id())],&["request_id"]),
        tool("team_delegate","Delegate a task with explicit acceptance criteria. To create a child, supply all three parent fields.",true,false,&[("to_member_id",id()),("title",string(200)),("request",string(32768)),("acceptance",criteria()),("continuation",continuation()),("dependency_task_ids",array(id(),20)),("parent_task_id",id()),("expected_parent_revision",revision()),("parent_attempt_id",id())],&["to_member_id","title","request","acceptance"]),
        tool("team_task_update","Begin/resume take no extra fields. Checkpoint needs continuation. Wait needs reason, detail and continuation. Resolve_blocker needs blocker_id and resolution. Reclaim needs previous_claim_id and continuation. Cancel needs reason.",true,true,&[("action",options(&["begin","resume","checkpoint","wait","resolve_blocker","reclaim","cancel"])),("reason",string(32768)),("detail",string(32768)),("continuation",continuation()),("blocker_id",id()),("resolution",string(32768)),("user_decision_id",id()),("previous_claim_id",id())],&["action"]),
        tool("team_ask","Ask a specific member or user. Blocking questions require a continuation and affect only this task.",true,true,&[("to",actor()),("question",string(32768)),("blocking",json!({"type":"boolean"})),("continuation",continuation()),("related_question_id",id())],&["to","question","blocking"]),
        tool("team_answer","Answer an open question addressed to you.",true,true,&[("question_id",id()),("answer",string(32768))],&["question_id","answer"]),
        tool("team_submit_result","Submit once per attempt with coverage of every acceptance criterion; review is separate.",true,true,&[("summary",string(32768)),("evidence",array(evidence(),20)),("criterion_results",array(result_item,20)),("limitations",json!({"type":"string","maxLength":32768}))],&["summary","evidence","criterion_results","limitations"]),
        tool("team_review_result","Accept or revise a submitted result with a per-criterion assessment. Revise requires requested_changes.",true,true,&[("result_id",id()),("decision",options(&["accept","revise"])),("assessment",assessment()),("requested_changes",string(32768))],&["result_id","decision","assessment"]),
    ];
    json!({"tools":tools})
}
/// MCP tools/list result, independent of a particular member's credentials.
pub fn tools_list() -> Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(catalog).clone()
}
