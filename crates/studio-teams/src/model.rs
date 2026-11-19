use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    InvalidArgument,
    LimitExceeded,
    Forbidden,
    NotFound,
    RevisionConflict,
    StaleAttempt,
    TaskTerminal,
    InvalidState,
    Blocked,
    GroupPaused,
    TurnNotActive,
    ClaimConflict,
    ReconciliationRequired,
    ResultNotReady,
    IdempotencyConflict,
    Storage(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Storage(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub backend: String,
    pub thread: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    pub session: Session,
    pub generation: String,
    pub id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnState {
    Active,
    Succeeded,
    Failed,
    Interrupted,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actor {
    User,
    Member(String),
}
/// Constructed by the trusted adapter, not deserializable from a tool payload.
#[derive(Debug, Clone)]
pub struct Caller {
    pub actor: Actor,
    pub turn: Option<Turn>,
}
impl Caller {
    pub fn user() -> Self {
        Self {
            actor: Actor::User,
            turn: None,
        }
    }
    pub fn member(id: impl Into<String>, turn: Turn) -> Self {
        Self {
            actor: Actor::Member(id.into()),
            turn: Some(turn),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberDraft {
    pub session: Session,
    pub name: String,
    pub responsibility: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberProfile {
    pub member_id: String,
    pub display_name: String,
    pub responsibility: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub session: Session,
    pub name: String,
    pub responsibility: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub revision: u64,
    pub paused: bool,
    pub members: Vec<Member>,
    pub orchestrator: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default = "initial_roster_version")]
    pub roster_version: u64,
}
fn initial_roster_version() -> u64 {
    1
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Continuation {
    pub summary: String,
    pub next_action: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parent {
    pub task_id: String,
    pub expected_revision: u64,
    pub attempt_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delegation {
    pub to_member_id: String,
    pub title: String,
    pub request: String,
    pub acceptance: Vec<Criterion>,
    pub dependencies: Vec<String>,
    pub parent: Option<Parent>,
    pub continuation: Option<Continuation>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Pending,
    Working,
    Waiting,
    Submitted,
    Completed,
    Cancelled,
}
impl Status {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    #[serde(default)]
    pub related_question_id: Option<String>,
    pub id: String,
    pub attempt_id: String,
    pub asked_by: Actor,
    pub to: Actor,
    pub text: String,
    pub blocking: bool,
    pub answer: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    pub id: String,
    pub needs_user: bool,
    pub detail: String,
    pub resolution: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub locator: String,
    pub description: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceKind {
    Commit,
    Url,
    File,
    Test,
    Note,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriterionStatus {
    Met,
    NotMet,
    NotChecked,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionResult {
    pub criterion_id: String,
    pub status: CriterionStatus,
    pub explanation: String,
    pub evidence_indexes: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    pub summary: String,
    pub evidence: Vec<Evidence>,
    pub criteria: Vec<CriterionResult>,
    pub limitations: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub attempt_id: String,
    pub submission: Submission,
    pub turn: Turn,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Fail,
    NotChecked,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentItem {
    pub criterion_id: String,
    pub verdict: Verdict,
    pub explanation: String,
    pub evidence: Vec<Evidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assessment {
    pub summary: String,
    pub criteria: Vec<AssessmentItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub result_id: String,
    pub accepted: bool,
    pub assessment: Assessment,
    pub requested_changes: Option<String>,
    pub turn: Option<Turn>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub group_id: String,
    pub requester: Actor,
    pub assignee: String,
    pub title: String,
    pub request: String,
    pub acceptance: Vec<Criterion>,
    pub dependencies: Vec<String>,
    pub parent_id: Option<String>,
    pub requester_continuation: Option<Continuation>,
    pub checkpoint: Option<Continuation>,
    pub status: Status,
    pub revision: u64,
    pub attempt_id: String,
    pub attempts: u32,
    #[serde(default)]
    pub extra_attempts: u32,
    #[serde(default)]
    pub reconciled_claim_id: Option<String>,
    #[serde(default)]
    pub reconciled_result_id: Option<String>,
    pub ready: bool,
    pub wait_generation: u64,
    pub claim: Option<Turn>,
    #[serde(default)]
    pub claim_id: Option<String>,
    pub questions: Vec<Question>,
    pub blockers: Vec<Blocker>,
    pub results: Vec<TaskResult>,
    pub reviews: Vec<Review>,
    pub barriers: Vec<Turn>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskVersion {
    pub task_id: String,
    pub expected_revision: u64,
    pub attempt_id: String,
}
/// Internal domain commands. The MCP adapter maps the spec's flat tool parameters here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Command {
    Delegate {
        delegation: Delegation,
    },
    Begin {
        task: TaskVersion,
    },
    Resume {
        task: TaskVersion,
    },
    ReclaimById {
        task: TaskVersion,
        previous_claim_id: String,
        continuation: Continuation,
    },
    AskRelated {
        task: TaskVersion,
        to: Actor,
        question: String,
        blocking: bool,
        continuation: Option<Continuation>,
        related_question_id: String,
    },
    ResolveApproved {
        task: TaskVersion,
        blocker_id: String,
        resolution: String,
        user_decision_id: String,
    },
    Reclaim {
        task: TaskVersion,
        previous_turn: Turn,
        continuation: Continuation,
    },
    Checkpoint {
        task: TaskVersion,
        continuation: Continuation,
    },
    Wait {
        task: TaskVersion,
        needs_user: bool,
        detail: String,
        continuation: Continuation,
    },
    Resolve {
        task: TaskVersion,
        blocker_id: String,
        resolution: String,
    },
    Ask {
        task: TaskVersion,
        to: Actor,
        question: String,
        blocking: bool,
        continuation: Option<Continuation>,
    },
    Answer {
        task: TaskVersion,
        question_id: String,
        answer: String,
    },
    Submit {
        task: TaskVersion,
        submission: Submission,
    },
    Review {
        task: TaskVersion,
        result_id: String,
        accept: bool,
        assessment: Assessment,
        requested_changes: Option<String>,
    },
    Cancel {
        task: TaskVersion,
        reason: String,
    },
    ConfigureGroup {
        expected_revision: u64,
        name: String,
        orchestrator: Option<String>,
        paused: bool,
    },
    SetGroupProfile {
        expected_revision: u64,
        name: Option<String>,
        members: Vec<MemberProfile>,
    },
    AddMember {
        expected_revision: u64,
        member: MemberDraft,
        make_orchestrator: bool,
    },
    ChangeMembers {
        expected_revision: u64,
        add: Vec<MemberDraft>,
        remove_member_ids: Vec<String>,
    },
    ArchiveGroup {
        expected_revision: u64,
    },
    GroupGoal {
        expected_revision: u64,
        to_member_id: Option<String>,
        title: String,
        request: String,
        acceptance: Vec<Criterion>,
        continuation: Option<Continuation>,
    },
    GrantReworkBudget {
        task: TaskVersion,
        additional_attempts: u32,
        reason: String,
    },
    ReconcileExecution {
        task: TaskVersion,
        claim_id: String,
        confirmed_settled: bool,
        evidence: Vec<Evidence>,
    },
    ReconcileResult {
        task: TaskVersion,
        result_id: String,
        allow_review: bool,
        evidence: Vec<Evidence>,
    },
    ReconcileReview {
        task: TaskVersion,
        result_id: String,
        barrier_turn_id: String,
        release: bool,
        evidence: Vec<Evidence>,
    },
    SetPaused {
        expected_revision: u64,
        paused: bool,
    },
    SetOrchestrator {
        expected_revision: u64,
        member_id: Option<String>,
    },
}
impl Command {
    pub(crate) fn version(&self) -> Option<&TaskVersion> {
        match self {
            Self::GrantReworkBudget { task, .. }
            | Self::ReconcileExecution { task, .. }
            | Self::ReconcileResult { task, .. }
            | Self::ReconcileReview { task, .. }
            | Self::Begin { task }
            | Self::Resume { task }
            | Self::Reclaim { task, .. }
            | Self::ReclaimById { task, .. }
            | Self::AskRelated { task, .. }
            | Self::ResolveApproved { task, .. }
            | Self::Checkpoint { task, .. }
            | Self::Wait { task, .. }
            | Self::Resolve { task, .. }
            | Self::Ask { task, .. }
            | Self::Answer { task, .. }
            | Self::Submit { task, .. }
            | Self::Review { task, .. }
            | Self::Cancel { task, .. } => Some(task),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    pub entity_id: Option<String>,
    pub id: String,
    pub group_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub recipient: Actor,
    pub purpose: String,
    pub barriers: Vec<Turn>,
    pub withdrawn: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReceipt {
    pub id: String,
    pub revision: u64,
    pub attempt_id: String,
    pub status: Status,
    pub ready: bool,
}
impl From<&Task> for TaskReceipt {
    fn from(t: &Task) -> Self {
        Self {
            id: t.id.clone(),
            revision: t.revision,
            attempt_id: t.attempt_id.clone(),
            status: t.status,
            ready: t.ready,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub task: Option<TaskReceipt>,
    pub group: Option<Group>,
    pub entity_id: Option<String>,
    pub effect_ids: Vec<String>,
    pub replayed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: i64,
    pub group_id: String,
    pub actor: Actor,
    pub kind: String,
    pub data: serde_json::Value,
}

/// Host-only removal diagnostics, produced by the same checks as removal itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRemovalBlocker {
    pub task: Task,
    pub reasons: Vec<MemberRemovalReason>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemberRemovalReason {
    UnfinishedWork,
    UnsettledExecution,
    UnsettledReview,
}
