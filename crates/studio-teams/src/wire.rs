//! Agent-facing MCP contract. The caller and backend identity enter separately from JSON.
use crate::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

struct Fields(Map<String, Value>);
impl Fields {
    fn new(v: Value) -> Result<Self> {
        match v {
            Value::Object(m) => Ok(Self(m)),
            _ => Err(Error::InvalidArgument),
        }
    }
    fn take<T: DeserializeOwned>(&mut self, k: &str) -> Result<T> {
        parse(self.0.remove(k).ok_or(Error::InvalidArgument)?)
    }
    fn optional<T: DeserializeOwned>(&mut self, k: &str) -> Result<Option<T>> {
        self.0.remove(k).map(parse).transpose()
    }
    fn end(self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }
}
fn parse<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|_| Error::InvalidArgument)
}
fn status_name(x: Status) -> &'static str {
    match x {
        Status::Pending => "pending",
        Status::Working => "working",
        Status::Waiting => "waiting",
        Status::Submitted => "submitted",
        Status::Completed => "completed",
        Status::Cancelled => "cancelled",
    }
}
fn status(s: &str) -> Result<Status> {
    [
        Status::Pending,
        Status::Working,
        Status::Waiting,
        Status::Submitted,
        Status::Completed,
        Status::Cancelled,
    ]
    .into_iter()
    .find(|x| status_name(*x) == s)
    .ok_or(Error::InvalidArgument)
}
fn actor(v: Value) -> Result<Actor> {
    let mut a = Fields::new(v)?;
    let value = match a.take::<String>("kind")?.as_str() {
        "user" => Actor::User,
        "member" => Actor::Member(a.take("member_id")?),
        _ => return Err(Error::InvalidArgument),
    };
    a.end()?;
    Ok(value)
}
fn actor_value(a: &Actor) -> Value {
    match a {
        Actor::User => json!({"kind":"user"}),
        Actor::Member(id) => json!({"kind":"member","member_id":id}),
    }
}
fn criterion(v: Value) -> Result<Criterion> {
    let mut a = Fields::new(v)?;
    let c = Criterion {
        id: a.take("criterion_id")?,
        text: a.take("text")?,
    };
    a.end()?;
    Ok(c)
}
fn vec_with<T>(a: &mut Fields, key: &str, decode: fn(Value) -> Result<T>) -> Result<Vec<T>> {
    a.take::<Vec<Value>>(key)?.into_iter().map(decode).collect()
}
fn evidence(v: Value) -> Result<Evidence> {
    let mut a = Fields::new(v)?;
    let kind = match a.take::<String>("kind")?.as_str() {
        "commit" => EvidenceKind::Commit,
        "url" => EvidenceKind::Url,
        "file" => EvidenceKind::File,
        "test" => EvidenceKind::Test,
        "note" => EvidenceKind::Note,
        _ => return Err(Error::InvalidArgument),
    };
    let out = Evidence {
        kind,
        locator: a.take("locator")?,
        description: a.take("description")?,
    };
    a.end()?;
    Ok(out)
}
fn evidence_value(e: &Evidence) -> Value {
    let k = match e.kind {
        EvidenceKind::Commit => "commit",
        EvidenceKind::Url => "url",
        EvidenceKind::File => "file",
        EvidenceKind::Test => "test",
        EvidenceKind::Note => "note",
    };
    json!({"kind":k,"locator":e.locator,"description":e.description})
}
fn criterion_result(v: Value) -> Result<CriterionResult> {
    let mut a = Fields::new(v)?;
    let status = match a.take::<String>("status")?.as_str() {
        "met" => CriterionStatus::Met,
        "not_met" => CriterionStatus::NotMet,
        "not_checked" => CriterionStatus::NotChecked,
        _ => return Err(Error::InvalidArgument),
    };
    let r = CriterionResult {
        criterion_id: a.take("criterion_id")?,
        status,
        explanation: a.take("explanation")?,
        evidence_indexes: a.take("evidence_indexes")?,
    };
    a.end()?;
    Ok(r)
}
fn assessment_item(v: Value) -> Result<AssessmentItem> {
    let mut a = Fields::new(v)?;
    let verdict = match a.take::<String>("verdict")?.as_str() {
        "pass" => Verdict::Pass,
        "fail" => Verdict::Fail,
        "not_checked" => Verdict::NotChecked,
        _ => return Err(Error::InvalidArgument),
    };
    let r = AssessmentItem {
        criterion_id: a.take("criterion_id")?,
        verdict,
        explanation: a.take("explanation")?,
        evidence: vec_with(&mut a, "evidence", evidence)?,
    };
    a.end()?;
    Ok(r)
}
fn assessment(v: Value) -> Result<Assessment> {
    let mut a = Fields::new(v)?;
    let r = Assessment {
        summary: a.take("summary")?,
        criteria: vec_with(&mut a, "criteria", assessment_item)?,
    };
    a.end()?;
    Ok(r)
}
fn version(a: &mut Fields) -> Result<TaskVersion> {
    Ok(TaskVersion {
        task_id: a.take("task_id")?,
        attempt_id: a.take("attempt_id")?,
        expected_revision: a.take("expected_revision")?,
    })
}
fn update(a: &mut Fields, task: TaskVersion) -> Result<Command> {
    Ok(match a.take::<String>("action")?.as_str() {
        "begin" => Command::Begin { task },
        "resume" => Command::Resume { task },
        "reclaim" => Command::ReclaimById {
            task,
            previous_claim_id: a.take("previous_claim_id")?,
            continuation: a.take("continuation")?,
        },
        "checkpoint" => Command::Checkpoint {
            task,
            continuation: a.take("continuation")?,
        },
        "wait" => {
            let needs_user = match a.take::<String>("reason")?.as_str() {
                "needs_user" => true,
                "external" => false,
                _ => return Err(Error::InvalidArgument),
            };
            Command::Wait {
                task,
                needs_user,
                detail: a.take("detail")?,
                continuation: a.take("continuation")?,
            }
        }
        "resolve_blocker" => {
            let blocker_id = a.take("blocker_id")?;
            let resolution = a.take("resolution")?;
            match a.optional("user_decision_id")? {
                Some(user_decision_id) => Command::ResolveApproved {
                    task,
                    blocker_id,
                    resolution,
                    user_decision_id,
                },
                None => Command::Resolve {
                    task,
                    blocker_id,
                    resolution,
                },
            }
        }
        "cancel" => Command::Cancel {
            task,
            reason: a.take("reason")?,
        },
        _ => return Err(Error::InvalidArgument),
    })
}
fn write_command(name: &str, a: &mut Fields) -> Result<Command> {
    if name == "team_delegate" {
        let parent = match (
            a.optional("parent_task_id")?,
            a.optional("expected_parent_revision")?,
            a.optional("parent_attempt_id")?,
        ) {
            (None, None, None) => None,
            (Some(task_id), Some(expected_revision), Some(attempt_id)) => Some(Parent {
                task_id,
                expected_revision,
                attempt_id,
            }),
            _ => return Err(Error::InvalidArgument),
        };
        return Ok(Command::Delegate {
            delegation: Delegation {
                to_member_id: a.take("to_member_id")?,
                title: a.take("title")?,
                request: a.take("request")?,
                acceptance: vec_with(a, "acceptance", criterion)?,
                dependencies: a.optional("dependency_task_ids")?.unwrap_or_default(),
                parent,
                continuation: a.optional("continuation")?,
            },
        });
    }
    let task = version(a)?;
    Ok(match name {
        "team_task_update" => update(a, task)?,
        "team_ask" => {
            let to = actor(a.take("to")?)?;
            let question = a.take("question")?;
            let blocking = a.take("blocking")?;
            let continuation = a.optional("continuation")?;
            if blocking && continuation.is_none() {
                return Err(Error::InvalidArgument);
            }
            match a.optional("related_question_id")? {
                Some(related_question_id) => Command::AskRelated {
                    task,
                    to,
                    question,
                    blocking,
                    continuation,
                    related_question_id,
                },
                None => Command::Ask {
                    task,
                    to,
                    question,
                    blocking,
                    continuation,
                },
            }
        }
        "team_answer" => Command::Answer {
            task,
            question_id: a.take("question_id")?,
            answer: a.take("answer")?,
        },
        "team_submit_result" => Command::Submit {
            task,
            submission: Submission {
                summary: a.take("summary")?,
                evidence: vec_with(a, "evidence", evidence)?,
                criteria: vec_with(a, "criterion_results", criterion_result)?,
                limitations: a.take("limitations")?,
            },
        },
        "team_review_result" => {
            let accept = match a.take::<String>("decision")?.as_str() {
                "accept" => true,
                "revise" => false,
                _ => return Err(Error::InvalidArgument),
            };
            let requested_changes = a.optional("requested_changes")?;
            if requested_changes.is_some() == accept {
                return Err(Error::InvalidArgument);
            }
            Command::Review {
                task,
                result_id: a.take("result_id")?,
                accept,
                assessment: assessment(a.take("assessment")?)?,
                requested_changes,
            }
        }
        _ => return Err(Error::InvalidArgument),
    })
}
fn result_value(r: &TaskResult) -> Value {
    let s = &r.submission;
    let criteria:Vec<_>=s.criteria.iter().map(|c|json!({"criterion_id":c.criterion_id,"status":match c.status{CriterionStatus::Met=>"met",CriterionStatus::NotMet=>"not_met",CriterionStatus::NotChecked=>"not_checked"},"explanation":c.explanation,"evidence_indexes":c.evidence_indexes})).collect();
    json!({"result_id":r.id,"attempt_id":r.attempt_id,"summary":s.summary,"limitations":s.limitations,"evidence":s.evidence.iter().map(evidence_value).collect::<Vec<_>>(),"criterion_results":criteria})
}
fn review_value(r: &Review) -> Value {
    let criteria:Vec<_>=r.assessment.criteria.iter().map(|a|json!({"criterion_id":a.criterion_id,"verdict":match a.verdict{Verdict::Pass=>"pass",Verdict::Fail=>"fail",Verdict::NotChecked=>"not_checked"},"explanation":a.explanation,"evidence":a.evidence.iter().map(evidence_value).collect::<Vec<_>>()})).collect();
    json!({"result_id":r.result_id,"decision":if r.accepted{"accept"}else{"revise"},"assessment":{"summary":r.assessment.summary,"criteria":criteria},"requested_changes":r.requested_changes})
}
fn task_value(t: &Task) -> Value {
    let current_result = t
        .results
        .iter()
        .find(|r| r.attempt_id == t.attempt_id)
        .map(result_value);
    let questions:Vec<_>=t.questions.iter().filter(|q|q.attempt_id==t.attempt_id).map(|q|json!({"question_id":q.id,"attempt_id":q.attempt_id,"asked_by":actor_value(&q.asked_by),"to":actor_value(&q.to),"question":q.text,"blocking":q.blocking,"state":if q.answer.is_some(){"answered"}else if t.status.terminal(){"closed"}else{"open"},"answer":q.answer,"related_question_id":q.related_question_id})).collect();
    let blockers:Vec<_>=t.blockers.iter().map(|b|json!({"blocker_id":b.id,"kind":if b.needs_user{"needs_user"}else{"external"},"state":if b.resolution.is_some(){"resolved"}else{"open"},"detail":b.detail,"resolution":b.resolution})).collect();
    json!({"group_id":t.group_id,"task_id":t.id,"title":t.title,"request":t.request,"acceptance":t.acceptance.iter().map(|c|json!({"criterion_id":c.id,"text":c.text})).collect::<Vec<_>>(),"requester":actor_value(&t.requester),"reviewer":actor_value(&t.requester),"assignee_member_id":t.assignee,"parent_task_id":t.parent_id,"dependency_task_ids":t.dependencies,"requester_continuation":t.requester_continuation,"assignee_checkpoint":t.checkpoint,"status":status_name(t.status),"attempt_id":t.attempt_id,"revision":t.revision,"ready":t.ready,"wait_generation":t.wait_generation,"claim_id":t.claim_id.as_deref().or(t.claim.as_ref().map(|c|c.id.as_str())),"questions":questions,"blockers":blockers,"result":current_result,"latest_review":t.reviews.last().map(review_value)})
}
fn context_value(g: &Group, c: &Caller) -> Value {
    let members:Vec<_>=g.members.iter().map(|m|json!({"member_id":m.id,"display_name":m.name,"responsibility":m.responsibility,"session_ref":{"backend_id":m.session.backend,"thread_id":m.session.thread}})).collect();
    json!({"group_id":g.id,"name":g.name,"state":if g.archived{"archived"}else if g.paused{"paused"}else{"active"},"revision":g.revision,"roster_version":g.roster_version,"caller":actor_value(&c.actor),"orchestrator_member_id":g.orchestrator,"members":members,"rules":["Read current task revisions before acting.","Waiting applies only to the task.","Submit results for explicit requester review."]})
}
fn receipt_value(r: &Reply) -> Value {
    let mut data = json!({});
    if let Some(t) = &r.task {
        data = json!({"task_id":t.id,"attempt_id":t.attempt_id,"revision":t.revision,"status":status_name(t.status),"ready":t.ready});
    }
    if let Some(id) = &r.entity_id {
        data["entity_id"] = json!(id);
    }
    data
}
fn tool_result(envelope: Value) -> Value {
    json!({"isError":envelope["ok"]==false,"structuredContent":envelope,"content":[{"type":"text","text":envelope.to_string()}]})
}
/// Format a domain rejection as an MCP CallToolResult without exposing database internals.
pub fn error_result(error: &Error, request_id: Option<&str>) -> Value {
    let (code, recovery) = match error {
        Error::InvalidArgument => ("INVALID_ARGUMENT", "none"),
        Error::LimitExceeded => ("LIMIT_EXCEEDED", "user_action"),
        Error::Forbidden => ("FORBIDDEN", "none"),
        Error::NotFound => ("NOT_FOUND", "none"),
        Error::RevisionConflict => ("REVISION_CONFLICT", "reload"),
        Error::StaleAttempt => ("STALE_ATTEMPT", "reload"),
        Error::TaskTerminal => ("TASK_TERMINAL", "reload"),
        Error::InvalidState => ("INVALID_STATE", "reload"),
        Error::Blocked => ("BLOCKERS_OPEN", "reload"),
        Error::GroupPaused => ("GROUP_PAUSED", "user_action"),
        Error::TurnNotActive => ("TURN_NOT_ACTIVE", "reconcile"),
        Error::ClaimConflict => ("CLAIM_CONFLICT", "reload"),
        Error::ReconciliationRequired => ("RECONCILIATION_REQUIRED", "user_action"),
        Error::ResultNotReady => ("RESULT_NOT_READY", "reload"),
        Error::IdempotencyConflict => ("IDEMPOTENCY_CONFLICT", "none"),
        Error::Storage(_) => ("STORAGE_ERROR", "reconcile"),
    };
    let mut e = json!({"ok":false,"protocol_version":1,"error":{"code":code,"message":code,"recovery":recovery}});
    if let Some(r) = request_id {
        e["request_id"] = json!(r)
    }
    tool_result(e)
}
fn write_metadata(cmd: &Command) -> (Option<&'static str>, Option<u64>) {
    let entity_key = match cmd {
        Command::Ask { .. } | Command::AskRelated { .. } => Some("question_id"),
        Command::Wait { .. } => Some("blocker_id"),
        Command::Submit { .. } => Some("result_id"),
        _ => None,
    };
    let parent_revision = match cmd {
        Command::Delegate { delegation } => delegation
            .parent
            .as_ref()
            .and_then(|p| p.expected_revision.checked_add(1)),
        _ => None,
    };
    (entity_key, parent_revision)
}
fn written(
    r: Reply,
    request_id: &str,
    entity_key: Option<&str>,
    parent_revision: Option<u64>,
) -> Value {
    let mut data = receipt_value(&r);
    if let (Some(key), Some(id)) = (entity_key, &r.entity_id) {
        data[key] = json!(id);
    }
    if let Some(rev) = parent_revision {
        data["parent_revision"] = json!(rev);
    }
    let effects: Vec<_> = r
        .effect_ids
        .iter()
        .map(|id| json!({"effect_id":id,"state":"recorded"}))
        .collect();
    tool_result(
        json!({"ok":true,"protocol_version":1,"request_id":request_id,"replayed":r.replayed,"data":data,"effects":effects}),
    )
}
fn read(data: Value) -> Value {
    tool_result(json!({"ok":true,"protocol_version":1,"data":data,"effects":[]}))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    scope: Value,
    after: i64,
    watermark: i64,
}
fn encode_cursor(cursor: &Cursor) -> Result<String> {
    use std::fmt::Write;
    let bytes = serde_json::to_vec(cursor)?;
    let mut token = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(&mut token, "{b:02x}").expect("formatting String is infallible");
    }
    Ok(token)
}
fn decode_cursor(token: String, scope: &Value) -> Result<Cursor> {
    let bytes = token.as_bytes();
    if bytes.is_empty() || bytes.len() > 16384 || !bytes.len().is_multiple_of(2) {
        return Err(Error::InvalidArgument);
    }
    let decoded: Option<Vec<u8>> = bytes
        .chunks_exact(2)
        .map(|c| {
            let hi = (c[0] as char).to_digit(16)?;
            let lo = (c[1] as char).to_digit(16)?;
            Some((16 * hi + lo) as u8)
        })
        .collect();
    let cursor: Cursor = serde_json::from_slice(&decoded.ok_or(Error::InvalidArgument)?)
        .map_err(|_| Error::InvalidArgument)?;
    if &cursor.scope != scope || cursor.after < 0 || cursor.watermark < 0 {
        return Err(Error::InvalidArgument);
    }
    Ok(cursor)
}
struct Page {
    limit: usize,
    token: Option<String>,
    status: Vec<Status>,
    assignee: Option<String>,
    requester: Option<Actor>,
    task_id: Option<String>,
}
impl Page {
    fn parse(a: &mut Fields, history: bool) -> Result<Self> {
        let limit = a.optional("limit")?.unwrap_or(20);
        if !(1..=100).contains(&limit) {
            return Err(Error::InvalidArgument);
        }
        let token = a.optional("cursor")?;
        let mut out = Self {
            limit,
            token,
            status: vec![],
            assignee: None,
            requester: None,
            task_id: None,
        };
        if history {
            out.task_id = Some(a.take("task_id")?)
        } else {
            let statuses: Vec<String> = a.optional("status")?.unwrap_or_default();
            for s in statuses {
                let parsed = status(&s)?;
                if out.status.contains(&parsed) {
                    return Err(Error::InvalidArgument);
                }
                out.status.push(parsed)
            }
            out.status.sort_by_key(|s| status_name(*s));
            out.assignee = a.optional("assignee_member_id")?;
            out.requester = a.optional::<Value>("requester")?.map(actor).transpose()?;
        }
        Ok(out)
    }
    fn run(self, store: &Store, caller: &Caller, g: &Group) -> Result<Value> {
        let Self {
            limit,
            token,
            status,
            assignee,
            requester,
            task_id,
        } = self;
        let scope = json!([
            store.cursor_namespace()?,
            g.id,
            caller.actor,
            task_id,
            status,
            assignee,
            requester
        ]);
        let now = store.read_watermark(&g.id)?;
        let cursor = match token {
            Some(token) => decode_cursor(token, &scope)?,
            None => Cursor {
                scope: scope.clone(),
                after: 0,
                watermark: now,
            },
        };
        if cursor.watermark > now || (task_id.is_none() && cursor.watermark != now) {
            return Err(Error::RevisionConflict);
        }
        let mut entries = if let Some(task_id) = task_id {
            let task = store.task(caller, &g.id, &task_id)?;
            store.wire_history(&g.id,&task_id,cursor.after,cursor.watermark,limit+1)?.into_iter().map(|e|{
                let mut data=e.data;
                if e.kind=="task_changed"{
                    if let Some(result)=data["entity_id"].as_str().and_then(|rid|task.results.iter().find(|r|r.id==rid)){
                        data["result"]=result_value(result);
                    }
                    if let Some(rid)=data["command"]["result_id"].as_str(){
                        if let Some(review)=task.reviews.iter().find(|r|r.result_id==rid){data["review"]=review_value(review);}
                    }
                }
                (e.seq,json!({"seq":e.seq,"actor":actor_value(&e.actor),"kind":e.kind,"data":data}))
            }).collect()
        } else {
            let mut rows = store.wire_tasks(
                &g.id,
                cursor.after,
                &status,
                assignee.as_deref(),
                requester.as_ref(),
                limit + 1,
            )?;
            for (_, row) in &mut rows {
                let a: Actor = parse(row["requester"].take())?;
                row["requester"] = actor_value(&a);
            }
            if store.read_watermark(&g.id)? != now {
                return Err(Error::RevisionConflict);
            }
            rows
        };
        let next = if entries.len() > limit {
            entries.pop();
            Some(encode_cursor(&Cursor {
                scope,
                after: entries.last().expect("nonempty page").0,
                watermark: cursor.watermark,
            })?)
        } else {
            None
        };
        Ok(
            json!({"items":entries.into_iter().map(|(_,v)|v).collect::<Vec<_>>(),"next_cursor":next}),
        )
    }
}
/// Execute a named MCP tool. Only the trusted adapter supplies Caller; domain rejections
/// use an MCP error result while malformed parameters/unknown tools return InvalidArgument.
pub fn call_tool(store: &mut Store, caller: &Caller, name: &str, args: Value) -> Result<Value> {
    if serde_json::to_vec(&args)?.len() > 131072 {
        return Ok(error_result(&Error::LimitExceeded, None));
    }
    let mut a = Fields::new(args)?;
    let group_id: String = a.take("group_id")?;
    crate::store::identifier(&group_id)?;
    let g = match store.authorize_tool(caller, &group_id) {
        Ok(g) => g,
        Err(Error::NotFound) if matches!(caller.actor, Actor::Member(_)) => {
            return Ok(error_result(&Error::Forbidden, None))
        }
        Err(e) => return Ok(error_result(&e, None)),
    };
    let outcome = match name {
        "team_context" => {
            a.end()?;
            Ok(context_value(&g, caller))
        }
        "team_task_get" => {
            let id: String = a.take("task_id")?;
            a.end()?;
            store.task(caller, &g.id, &id).map(|t| task_value(&t))
        }
        "team_task_list" | "team_task_history" => {
            let p = Page::parse(&mut a, name == "team_task_history")?;
            a.end()?;
            p.run(store, caller, &g)
        }
        "team_command_get" => {
            let id: String = a.take("request_id")?;
            a.end()?;
            store
                .command_details(caller, &g.id, &id)
                .map(|saved| match saved {
                    Some((cmd, Ok(reply))) => {
                        let (key, parent) = write_metadata(&cmd);
                        let mut v = written(reply, &id, key, parent);
                        json!({"found":true,"response":v["structuredContent"].take()})
                    }
                    Some((_, Err(err))) => {
                        let mut v = error_result(&err, Some(&id));
                        json!({"found":true,"response":v["structuredContent"].take()})
                    }
                    None => json!({"found":false}),
                })
        }
        "team_delegate" | "team_task_update" | "team_ask" | "team_answer"
        | "team_submit_result" | "team_review_result" => {
            let id: String = a.take("request_id")?;
            let cmd = write_command(name, &mut a)?;
            a.end()?;
            let (entity_key, parent_revision) = write_metadata(&cmd);
            return Ok(match store.execute(caller, &group_id, &id, cmd) {
                Ok(r) => written(r, &id, entity_key, parent_revision),
                Err(e) => error_result(&e, Some(&id)),
            });
        }
        _ => return Err(Error::InvalidArgument),
    };
    Ok(match outcome {
        Ok(data) => read(data),
        Err(e) => error_result(&e, None),
    })
}
