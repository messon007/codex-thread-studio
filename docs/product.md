# Codex Thread Studio product specification

## Goal

Provide a clear desktop UI for Codex that uses App Server's structured Thread/Turn/Item protocol instead of terminal scraping. The UI must make progress, tool activity, approvals, final status, and failure reasons independently visible and selectable.

## Scope

- Codex only; no Claude Code or OpenCode adapter in this release.
- Direct `codex app-server` integration; no terminal, tmux, or xterm fallback.
- Local desktop operation on Linux first, with Tauri-compatible macOS support.
- Existing Codex authentication and configuration; no API-key storage in Studio.

## Functional requirements

1. Thread navigation: project-directory grouping, list, search, select, automatic resume, reload, create, rename, fork, archive, and delete. Native session-tree, fork, parent, source, and CLI metadata are visible when supplied by Codex.
2. Structured transcript: messages, reasoning summaries, plan, commands/output, file changes/diffs, supported tool calls, token usage, and explicit Turn completion status/error. Agent prose supports sanitized GitHub-flavored Markdown, including headings, lists, links, quotes, tables, task lists, inline code, and fenced code with copy controls.
3. Interaction: start a Turn, steer an active Turn, interrupt it from the composer or Thread toolbar, and handle command/file/permission approvals.
4. Comments: select rendered text, enter a comment in a dialog, repeat across items, persist by Thread, edit through delete/clear/additional guidance, and insert the assembled prompt without automatic send.
5. Appearance: light/dark theme, configurable UI/code font family and weight, code size, high-contrast secondary text, and persisted Comfortable/Wide/Full transcript width.
6. Diagnostics: show App Server state, resolved Codex binary, protocol/transport, reconnect state, and actionable spawn/protocol errors.

## Acceptance criteria

1. With a logged-in Codex CLI that supports `app-server`, Studio completes initialization and `thread/list` without a terminal process.
2. Selecting a persisted Thread calls `thread/resume` and renders its available Turns and Items.
3. A new Thread can be created for an absolute project directory with chosen approval and sandbox policy.
4. A Thread can be renamed through `thread/name/set`; name-update notifications refresh both the sidebar and header.
5. Sending a message produces a user item, streaming agent/tool items, and a final completed/interrupted/failed state.
6. While a regular Turn is active, additional text uses `turn/steer`; both Stop controls use `turn/interrupt` with the active IDs.
7. Command and file-change approval requests can be accepted once, accepted for the session, or declined; permission requests return an allowed subset or empty denial.
8. At least three disjoint transcript selections can be commented; the draft survives restart and inserts one assembled prompt without sending.
9. Each structured annotation records `threadId` through its preference key and retains `turnId`/`itemId` when selection originates in an Item.
10. Agent Markdown renders without a network connection. Raw HTML cannot inject script, style, frames, embedded objects, forms, event handlers, or unsafe URLs; non-Markdown payloads remain escaped text.
11. Theme, typography, and content width apply immediately and survive restart. Comfortable is the default at 960px, Wide is 1280px, and Full uses the available workspace width; all modes shrink on smaller windows.
12. A missing Codex executable or failed App Server initialization is shown in the workspace and backend details.
13. Selecting text inside rendered headings, lists, tables, quotes, and code retains the outer Turn/Item annotation anchor.
14. Markdown parser, sanitizer, stylesheet, and licenses are locally vendored from exact lockfile versions; no CDN request is required.
15. `cargo test --workspace --locked`, `npm test`, and `npm audit --offline --audit-level=moderate` pass.

## Deferred

- Custom user-defined groups and archived-Thread browsing/unarchive management. The current sidebar groups by `cwd`; `sessionId` represents Codex's native Thread tree rather than a user-defined group.
- A literal Thread “restart.” Native Threads have no dedicated tmux or worker process; selecting resumes them and Reload re-reads persisted state.
- App Server daemon/control-socket ownership so active Turns survive window close.
- Full `requestUserInput`, MCP elicitation forms, authentication/login, skills/apps pickers, images, math/diagram rendering, and diff syntax highlighting.
- Notifications for completed background work while Studio is closed.
- Packaging/signing and native Windows process management.
