# Agent Teams host integration

The feature lives on `feat/agent-teams`; it is not merged into `main`. The standalone domain crate remains independent of the browser, Tauri, App Server transport, and the reader SDK.

## Ownership and boundaries

- `crates/studio-teams`: transactional tasks, membership, explicit completion/review, idempotent commands, native-turn barriers, outbox, and persistent admission/reservation state.
- `src-tauri/src/teams.rs`: native-event correlation, backend registration, outbox delivery, control-plane fencing, MCP bridge, and authenticated user API.
- `ui-src/teams.mts`: a Teams view in the shared right workspace with focused operation dialogs and a small reservation client. Its dependencies are an authenticated gateway request and session navigation.
- The existing global Thread Router is not a team orchestrator. An optional orchestrator is selected from a group's existing members. Groups may be created empty and populated one member at a time (maximum five); fewer than two members cannot resume collaboration.

Teams never uses `turn/start`, `turn/steer`, or `thread/queue/start` for automatic delivery. It records the send intent before a single `thread/queue/add`. Busy members remain busy with their current work; Codex starts queued work when able. A request that may have reached Codex is not automatically retried. Stable message IDs correlate receipts; they are not assumed to deduplicate native queue admission.

Ordinary Studio messages keep their existing payloads, attachments and turn settings. Their dispatch acquires the same durable reservation checked by the Teams worker. Supervision holds a reservation for its entire job. A page closing or an uncertain send retains the reservation for explicit reconciliation. Team waiting is per task, not a session reservation; unrelated work can continue.

## Tool installation

Installing tools is an explicit UI operation. The preview identifies the exact Codex user configuration file and version. Its impact includes other Codex clients sharing that file, which can see tool names. The server rejects calls from nonmembers and requires a matching native tool-call event, current thread/turn and backend registration. Arguments cannot choose a caller identity.

The host writes only its own uniquely named MCP entry with compare-and-swap configuration versioning. Named Teams tools receive explicit tool-level approval in the installation; this does not relax shell, filesystem or approval policies for the member's work. The UI describes this scope before installation. It refuses to overwrite an externally changed registration. MCP reload refreshes the MCP configuration category.

The stdio command is this exact Studio executable with `--teams-mcp --runtime-file … --backend …`. That mode does not open a window or start a daemon. A private instance descriptor contains the loopback address and a separate backend credential, not the browser gateway credential. The descriptor is refreshed when Studio starts on a new port. Uninstall removes only the owned entry after pending receipts have been settled.

Codex 0.170.0 is the local compatibility baseline. Loaded sessions do not accept new MCP tools through `thread/resume.config`; the verified installation path is configuration CAS plus `config/mcpServer/reload`. Only same-host backends are enabled in this implementation. WSL/remote backend execution needs a separately verified bridge and is rejected rather than implicitly sharing credentials.

## Commands and completion

An existing member can call `team_context({})` without first knowing its group ID; the trusted host derives membership from its native session. Other tools use explicit group and task identities.

The MCP catalog has eleven tools: `team_context`, `team_task_get`, `team_task_history`, `team_task_list`, `team_command_get`, `team_delegate`, `team_task_update`, `team_ask`, `team_answer`, `team_submit_result`, and `team_review_result`.

Mutations carry a request ID; task mutations also carry revision and attempt IDs. A retry with the same request ID and payload receives its recorded outcome. A changed payload is an idempotency conflict. The UI also retains an uncertain mutation's request ID across reloads.

A member must explicitly claim work, checkpoint or wait, submit a result with evidence and criterion results, and obtain requester review. A successful native turn only releases a barrier; it does not complete a task. Failed/interrupted/unknown execution and review barriers require fresh native evidence or an explicit user decision recorded with evidence.

## Recovery and disable

Restart invalidates stale active observations and marks unacknowledged send intents uncertain. Durable tasks, outbox entries, queue IDs and native turn IDs remain available for reconciliation. Positive native receipts can settle a delivery; failure to find a receipt in a bounded search is not evidence that nothing executed.

Pause/disable first prevents new handoff. Unsent messages can be withdrawn locally; native queued messages require a positive deletion receipt. Running work is not interrupted. An unresolved native send remains visible. Resuming a scope must not replay already settled effects; a previously withdrawn notification is eligible only under a new, successfully activated generation.

Disabling Teams preserves its audit and tasks. Physically unloading the MCP registration is separate because running work may still have a result to report. The default is disabled. No public build depends on this checkout's validation tools, synthetic provider, private tokens, or another repository.

## Validation fixtures

`scripts/probes/agent-teams/studio-integration.mjs` runs the actual Studio executable and actual Codex App Server with an isolated empty Codex configuration and synthetic Responses provider. It is intended to run inside the documented Bubblewrap fixture, with all writable state under `/media/rui/codex-thread-studio-agent-teams/validation`. It does not validate real-model reasoning quality or authorize changes to real user sessions.

The authentication model is a single user's trusted Studio/App Server host. Native-event correlation prevents absent-event and wrong-session calls; it is not an unforgeable identity guarantee against another malicious client holding equivalent authenticated App Server host authority. This limitation, observed in the earlier echo probe, is retained explicitly. It does not grant browser pages or ordinary members the host control credential.

The entry opens the group list in Studio’s shared right workspace. Group sections
and task details stay in that rail; edit/create/review/delete actions use focused
dialogs. See [interaction design](interaction-design.md) for navigation and deletion
semantics. `delete_group` is a host-only revision-checked, idempotent operation; it
removes the group, memberships, tasks, effects and visible history in one domain
transaction. Native sessions, files, command receipts and native queue receipts
are not deleted. Live/unknown claims, unfinished tasks, pending deliveries or
held member reservations block deletion.

`AddMember` atomically joins one session and optionally assigns the single
orchestrator/owner slot. A second requested owner is rejected without adding the
member; intentional replacement uses `SetOrchestrator`.

`member_removal_preview` is a read-only host endpoint. It returns the current group,
blocking tasks/reasons from the domain's actual removal predicates, plus pending
native deliveries and reservations for that member's session. It is not an MCP
agent tool and does not authorize removal: `ChangeMembers` revalidates revision,
references and live queue constraints when the user confirms. A preview includes
terminal tasks whose execution or review receipts remain unsettled.
