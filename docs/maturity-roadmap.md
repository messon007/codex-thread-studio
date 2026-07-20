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
- [x] `/` command palette with keyboard and pointer navigation.
- [x] App Server backed model, permissions, status, compact, review, diff, skills, and MCP actions.
- [x] Local copy/new plus existing rename/fork/archive/delete actions in the same command palette.
- [x] Select-and-comment review workflow with persisted drafts.
- [x] Compact in-session Turn navigator with reading-position tracking, prompt previews, and direct jump.

### Rich composer and tool interaction

- [ ] Local image input and image preview through structured `localImage` input.
- [ ] `tool/requestUserInput` questions and MCP elicitation forms.
- [ ] App and plugin discovery/invocation, including structured mention inputs.
- [ ] Subagent/thread tree navigation and active-agent switching.
- [ ] Shell-command composer prefix and background-terminal management.
- [ ] Prompt history search, queued follow-ups, and edit-previous-message fork.

### Review and large-session ergonomics

- [ ] Virtualized/paged historical Turns using App Server history pagination where supported.
- [ ] Syntax-highlighted diffs with per-file navigation and line comments.
- [ ] Search inside a Thread and jump between tool/file/result Items.
- [ ] Completed-work desktop notifications and waiting-for-input workspace.
- [ ] Reconnect reconciliation for missed events and pending approvals.

### Lifecycle and release maturity

- [ ] App Server process ownership independent from the Tauri window.
- [ ] Capability negotiation by installed Codex version and generated schema.
- [ ] Linux and macOS packages, signatures, upgrade guidance, and Windows/WSL policy.
- [ ] End-to-end fixtures for long streaming output, approvals, file search, commands, and reconnects.
- [ ] Accessibility audit for keyboard-only use, focus, screen readers, contrast, and reduced motion.

## Definition of done for a capability

A capability is complete only when:

1. the supporting App Server method/event and local-version boundary are documented;
2. loading, success, empty, denied, failed, and reconnect states are visible;
3. keyboard operation works without a mouse;
4. high-frequency events do not trigger full-history rendering;
5. protocol payloads are escaped or sanitized at the UI boundary;
6. focused unit tests and an end-to-end acceptance path exist; and
7. the feature is listed in both product requirements and user-facing documentation.
