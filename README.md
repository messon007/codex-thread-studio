# Codex Thread Studio

An independent, unofficial Tauri desktop client for the structured [`codex app-server`](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) protocol.

It lists Codex Threads directly, renders structured Turns and Items, handles approvals, and sends messages through the App Server v2 API. It is not an Agent Deck frontend and does not embed a terminal or depend on tmux.

[简体中文](README.zh-CN.md)

## Features

- Groups Threads by project directory; lists, starts, automatically resumes, reloads, renames, forks, archives, and deletes persisted Codex Threads; and shows native session-tree/fork/parent metadata.
- Renders user and agent messages, reasoning summaries, plans, commands, command output, file changes, tool calls, turn status, errors, and usage. Agent prose uses locally vendored, sanitized GitHub-flavored Markdown with readable code blocks and tables.
- Streams long responses efficiently by batching App Server deltas and updating only the active Item; completed history is not reparsed for every token.
- Sends new turns, steers an active turn, interrupts work, and answers command/file/permission approvals.
- Provides `@` project-file search and a keyboard-first `/` command palette for models, reasoning effort, permissions, status, compact, review, diff, skills, MCP servers, and Thread operations.
- Lets the user select structured output, attach comments anchored to the originating Turn and Item, assemble repeated annotations, and insert the result into the composer without sending it.
- Provides persistent light/dark themes, typography, contrast, Comfortable/Wide/Full content width, selected thread, comment drafts, and a configurable annotation prompt template.
- Uses the installed Codex CLI and existing Codex authentication; Studio stores no model credential.

## Architecture

```text
Tauri WebView
  └─ same-origin WebSocket
      └─ Rust broker
          └─ JSONL over stdin/stdout
              └─ codex app-server
                  └─ Codex threads, turns, tools, and project files
```

The Rust broker owns the one-time `initialize`/`initialized` handshake. Browser requests and server events then pass through as JSON-RPC messages with their native payloads preserved.

## Requirements

- Rust stable toolchain
- Tauri 2 system dependencies
- A current Codex CLI with `codex app-server`
- A supported system WebView (WebKitGTK on Linux or WKWebView on macOS)

## Run

```bash
./scripts/install-linux-dev-deps.sh
codex --version
cargo run -p codex-thread-studio
```

Build without launching:

```bash
cargo build -p codex-thread-studio
./target/debug/codex-thread-studio
```

If a GUI launcher cannot find Codex:

```bash
CODEX_THREAD_STUDIO_CODEX_BIN=/absolute/path/to/codex cargo run -p codex-thread-studio
```

## Verification

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
```

The desktop runtime has no Node.js or network dependency. Markdown browser assets and their license texts are committed under `ui/vendor`; maintainers can refresh the lockfile-pinned copies with `npm ci && npm run vendor:markdown`.

See [Maturity roadmap](docs/maturity-roadmap.md), [Architecture](docs/architecture.md), [Product and acceptance criteria](docs/product.md), [Development](docs/development.md), and [Troubleshooting](docs/troubleshooting.md).

## Current lifecycle boundary

The first implementation runs one App Server child per Studio process. Codex Thread history is persisted by Codex, but closing Studio can interrupt an active Turn. Moving process ownership to the App Server daemon/control socket is the next lifecycle milestone.

## License

MIT. This is an unofficial client and is not an OpenAI product. See [Third-party notices](THIRD_PARTY_NOTICES.md) for vendored UI dependencies.
