# Structured multi-backend architecture

## Process model

The first release uses three logical layers and normally two application processes plus system WebView helper processes:

```text
Tauri process
├─ OS WebView: shared HTML/CSS/JavaScript UI
└─ Rust/Axum loopback gateway
   ├─ WebSocket ↔ Codex JSONL broker ↔ codex app-server
   └─ HTTP/SSE proxy ↔ password-protected opencode serve
```

There is no terminal emulator, tmux process, direct model API integration, or separate Studio session database. Each backend remains the source of truth for its own sessions and credentials.

On Windows 11, only the client and loopback gateway are native Windows processes. The backend boundary is `wsl.exe`: Codex JSONL is relayed over its stdio and OpenCode HTTP/SSE is reached through WSL2 localhost forwarding. Windows Codex/OpenCode executables are intentionally out of scope. Linux and macOS launch their local backend executables directly.

The experimental Session Map feature adds Studio-owned structured navigation state without changing that history boundary. Its revisioned Items, Relations, and change history are local presentation/workflow data; Codex and OpenCode remain authoritative for Threads, Turns, and Items. A Map is created only after an explicit user action. Missing Maps are represented by the absence of a database row and do not produce a rail or placeholder for existing Threads. See the [Session Map specification](session-map/specification.md).

The optional Thread Router is a backend-neutral orchestration layer implemented in the WebView. A registry maps stable `backend:id` session references to native `read` and `startTurn` adapters; routing policy sees only the merged session catalog and concise responsibility metadata. The durable controller can use Codex or OpenCode independently of the selected target's backend. A dispatch becomes a normal user message in the target history, while Router tracks only terminal state and an opening link. See the [Thread Router specification](thread-router/specification.md).

## Transport and handshake

App Server uses newline-delimited JSON over stdio. The Rust broker:

1. resolves Codex from `CODEX_THREAD_STUDIO_CODEX_BIN`, NVM/FNM/Cargo/common user paths, then `PATH` on Linux/macOS; Windows starts the configured WSL command through `wsl.exe`;
2. starts `codex app-server --stdio` on the first WebSocket connection;
3. sends one `initialize` request with Studio client metadata;
4. sends `initialized` only after the initialization response;
5. forwards subsequent protocol messages without adding `jsonrpc`;
6. broadcasts stdout messages to the WebView and exposes stderr as diagnostic events; and
7. rejects browser attempts to repeat the handshake or use Studio's reserved request ID.

The App Server experimental WebSocket listener is deliberately not used. Studio exposes its own loopback-only WebSocket so the WebView never owns a local subprocess or filesystem capability.

## OpenCode transport

The Rust gateway resolves `opencode`, starts `opencode serve` lazily on a random loopback port, and supplies a fresh `OPENCODE_SERVER_PASSWORD`. On Windows the command runs inside the selected WSL2 distribution and the gateway uses Windows-to-WSL localhost forwarding. Only Rust knows the password. The WebView calls same-origin `/opencode/*`; Rust adds Basic authentication and streams ordinary HTTP or SSE responses.

The adapter uses `/experimental/session` for the cross-project list, `/session/*` for CRUD/history/prompt/abort, and `/global/event` for live events. Every project-scoped request carries the session `directory`. SSE reconnects trigger history reconciliation because OpenCode does not expose an SSE replay cursor.

Studio declares experimental structured-interaction and MCP form capabilities during the Codex `initialize` handshake, retains the returned platform metadata, and exposes its effective client capability set in connection state. On every Codex WebSocket reconnect it reloads the selected Thread; a broadcast lag signal performs the same reconciliation immediately. The compatibility fixture generates JSON Schema from the installed Codex and asserts the interaction methods Studio implements.

Artifact binaries and project environment profiles stay behind authenticated gateway routes. PDF/XLSX bytes are bounded and signature-checked before entering local parsers. Environment GET responses redact Secret values; the Rust layer injects them into PTYs or applies them to Codex through `thread/resume.config.shell_environment_policy` without sending stored values back to the WebView.

Git Review also stays behind the authenticated loopback gateway. Rust invokes `git` with explicit argument arrays and literal pathspecs in the selected session root, parses NUL-delimited porcelain status, caps status/diff output, and validates mutations against the current changed-path set. The WebView receives structured status plus bounded unified text; it can stage or unstage but has no discard endpoint.

## UI state model

```text
Thread
└─ Turn
   ├─ userMessage
   ├─ agentMessage (delta streaming)
   ├─ reasoning
   ├─ plan update
   ├─ commandExecution + output
   ├─ fileChange + diff
   ├─ tool/search/collaboration items
   └─ completion {completed | interrupted | failed, error}
```

The browser lazily builds one in-memory render model per `backend:thread-id` from Codex `thread/resume` or OpenCode session/message history, then keeps that model current with live notifications. Returning to an unchanged Thread renders this cache immediately instead of reading its complete history again. Catalog timestamps invalidate stale models, and Reload explicitly re-reads the selected Thread. Every selectable rendered block carries `data-turn-id` and `data-item-id`.

Agent prose and reasoning summaries pass through locally vendored Marked (GFM parsing), then DOMPurify (HTML sanitization), then GitHub Markdown CSS plus Studio theme overrides. Code-copy and table wrappers are added only after sanitization. A validated `markdown.mode` preference changes only presentation density; fenced plain-text blocks deliberately omit code chrome. Mermaid appearance uses a separate validated whitelist while its security controls remain fixed. See [rendering configuration](rendering-configuration.md). The outer structured Item element remains the selection/comment anchor.

## Streaming render path

App Server delta notifications update only the affected Item in the browser model. Studio batches high-frequency updates with `requestAnimationFrame` and patches the active message, plan, reasoning, or command-output node directly. Markdown parsing and sanitization run after Item completion, not once for every token delta. Structural notifications still trigger a full transcript render.

Codex Threads loaded during the current Studio process remain subscribed so their cached render models can receive routed notifications. The loaded set and render-model cache are deliberately process-local: after restart, Studio loads the most recently updated session for the selected backend and restores other histories only on first selection. The flat Attention view exposes this loaded workset and orders it by backend session update time. OpenCode uses the global SSE stream and routes events by session ID. Backend changes close the old browser transport, reject its pending UI requests, reset transient render state, and restore a cached model only when it was loaded in the current process and remains valid.

Before a Router dispatch, Studio lazily reads the selected target Thread into that same cache. It then sends `turn/start` without changing the active Thread. Notifications are associated with the target model by `threadId`/`turnId`, so the target row changes state immediately and its transcript is ready when opened.

Session Resources builds a disposable per-Thread index from only the latest Turn in the complete render model. A priority-ordered Extractor Registry handles Markdown targets, inline code paths, generic URLs, and plain workspace paths. Text extraction is deliberately conservative: it scans only user/Agent narrative content, excludes fenced code and Mermaid, and requires strong file syntax so slash-separated terminology, markup, file changes, command output, and tool payloads do not become false resources. Canonical Resources are stored separately from their Turn/Item Occurrences, so duplicate targets within that Turn merge without losing source navigation. Detection is local and runs only after completed Items or hydration; it never fetches a URL. A new Turn replaces the old index; Favorites are the explicit persistence boundary. The action layer sends HTTP(S) targets through the existing Browser policy, previewable root-confined files through Document Viewer, and directories or unsupported files through Files reveal. The complete contract is defined in the [Session Resources specification](session-resources/specification.md).

## Composer orchestration

The composer is a structured App Server client rather than a terminal command parser:

- Local images are signature-checked and encoded as bounded data URLs so the same input works with native Codex, Windows/WSL Codex, and OpenCode without exposing a general filesystem upload endpoint. Codex receives structured `image` UserInput; OpenCode receives its native `{type: "file", mime, filename, url}` part. Router turns receive the same image inputs and forward them unchanged to the selected target session.

- `@query` invokes `fuzzyFileSearch` with the active Thread working directory and inserts the chosen relative path.
- `$query` filters skills returned by `skills/list`; choosing one inserts `$skill-name` and queues a structured `skill` UserInput containing its App Server path.
- A composer value beginning with `!` calls `thread/shellCommand`. App Server evaluates the remainder with the Thread shell and publishes the resulting structured Items. Per the protocol, this user-invoked command runs with local user access and does not inherit the model Turn sandbox.
- `/model`, `/skills`, and `/mcp` populate controls through `model/list`, `skills/list`, and `mcpServerStatus/list`.
- `/compact` and `/review` invoke `thread/compact/start` and `review/start`.
- `/permissions` stores a valid approval/sandbox override for the next `turn/start`.
- Local Thread operations such as rename, fork, archive, and delete reuse the same structured RPCs as their toolbar actions.

Slash commands are client-side affordances over App Server capabilities; they are never forwarded as ordinary model text. Skills selected from either `$` completion or `/skills` are sent as structured `skill` input Items in addition to the visible `$skill-name` reference.

## Comment model

A comment draft is keyed by backend and session:

```text
{ backend:threadId, turnId?, itemId?, quote, comment, createdAt }
```

The quote remains human-readable even when old history changes. Turn/Item IDs preserve the structured source anchor. The configurable template assembles all comments and optional overall guidance into ordinary text for `turn/start` or `turn/steer`; insertion never sends automatically.

## Native favorites

Favorites attach to complete structured AI messages rather than terminal text or clipboard snapshots. The UI resolves the user question from the same Turn and offers it as an explicit inclusion choice. Each saved record keeps `{ backend, threadId, turnId, itemId }`, source metadata, the original Markdown answer, the optional question, tags, and a note. The source anchor enables a favorite to reopen its backend and Thread and highlight the original Item; the copied content remains useful even when the source Thread has later been archived or deleted.

## Persistence

Codex and OpenCode own history and credentials in their normal state directories. Studio stores UI language, appearance settings, per-backend selection, and language-specific comment templates in:

```text
~/.config/codex-thread-studio/settings.json
```

Larger user-authored state is stored in one shared SQLite database:

```text
~/.config/codex-thread-studio/favorites.sqlite3
```

Separate tables hold Favorites, per-session comment drafts and additional guidance, and per-session opening-question/responsibility metadata. All records are validated and bounded. The first database initialization transactionally imports the legacy `favorites.json` once and leaves it intact as a migration source. A separate one-time transaction moves legacy dynamic session state out of `settings.json`; the JSON fields are removed only after the database commit succeeds. Favorite search returns summaries, and full Markdown content is loaded only when a favorite is opened. The global library can be exported as a single UTF-8 Markdown document.

Session Maps use the rollout-isolated `session-maps.sqlite3` database beside Studio settings. The Rust gateway owns schema creation, validation, optimistic revision checks, transactional operation batches, and undo snapshots. Provider history is not copied into these tables. Declarative custom template snapshots remain a later milestone.

Thread Router configuration is small structured metadata stored in `settings.json`: Studio-managed controller identities and up to three fallback session keys with conditions. Per-session opening-question/responsibility metadata lives in `favorites.sqlite3`. Every unlisted session is a regular target. Routing requests and decisions remain in the Router Thread's native history; Studio does not copy chat history into its preferences.

On first launch, the app imports compatible settings from the former experimental path at `~/.config/agent-deck-studio/codex-native-settings.json` when the new file does not yet exist. Writes use a temporary file plus rename. Payload shape and size are validated in Rust.

The WebView uses a local Chinese-to-English interface catalog for both initial markup and controls rendered after App Server events. Thread titles, project paths, prompts, AI responses, favorites, comments, code, and tool output are protected from translation. Legacy single-template settings are detected and moved to the matching language slot.

## Security boundaries

- The gateway binds to a random `127.0.0.1` port.
- No provider credential or OpenCode server password crosses into browser storage or Studio preferences.
- App Server and Codex configuration decide sandbox and approval behavior.
- User, command, diff, tool, and unknown payloads are escaped as text. Agent Markdown is sanitized with DOMPurify; scripts, styles, frames, embedded objects, forms, buttons, and inline style attributes are forbidden. Fenced Mermaid source is rendered separately with Mermaid `securityLevel: strict`, bounded input, automatic execution disabled, SVG text labels instead of `foreignObject` HTML, and a second SVG/HTML sanitization pass; invalid diagrams fall back to their source.
- App Server request messages and proxied OpenCode bodies are bounded to 4 MiB; preferences are bounded to 1 MiB.

## Lifecycle limitation

Backend children belong to the Tauri process in this milestone. On Windows, each WSL launch records a private Linux process group and Studio explicitly terminates that group during managed shutdown or startup failure. History is durable, but active execution is not guaranteed to survive closing Studio. Codex supports steering an active Turn; OpenCode mode intentionally disables send while busy because its server has no equivalent steer operation.
