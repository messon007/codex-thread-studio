# Turn-scoped supervision

## Implemented first version

Open Composer **＋ → Supervise this turn** while an ordinary task is running.
The wand applies to that task, not the next send or the entire session. There is
no pre-send checkbox or intent. The running identity is checked again after
acquiring ownership, so clicking a stale button cannot start supervision on an
already completed or different task.
Codex/EPT Codex and their Router target responses are supported; OpenCode is not
enabled yet. Composer supervision controls and state stay inside the compact
**＋** menu; there is no separate Composer status chip. Router task wands retain
their per-task active state.

Router targets expose a wand beside the running response, with separate task
identities and Stop controls. Reply/Files/Terminal/Review append after all native
response actions, not specifically after Favorite. The wand uses the native
18px, 1.4px stroke icon style and 32px hit target, without animation.
The paperclip directly uploads images; project files still use the @ picker.
When supervision starts a continuation, the Router response follows that new
native Turn, with the link persisted in Router history. A late completion from
the preceding Turn cannot close the new continuation.

Jobs belong to the open Studio page: session/backend switches preserve them,
but refresh/closure stops them. Nothing is persisted or automatically replayed.
Web Locks prevent duplicate owners in windows of the same origin; unsupported
browsers cannot enable supervision. This is not a cross-server lease: do not
supervise the same session from independently hosted Studio instances.

The hidden ephemeral evaluator follows the source session's model/backend, with
read-only sandbox and approval policy `never`. It is instructed not to use tools;
this is not a hard tool-disable mechanism. It receives the original task, Steer
additions, previous continuation prompts/responses and latest assistant response.
Only `continue` sends a labeled `[Studio turn supervisor]` message. At most five
automatic continuations; duplicate prompts and oversized context stop for review.
An uncertain send is never retried. Pending approvals, errors, manual Stop and
unrelated user sends stop supervision. Queue waits; unsuccessful supervision
pauses Queue for manual review, successful completion releases it.

Tests: `ui/turn-supervision.test.mjs` exercises terminal/idle gating, duplicate
ticks, task identity, cancellation and Steer races, user decisions, approval
races, send uncertainty, malformed output and no replay after reload.

The server-owned SQLite design below remains a future design, not implemented.

## Scope and identity

Opt in for one user task, not the whole session. A logical task has one root turn
and can span multiple native `turn/start` IDs when the supervisor continues it.
Steer/additional input for that task updates the same task's instructions and
invalidates any in-flight evaluation. Independent queued requests must not be
silently merged into the task. Supervision ends when the task finishes or the user
stops it; the next ordinary request defaults to supervision off.

## Trigger and decision

After a matching native turn completes and the session is idle, send the original
user request, its additions, the latest final assistant response, stop/error
metadata, and a compact task-progress ledger to a hidden reusable evaluator.
Retaining the ledger avoids repeatedly requesting tasks completed in earlier
continuations. The evaluator cannot use tools, modify files or grant approvals.

Structured outcomes:

| Outcome | Action |
| --- | --- |
| complete | Record a short completion assessment; stop supervision. |
| continue | Send focused instructions for unfinished work within original scope. |
| needs_user | Pause with the unresolved question; never invent the user's answer. |
| error | Stop and expose the cause; no automatic retry by default. |

Routine “shall I continue?” pauses may be continued. Permission requests, external
side effects not originally authorized, ambiguous requirements, credentials,
manual Stop and backend errors must not be treated as routine pauses. Automatic
messages must be visibly labeled as supervisor-generated, not new user approval.

## Race and lifetime handling

The server should own the job so switching sessions or closing a browser tab does
not duplicate or lose it while Studio is running. Persist root/session identity,
native-turn chain, revision, evaluation lease, decision, send receipt and counters
in `studio.sqlite3`. Immediately before sending, recheck the job revision, enabled
state, terminal turn ID, session idle state and absence of pending approvals/input.
Duplicate completion notifications and repeated evaluator output must not resend.
On uncertain delivery after a restart, pause for review instead of replaying.

Do not run evaluation from an `idle` notification alone: it can arrive before the
final response/turn completion. Do not use an inactivity timer as evidence that
the agent stopped. Human Stop cancels evaluation and any pending continuation.

## Control policy

- Opt in during execution via Composer **＋** or the Router task's wand.
- Per-task status and a Stop supervision action; automatic messages remain visible.
- Initial budget proposal: at most five automatic continuations, plus repetition/
  no-progress detection. Exhaustion pauses rather than claims completion.
- Evaluator model initially follows the session's model, with an explicit future
  override if desired; no silent cross-backend fallback.
- Independent Queue items wait until this task ends; Steer and explicitly added
  task input remain part of the root task.

An LLM's completion assessment is evidence, not a guarantee. Tests must include
premature pauses, real completion, errors, human decisions/approvals, manual Stop,
Steer during evaluation, duplicate events, reconnects and uncertain send receipts.
