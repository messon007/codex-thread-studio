# Codex Thread Studio product specification

## Goal

Provide a clear desktop UI for Codex and OpenCode using their structured protocols instead of terminal scraping. The UI must make progress, tool activity, approvals, final status, and failure reasons independently visible and selectable.

The long-term target is a mature Codex desktop client: daily CLI workflows should be available through native structured controls while Studio retains richer review, selection, and commenting interactions. See the [maturity roadmap](maturity-roadmap.md).

## Scope

- Codex App Server and OpenCode Server; no terminal scraping, tmux, or xterm transport fallback. A user-controlled xterm/PTTY project tool is available independently of Agent events.
- Local desktop operation on Linux first, with Tauri-compatible macOS support.
- Existing CLI authentication and configuration; no API-key storage in Studio.

## Functional requirements

1. Thread navigation: project-directory grouping, list, search, select, automatic resume, reload, create, rename, fork, archive, and delete. A completed response can fork from its Turn into a new same-project Thread without changing the source Thread. Native session-tree, fork, parent, source, and CLI metadata are visible when supplied by Codex.
2. Structured transcript: messages, reasoning summaries, plan, commands/output, file changes/diffs, supported tool calls, token usage, and explicit Turn completion status/error. Agent prose supports sanitized GitHub-flavored Markdown, including headings, lists, links, quotes, tables, task lists, inline code, and fenced code with copy controls.
3. Interaction: start a Turn, steer an active Turn, interrupt it from the composer or Thread toolbar, and handle command/file/permission approvals. Typing `@` searches the active project through App Server and inserts a selected path; typing `$` discovers App Server skills and queues the selected structured skill input; starting the composer with `!` runs the remainder through the Thread's local shell. Typing `/` opens a keyboard-navigable command palette. The composer accepts up to four PNG/JPEG/WebP/GIF images through a picker, clipboard paste, or drag-and-drop, shows removable previews, and preserves them in Codex, OpenCode, and Router-dispatched Turns. A compact right-side navigator maps each user/Codex interaction to its structured Turn and supports direct navigation within long sessions.
4. Comments: select rendered text, enter a comment in a dialog, repeat across items, persist by Thread, edit through delete/clear/additional guidance, and insert the assembled prompt without automatic send.
5. Favorites: save a complete AI Item with one click, automatically resolve the user question from the same Turn, optionally include it, add a title/tags/note, search globally across backends and Threads, and navigate back to the original Item.
6. Appearance: Simplified Chinese/English/system-language selection, light/dark theme, configurable UI/code font family and weight, code size, high-contrast secondary text, and persisted Comfortable/Wide/Full transcript width. Thread, user, and model content is never translated.
7. Diagnostics: show App Server state, resolved Codex binary, protocol/transport, reconnect state, and actionable spawn/protocol errors.
8. Performance: high-frequency text, reasoning, plan, and command-output deltas update only their active Item at animation-frame cadence. Completed history is not reparsed for every delta.
9. Backend switching: selecting Codex or OpenCode replaces the session list and transport without mixing IDs, drafts, selections, or transient event state.
10. Session Map (experimental): after the user explicitly creates one, a compact goal row, current location, remaining scope, and progress remain visible during long conversations. Threads without a Map keep the original layout. Navigation actions never send a visible message automatically; safe Codex synchronization is revisioned and undoable. The complete target boundary is defined in the [Session Map specification](session-map/specification.md).
11. Structured interaction: show `item/tool/requestUserInput` and MCP form/url elicitation requests inline, preserve secret input masking, and return typed response payloads without converting them to chat text.
12. Artifact workspace: render PDF locally with search and text/region comments; render CSV/XLSX as bounded grids with local charting. Documents never load parsing code from a CDN.
13. Project environments: ordinary variables and cache variables are visible; Secret values are stored separately with restricted permissions and never returned by read APIs. Profiles apply to Studio PTYs and Codex shell commands, while network access maps to the next Turn sandbox policy.
14. Git Review: inspect the selected session repository without shell interpolation; filter all, staged, and unstaged code/Markdown/CSV/text changes; read bounded line-numbered unified diffs; and stage or unstage a selected file. Destructive discard is not exposed.
15. Session Resources: deterministically extract HTTP(S) URLs, workspace paths, and code locations from user/Agent narrative Items in only the latest Turn; exclude historical Turns, file changes, fenced code, Mermaid, command/tool payloads, markup, and slash-separated terminology; merge identical targets without losing Item sources; search and filter them in the shared right workspace; and open them through Embedded Browser, Document Viewer, or Files. Resources are intentionally ephemeral; Favorites provide explicit long-term retention. See the [Session Resources specification](session-resources/specification.md).
16. Selection translation: translate selected transcript or document text into Simplified Chinese through a hidden task on the current session backend. Translation never falls back to another backend, never adds messages to the selected session, preserves structured text, and removes temporary OpenCode sessions after completion.

## Acceptance criteria

1. With a logged-in Codex CLI that supports `app-server`, Studio completes initialization and `thread/list` without a terminal process.
2. Selecting a persisted Thread lazily calls `thread/resume` the first time and caches its available Turns and Items by backend and Thread ID.
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
16. Typing `@query` calls App Server `fuzzyFileSearch` with the selected Thread `cwd`; stale responses cannot replace results for a newer query.
17. Typing `/` lists supported commands and supports Up/Down, Enter/Tab, Escape, and pointer selection without sending the command text to the model.
18. Model, compact, review, skills, and MCP menus obtain their data or action from `model/list`, `thread/compact/start`, `review/start`, `skills/list`, and `mcpServerStatus/list` respectively.
19. Selecting a model, reasoning effort, or permission profile supplies valid override fields on the next `turn/start`; selecting a skill adds both `$skill-name` text and a structured `skill` input Item.
20. Streaming `agentMessage`, `plan`, reasoning, and command output does not replace the transcript container or rerun Markdown parsing until Item completion.
21. Typing `$query` filters enabled skills returned by `skills/list`; choosing a skill inserts `$skill-name`, supplies its `name` and `path` as a structured `skill` UserInput, and never relies only on visible text.
22. Submitting `!command` while the Thread is idle calls `thread/shellCommand` with the exact selected Thread ID, visibly distinguishes local shell mode, and explains that this user-invoked command does not inherit the model Turn sandbox. Empty commands and commands submitted during an active Turn are not run.
23. When the transcript is following live output, streaming deltas, Item completion, Markdown reflow, and structural rerenders keep it pinned to the latest content. A deliberate upward scroll pauses following, and returning to the bottom or sending a new interaction resumes it.
24. Switching back to an unchanged, previously loaded Thread renders its in-memory model immediately; a newer catalog timestamp or explicit Reload re-reads it from the backend.
25. The sidebar groups All and Active sessions by the last component of their project directory. Attention is the flat in-memory workset of sessions whose history has been loaded during the current Studio process, ordered by each session's latest backend update time.
26. A fresh Studio process starts with no retained Thread history cache, automatically loads the most recently updated session for the selected backend, and loads other sessions only when the user selects them.
25. With two or more Turns, the navigator creates one marker per Turn, highlights the Turn at the reading position, exposes a normalized user-prompt preview on hover/focus, and scrolls to the selected Turn. It stays hidden for a single Turn and narrow windows.
26. OpenCode mode starts a password-protected loopback server, lists sessions across project directories, restores message history, consumes SSE deltas, supports create/rename/fork/delete/abort, and never exposes the child password to JavaScript.
27. OpenCode project-scoped requests include the session directory; missing status entries are displayed as idle, and an SSE reconnect re-reads the selected session.
28. Comment drafts and last-selected IDs are namespaced per backend. Switching back restores the previous backend selection without showing the other backend's sessions.
29. Saving an agent message stores its exact backend/Thread/Turn/Item anchor and original Markdown; the same Turn's user question is included only when selected.
30. Favorites survive restart in SQLite, search across title/content/question/note/tags/source metadata, render sanitized Markdown, can export the complete global library as Markdown, and can reopen and highlight an available source Item without losing the saved copy when the source is unavailable.
31. `item/tool/requestUserInput` and MCP form/url elicitations remain visible until answered, return protocol-shaped results, and show unsupported request types as explicit errors.
32. A re-established Codex WebSocket reloads the selected Thread before treating its cached state as current; lagged event broadcasts trigger the same reconciliation.
33. PDF/XLSX requests are root-confined, size/signature checked, parsed locally, and never execute document scripts or macros. XLSX expanded size and entry counts are bounded.
34. Reading an environment profile never returns Secret values. Saving it applies values to the selected Codex Thread, and a newly started Studio PTY receives the same project environment.
35. Git status handles spaces, untracked files, staged/unstaged overlap, renames, and conflicts through NUL-delimited porcelain output. Diff and mutation requests accept only current root-relative changed paths, remain bounded, and never pass a command through a shell.
36. Session Resources scans completed narrative Items in only the latest Turn rather than streaming deltas, ignores file-change activity, never fetches a URL while detecting it, keeps paths confined to the selected session root, and restores the resource panel after a document opened from it is closed.
37. Image attachments are validated by byte signature, limited to four images, 10 MiB each, and 20 MiB total, sent as structured protocol input rather than prompt markup, and displayed in both pending composer state and user-message history. Authenticated Codex/OpenCode transport bodies are capped at 32 MiB to include base64 and JSON overhead. Failed sends restore the original text and attachments.

## Deferred

- Custom user-defined groups and archived-Thread browsing/unarchive management. The current sidebar groups by `cwd`; `sessionId` represents Codex's native Thread tree rather than a user-defined group.
- A literal Thread “restart.” Native Threads have no dedicated tmux or worker process; selecting resumes them and Reload re-reads persisted state.
- App Server daemon/control-socket ownership so active Turns survive window close.
- Authentication/login, app/plugin pickers, math rendering, and diff syntax highlighting.
- Notifications for completed background work while Studio is closed.
- Package signing and native Windows process management.
- Cross-Thread Session Map knowledge graphs and executable third-party Map renderers. The proposed per-Thread, declarative design is documented in [Session Map templates](session-map/template-system.md).
