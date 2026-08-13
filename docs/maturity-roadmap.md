# Mature Codex client roadmap

## Product target

Codex Thread Studio aims to be a mature desktop Codex client, not a protocol demo. A mature client must preserve the advantages of a structured UI while covering the daily workflows users expect from Codex CLI.

The App Server is the source of truth. Studio should call an App Server RPC, consume its event, or use its generated schema whenever the capability exists. Local implementations are reserved for presentation-only behavior such as clipboard access, dialogs, keyboard navigation, and rendering.

## Design rules

1. Prefer App Server discovery APIs over hard-coded catalogs: models, skills, MCP servers, apps, plugins, permissions, and experimental features.
2. Keep Thread, Turn, Item, approval, usage, and error state structured; never infer them from terminal text.
3. Treat the CLI command menu as a UX reference, then map each command to an App Server RPC or an explicit local action.
4. Generate protocol schemas from the installed Codex during compatibility work. Online documentation can be newer than the local binary.
5. Stream cheaply: accumulate every protocol delta, paint at animation-frame cadence, finalize Markdown only when an Item completes, and avoid rebuilding completed history.
6. Unsupported capability requests must remain visible and actionable rather than disappearing from the transcript.

## Capability matrix

### Daily-use foundation

- [x] Structured Thread list, resume, create, rename, fork, archive, delete, and unsubscribe.
- [x] Structured messages, plans, reasoning, commands, files, tools, approvals, usage, and final status.
- [x] Batched incremental rendering for agent, plan, reasoning, and command-output deltas.
- [x] `@` workspace file search through `fuzzyFileSearch`.
- [x] `$` skill discovery through `skills/list` with structured skill UserInput.
- [x] `!` local shell-command composer prefix through `thread/shellCommand`.
- [x] `/` command palette with keyboard and pointer navigation.
- [x] App Server backed model, permissions, status, compact, review, diff, skills, and MCP actions.
- [x] Local copy/new plus existing rename/fork/archive/delete actions in the same command palette.
- [x] Select-and-comment review workflow with persisted drafts.
- [x] Compact in-session Turn navigator with reading-position tracking, prompt previews, and direct jump.

### Rich composer and tool interaction

- [ ] Local image input and image preview through structured `localImage` input.
- [x] `item/tool/requestUserInput` questions and MCP elicitation forms, using the installed App Server schema contract.
- [ ] App and plugin discovery/invocation, including structured mention inputs.
- [ ] Subagent/thread tree navigation and active-agent switching.
- [ ] Background-terminal list, interaction, and termination management.
- [ ] Prompt history search, queued follow-ups, and edit-previous-message fork.

### Review and large-session ergonomics

- [x] Experimental Session Map foundation: optional per-Thread Map, compact goal row, current-location breadcrumb, generic Items/Relations, abstract Structure selection, revision conflicts, and undo.
- [x] Experimental Codex Map automation: bounded hidden context, in-Turn dynamic-tool support where available, a filtered response-tail fallback using the same Turn, and server-side safe-operation validation.
- [ ] OpenCode Map automation adapter; experimental OpenCode Maps are currently edited manually.
- [ ] Declarative built-in/user templates with version pinning, validation, import/export, and migration preview. See the [feature package](session-map/README.md).
- [ ] Virtualized/paged historical Turns using App Server history pagination where supported.
- [x] Git Review rail with per-file navigation, staged/unstaged scopes, bounded line-numbered text diffs, and safe stage/unstage actions.
- [ ] Language-aware token highlighting, side-by-side mode, hunk staging, and line comments on Git diffs.
- [ ] Search inside a Thread and jump between tool/file/result Items.
- [x] Completed-work/waiting-for-input desktop notifications and inline waiting-input cards. A consolidated Activity Inbox remains future work.
- [x] Reconnect reconciliation for missed events: lag detection and every re-established Codex connection reload the selected Thread.

### Lifecycle and release maturity

- [ ] App Server process ownership independent from the Tauri window.
- [x] Initialize capability declaration, visible initialization metadata, unsupported-request fallback, and a generated-schema compatibility fixture against the installed Codex. Per-method server advertisement remains unavailable in the protocol.
- [ ] Linux and macOS packages, signatures, upgrade guidance, and Windows/WSL policy.
- [x] Protocol lifecycle/reconnect fixtures plus gateway route and filesystem/PTY integration tests. Full GUI automation remains future work.

## Definition of done for a capability

A capability is complete only when:

1. the supporting App Server method/event and local-version boundary are documented;
2. loading, success, empty, denied, failed, and reconnect states are visible;
3. keyboard operation works without a mouse;
4. high-frequency events do not trigger full-history rendering;
5. protocol payloads are escaped or sanitized at the UI boundary;
6. focused unit tests and an end-to-end acceptance path exist; and
7. the feature is listed in both product requirements and user-facing documentation.
