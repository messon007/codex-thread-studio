use serde_json::{json, Value};
use studio_teams::queue::{Queue, State};
use studio_teams::*;

fn session(id: &str) -> Session {
    Session {
        backend: "codex-a".into(),
        thread: id.into(),
    }
}
fn draft(id: &str) -> MemberDraft {
    MemberDraft {
        session: session(id),
        name: id.into(),
        responsibility: "work".into(),
    }
}
fn delegate(g: &Group, who: usize, r: &str) -> Value {
    json!({"group_id":g.id,"request_id":r,"to_member_id":g.members[who].id,"title":"Do work","request":"Inspect","acceptance":[{"criterion_id":"c1","text":"verified"}]})
}
fn submit(store: &mut Store, c: &Caller, name: &str, args: Value) -> Value {
    let e = call_tool(store, c, name, args).unwrap();
    assert_eq!(e["isError"], false, "{e}");
    let v = e["structuredContent"].clone();
    assert_eq!(
        serde_json::from_str::<Value>(e["content"][0]["text"].as_str().unwrap()).unwrap(),
        v
    );
    v
}
#[test]
fn flat_native_tools_cover_delegate_question_submit_review_and_history() {
    let mut s = Store::open(":memory:").unwrap();
    let user = Caller::user();
    let g = s
        .create_group(&user, "new", "Team", vec![draft("A"), draft("B")])
        .unwrap();
    let turn = Turn {
        session: session("B"),
        generation: "1".into(),
        id: "b-turn".into(),
    };
    let b = Caller::member(&g.members[1].id, turn.clone());
    s.observe_turn(&turn, TurnState::Active).unwrap();
    assert_eq!(s.groups().unwrap().len(), 1);
    assert_eq!(
        s.member_for_session(&session("B")).unwrap().unwrap().1.id,
        g.members[1].id
    );
    let tools = tools_list();
    assert_eq!(tools["tools"].as_array().unwrap().len(), 11);
    for tool in tools["tools"].as_array().unwrap() {
        let requires_group = tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key == "group_id");
        assert_eq!(requires_group, tool["name"] != "team_context");
    }
    let ctx = submit(&mut s, &b, "team_context", json!({"group_id":g.id}));
    assert_eq!(ctx["data"]["caller"]["member_id"], g.members[1].id);
    let mut bad = delegate(&g, 1, "bad");
    bad["caller"] = json!("user");
    assert_eq!(
        call_tool(&mut s, &b, "team_delegate", bad).unwrap_err(),
        Error::InvalidArgument
    );
    let t = submit(&mut s, &user, "team_delegate", delegate(&g, 1, "d"))["data"].clone();
    let id = t["task_id"].as_str().unwrap();
    let args = |r: &str, rev: u64| json!({"group_id":g.id,"request_id":r,"task_id":id,"attempt_id":t["attempt_id"],"expected_revision":rev});
    let mut a = args("begin", 1);
    a["action"] = json!("begin");
    submit(&mut s, &b, "team_task_update", a);
    let mut a = args("ask", 2);
    a["to"] = json!({"kind":"user"});
    a["question"] = json!("color?");
    a["blocking"] = json!(false);
    let q = submit(&mut s, &b, "team_ask", a);
    assert_eq!(
        submit(
            &mut s,
            &b,
            "team_command_get",
            json!({"group_id":g.id,"request_id":"ask"})
        )["data"]["response"],
        q
    );
    let mut a = args("answer", 3);
    a["question_id"] = q["data"]["question_id"].clone();
    a["answer"] = json!("blue");
    submit(&mut s, &user, "team_answer", a);
    let mut a = args("submit", 4);
    a["summary"] = json!("Done");
    a["limitations"] = json!("");
    a["evidence"] = json!([{"kind":"note","locator":"sample","description":"pass"}]);
    a["criterion_results"] = json!([{"criterion_id":"c1","status":"met","explanation":"checked","evidence_indexes":[0]}]);
    let r = submit(&mut s, &b, "team_submit_result", a);
    s.observe_turn(&turn, TurnState::Succeeded).unwrap();
    let mut a = args("accept", 5);
    a["result_id"] = r["data"]["result_id"].clone();
    a["decision"] = json!("accept");
    a["assessment"] = json!({"summary":"verified","criteria":[{"criterion_id":"c1","verdict":"pass","explanation":"okay","evidence":[]}]});
    let final_reply = submit(&mut s, &user, "team_review_result", a.clone());
    assert_eq!(final_reply["data"]["status"], "completed");
    assert_eq!(
        submit(&mut s, &user, "team_review_result", a)["replayed"],
        true
    );
    let get = submit(
        &mut s,
        &user,
        "team_task_get",
        json!({"group_id":g.id,"task_id":id}),
    )["data"]
        .clone();
    assert_eq!(get["questions"][0]["answer"], "blue");
    assert_eq!(get["result"]["evidence"][0]["kind"], "note");
    assert_eq!(get["acceptance"][0]["criterion_id"], "c1");
    let history = submit(
        &mut s,
        &user,
        "team_task_history",
        json!({"group_id":g.id,"task_id":id}),
    );
    assert!(history["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["data"]["result"]["result_id"] == r["data"]["result_id"]));
    let old = submit(
        &mut s,
        &user,
        "team_command_get",
        json!({"group_id":g.id,"request_id":"accept"}),
    );
    assert_eq!(old["data"]["response"]["data"]["status"], "completed");
    let list = submit(
        &mut s,
        &user,
        "team_task_list",
        json!({"group_id":g.id,"status":["completed"],"requester":{"kind":"user"}}),
    );
    assert_eq!(list["data"]["items"][0]["requester"]["kind"], "user");
    assert_eq!(list["data"]["items"][0]["ready"], false);
}
#[test]
fn pagination_is_bound_to_filters_actor_scope_and_mutation_watermark() {
    let mut s = Store::open(":memory:").unwrap();
    let u = Caller::user();
    let g = s
        .create_group(&u, "group", "Team", vec![draft("1"), draft("2")])
        .unwrap();
    for i in 0..3 {
        submit(
            &mut s,
            &u,
            "team_delegate",
            delegate(&g, 0, &format!("d{i}")),
        );
    }
    let first = submit(
        &mut s,
        &u,
        "team_task_list",
        json!({"group_id":g.id,"limit":1}),
    )["data"]
        .clone();
    let a = json!({"group_id":g.id,"limit":1,"cursor":first["next_cursor"]});
    let second = submit(&mut s, &u, "team_task_list", a.clone());
    assert_ne!(
        second["data"]["items"][0]["task_id"],
        first["items"][0]["task_id"]
    );
    let mut wrong = a.clone();
    wrong["assignee_member_id"] = json!("other");
    assert_eq!(
        call_tool(&mut s, &u, "team_task_list", wrong).unwrap()["isError"],
        true
    );
    submit(&mut s, &u, "team_delegate", delegate(&g, 0, "another"));
    assert_eq!(
        call_tool(&mut s, &u, "team_task_list", a).unwrap()["structuredContent"]["error"]["code"],
        "REVISION_CONFLICT"
    );
}
#[test]
fn queue_host_reads_and_resume_only_requeue_proven_withdrawals() {
    let mut q = Queue::open(":memory:").unwrap();
    let s = session("a");
    q.activate("scope", 1).unwrap();
    let e = q
        .admit(s.clone(), "teams", "delivery", "scope", 1, "body")
        .unwrap();
    q.set_ready(&s, true).unwrap();
    assert_eq!(
        q.by_stable_id(&s, "teams", "delivery").unwrap().unwrap().id,
        e.id
    );
    let reserve = q.reserve(&s, "owner", 10, "paused").unwrap();
    assert_eq!(q.reservations(&s).unwrap()[0].id, reserve.id);
    q.release(&reserve.id, 10).unwrap();
    let pre = q.authorize_head(&s).unwrap().unwrap();
    q.withdraw_local(&pre.id).unwrap();
    assert!(q
        .prepare_send(&e.id, pre.dispatch_id.as_deref().unwrap())
        .is_err());
    q.fence("scope", 1, "pause").unwrap();
    q.activate("scope", 2).unwrap();
    let again = q.requeue_withdrawn(&e.id, 2).unwrap();
    assert_eq!(again.seq, e.seq);
    assert_eq!(again.state, State::Queued);
    assert!(again.dispatch_id.is_none());
    assert!(q.requeue_withdrawn(&e.id, 2).is_err());
    let pre = q.authorize_head(&s).unwrap().unwrap();
    let d = pre.dispatch_id.as_deref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.accepted(&e.id, d, "native").unwrap();
    assert!(q.withdraw_local(&e.id).is_err());
    q.withdrawn(&e.id, "native").unwrap();
    q.fence("scope", 2, "pause2").unwrap();
    q.activate("scope", 3).unwrap();
    assert!(q
        .requeue_withdrawn(&e.id, 3)
        .unwrap()
        .native_queue_id
        .is_none());
    assert_eq!(q.all_pending().unwrap().len(), 1);
}
