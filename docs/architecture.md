# Structured multi-backend architecture

## Process model

The first release uses three logical layers and normally two application processes plus WebKitGTK helper processes:

```text
Tauri process
├─ OS WebView: shared HTML/CSS/JavaScript UI
└─ Rust/Axum loopback gateway
   ├─ WebSocket ↔ Codex JSONL broker ↔ codex app-server
   └─ HTTP/SSE proxy ↔ password-protected opencode serve
```

There is no terminal emulator, tmux process, direct model API integration, or separate Studio session database. Each backend remains the source of truth for its own sessions and credentials.

## Transport and handshake

App Server uses newline-delimited JSON over stdio. The Rust broker:

1. resolves Codex from `CODEX_THREAD_STUDIO_CODEX_BIN`, NVM/FNM/Cargo/common user paths, then `PATH`;
2. starts `codex app-server --stdio` on the first WebSocket connection;
3. sends one `initialize` request with Studio client metadata;
4. sends `initialized` only after the initialization response;
5. forwards subsequent protocol messages without adding `jsonrpc`;
6. broadcasts stdout messages to the WebView and exposes stderr as diagnostic events; and
7. rejects browser attempts to repeat the handshake or use Studio's reserved request ID.

The App Server experimental WebSocket listener is deliberately not used. Studio exposes its own loopback-only WebSocket so the WebView never owns a local subprocess or filesystem capability.

## OpenCode transport

The Rust gateway resolves `opencode`, starts `opencode serve` lazily on a random loopback port, and supplies a fresh `OPENCODE_SERVER_PASSWORD`. Only Rust knows that password. The WebView calls same-origin `/opencode/*`; Rust adds Basic authentication and streams ordinary HTTP or SSE responses.

The adapter uses `/experimental/session` for the cross-project list, `/session/*` for CRUD/history/prompt/abort, and `/global/event` for live events. Every project-scoped request carries the session `directory`. SSE reconnects trigger history reconciliation because OpenCode does not expose an SSE replay cursor.

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

Agent prose and reasoning summaries pass through locally vendored Marked (GFM parsing), then DOMPurify (HTML sanitization), then GitHub Markdown CSS plus Studio theme overrides. Code-copy and table wrappers are added only after sanitization. The outer structured Item element remains the selection/comment anchor.

## Streaming render path

App Server delta notifications update only the affected Item in the browser model. Studio batches high-frequency updates with `requestAnimationFrame` and patches the active message, plan, reasoning, or command-output node directly. Markdown parsing and sanitization run after Item completion, not once for every token delta. Structural notifications still trigger a full transcript render.

Codex Threads loaded during the current Studio process remain subscribed so their cached render models can receive routed notifications. OpenCode uses the global SSE stream and routes events by session ID. Backend changes close the old browser transport, reject its pending UI requests, reset transient render state, and restore the cached model for the last selected session when it remains valid.

## Composer orchestration

The composer is a structured App Server client rather than a terminal command parser:

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

Codex and OpenCode own history and credentials in their normal state directories. Studio stores UI language, appearance settings, per-backend selection, language-specific comment templates, and explicit comment drafts in:

```text
~/.config/codex-thread-studio/settings.json
```

The global favorites library is intentionally separate because it can contain substantially larger user-selected message content:

```text
~/.config/codex-thread-studio/favorites.json
```

Favorites use validated, bounded records and atomic temporary-file replacement. Search returns summaries; full Markdown content is loaded only when a favorite is opened.

On first launch, the app imports compatible settings from the former experimental path at `~/.config/agent-deck-studio/codex-native-settings.json` when the new file does not yet exist. Writes use a temporary file plus rename. Payload shape and size are validated in Rust.

The WebView uses a local Chinese-to-English interface catalog for both initial markup and controls rendered after App Server events. Thread titles, project paths, prompts, AI responses, favorites, comments, code, and tool output are protected from translation. Legacy single-template settings are detected and moved to the matching language slot.

## Security boundaries

- The gateway binds to a random `127.0.0.1` port.
- No provider credential or OpenCode server password crosses into browser storage or Studio preferences.
- App Server and Codex configuration decide sandbox and approval behavior.
- User, command, diff, tool, and unknown payloads are escaped as text. Agent Markdown is sanitized with DOMPurify; scripts, styles, frames, embedded objects, forms, buttons, and inline style attributes are forbidden.
- App Server request messages and proxied OpenCode bodies are bounded to 4 MiB; preferences are bounded to 1 MiB.

## Lifecycle limitation

Backend children belong to the Tauri process in this milestone. History is durable, but active execution is not guaranteed to survive closing Studio. Codex supports steering an active Turn; OpenCode mode intentionally disables send while busy because its server has no equivalent steer operation.
