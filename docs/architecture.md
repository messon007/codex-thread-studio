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
