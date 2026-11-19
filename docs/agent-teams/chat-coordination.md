# Group collaboration through ordinary chat

The group panel manages membership and configuration. Requests are entered in the
existing orchestrator session's Composer; there is no Tasks entry or manual
assignment form. Activity still exposes task evidence, questions, reviews and
recovery. Completed history is collapsed. No scheduler-owned chat is created.

## Configuration and authority

Create a named group, add up to five existing sessions, then open Edit group.
Name, optional Orchestrator (none by default), and Enable collaboration are saved
by one revision-checked `ConfigureGroup` command. At least two members and global
Teams enablement are required to enable a group. No orchestrator is required for
peer collaboration. Adding a member never assigns a role or enables collaboration.

The backend derives effective authority at actual dispatch from the current group,
owner, paused/archive state and global enablement. Configuration never starts,
steers or interrupts a turn. The next ordinary Composer/queue message of the active
owner gets a separate host-labelled text prefix with group/member IDs, names,
responsibilities and tool-use guidance. Original text elements, images, attachments,
model and turn settings remain intact. Group JSON is explicitly labelled as data.

A previously announced owner gets an exit-role prefix on its next ordinary message
after disable, handover, removal or deletion, including after restart or leaving
all groups. If a replacement role applies, its prefix replaces the earlier role.
Changes before a message coalesce to current effective authority; toggling off and
on without a message in between does not require two notices. Cosmetic/name/roster
changes do not repeat the enable notice; agents fetch current data with team_context.

## Dispatch, receipts and boundaries

Only the actual ordinary Composer and ordinary queue path mark the internal origin.
The adapter strips this marker before native RPC and forwards it to the host dispatch
gate. Auxiliary/supervision/router/automatic Teams work does not consume a role update.
Active-turn Steer is not a new ordinary turn and does not consume it either. Direct
external CLI/App Server sends bypass this Studio-specific prefix mechanism.

The existing stable clientUserMessageId and original payload hash remain the replay
identity. In the same SQLite transaction that records the send intent, the host
stores only an immutable text prefix and intended role; it does not duplicate images
or full user payloads. The native receipt and confirmed role update commit together.
Uncertain sends are never automatically resent or considered delivered. A repeated
accepted ID returns its original receipt despite later group edits. Repeated old
receipts cannot rewind the role.

Any unresolved ordinary dispatch blocks another ID for that session even if a queue
reservation is manually released. Group deletion and member removal also check these
unresolved submissions. Reconcile through positive native evidence in Activity (or Collaboration settings
when the session no longer belongs to any group);
absence of a receipt is not proof that nothing executed. This ordering prevents a
late first receipt from an older role overwriting a newer confirmed role.

The prompt requests explicit delegation, evidence and acceptance. Native turn
completion alone still does not finish a collaboration task. Disabling immediately
fences new handoffs independently of when the user next sends a message; it does not
interrupt running work or prevent permitted results from being reported. Existing
MCP tool installation, caller authorization and durable task/queue semantics remain
required. This mechanism is advisory model context, not an authorization boundary.

Validation uses the actual Studio executable, Codex App Server and Chromium with an
isolated synthetic provider; it validates protocol and UI, not autonomous planning
quality of a real model. The feature remains on feat/agent-teams, unmerged into main.
