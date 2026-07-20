# Codex native architecture

## Process model

The first release uses three logical layers and normally two application processes plus WebKitGTK helper processes:

```text
Tauri process
├─ OS WebView: HTML/CSS/JavaScript structured UI
└─ Rust/Axum loopback gateway
       │ WebSocket messages
       ▼
codex app-server process
       │ Codex core
       ▼
$CODEX_HOME state + selected project directory
```

There is no terminal emulator, tmux process, third-party session backend, HTTP model API integration, or separate Studio session database.

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

The browser keeps only a render model reconstructed from `thread/resume` plus live notifications. Codex remains the source of truth. Every selectable rendered block carries `data-turn-id` and `data-item-id`.

Agent prose and reasoning summaries pass through locally vendored Marked (GFM parsing), then DOMPurify (HTML sanitization), then GitHub Markdown CSS plus Studio theme overrides. Code-copy and table wrappers are added only after sanitization. The outer structured Item element remains the selection/comment anchor.

## Streaming render path

App Server delta notifications update only the affected Item in the browser model. Studio batches high-frequency updates with `requestAnimationFrame` and patches the active message, plan, reasoning, or command-output node directly. Markdown parsing and sanitization run after Item completion, not once for every token delta. Structural notifications still trigger a full transcript render.

When the selected Thread changes, Studio first calls `thread/unsubscribe` for the previous Thread and then resumes the new one. This keeps background activity out of the visible render model while Codex continues to own all durable history.

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

A comment draft is stored as:

```text
{ threadId, turnId?, itemId?, quote, comment, createdAt }
```

The quote remains human-readable even when old history changes. Turn/Item IDs preserve the structured source anchor. The configurable template assembles all comments and optional overall guidance into ordinary text for `turn/start` or `turn/steer`; insertion never sends automatically.

## Persistence

Codex owns thread history and credentials in its normal state directory. Studio stores only UI settings and explicit comment drafts in:

```text
~/.config/codex-thread-studio/settings.json
```

On first launch, the app imports compatible settings from the former experimental path at `~/.config/agent-deck-studio/codex-native-settings.json` when the new file does not yet exist. Writes use a temporary file plus rename. Payload shape and size are validated in Rust.

## Security boundaries

- The gateway binds to a random `127.0.0.1` port.
- No OpenAI credential crosses into browser storage or Studio preferences.
- App Server and Codex configuration decide sandbox and approval behavior.
- User, command, diff, tool, and unknown payloads are escaped as text. Agent Markdown is sanitized with DOMPurify; scripts, styles, frames, embedded objects, forms, buttons, and inline style attributes are forbidden.
- App Server request messages are bounded to 4 MiB; preferences are bounded to 1 MiB.

## Lifecycle limitation

The App Server child belongs to the Tauri process in this milestone. Thread history is durable, but active execution is not guaranteed to survive closing Studio. A later implementation should start/connect to the supported App Server daemon/control socket and rehydrate missed events from persisted thread state.
