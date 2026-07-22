# Codex Thread Studio

An independent, unofficial Tauri desktop client for structured Codex and OpenCode sessions. It connects to [`codex app-server`](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) or a local [`opencode serve`](https://opencode.ai/docs/server/) process without scraping a terminal.

It lists Codex Threads directly, renders structured Turns and Items, handles approvals, and sends messages through the App Server v2 API. It is not an Agent Deck frontend and does not embed a terminal or depend on tmux.

[简体中文](README.zh-CN.md)

## Features

- Groups Threads by project directory; lists, starts, automatically resumes, reloads, renames, forks, archives, and deletes persisted Codex Threads; and shows native session-tree/fork/parent metadata.
- Renders user and agent messages, reasoning summaries, plans, commands, command output, file changes, tool calls, turn status, errors, and usage. Agent prose uses locally vendored, sanitized GitHub-flavored Markdown with readable code blocks and tables.
- Streams long responses efficiently by batching App Server deltas and updating only the active Item; completed history is not reparsed for every token.
- Sends new turns, steers an active turn, interrupts work, and answers command/file/permission approvals.
- Provides `@` project-file search, direct `$skill-name` discovery with structured App Server skill inputs, `!command` local shell mode, and a keyboard-first `/` command palette for models, reasoning effort, permissions, status, compact, review, diff, skills, MCP servers, and Thread operations.
- Provides a compact in-session Turn navigator: the current interaction is highlighted, hover reveals user-prompt previews, and selecting a marker scrolls directly to that Turn.
- Lets the user select structured output, attach comments anchored to the originating Turn and Item, assemble repeated annotations, and insert the result into the composer without sending it.
- Provides persistent light/dark themes, typography, contrast, Comfortable/Wide/Full content width, selected thread, comment drafts, and a configurable annotation prompt template.
- Switches between isolated Codex and OpenCode session lists, remembers the selected session for each backend, and namespaces comment drafts per backend.
- Uses installed CLIs and their existing authentication; Studio stores no model credential. The Rust layer starts OpenCode with an ephemeral password that never enters browser storage.

## Architecture

```text
Tauri WebView
  └─ Rust loopback gateway
      ├─ WebSocket ↔ JSONL stdio ↔ codex app-server
      └─ same-origin HTTP/SSE proxy ↔ opencode serve
```

The Rust broker owns the Codex handshake and the OpenCode process/authentication boundary. The WebView receives a common Thread/Turn/Item render model while backend-native data stays available to the adapters.

## Requirements

- Rust stable toolchain
- Tauri 2 system dependencies
- A current Codex CLI with `codex app-server`
- OpenCode CLI for OpenCode mode (`opencode serve`)
- A supported system WebView (WebKitGTK on Linux or WKWebView on macOS)

## Run

```bash
./scripts/install-linux-dev-deps.sh
codex --version
opencode --version
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

Use `CODEX_THREAD_STUDIO_OPENCODE_BIN=/absolute/path/to/opencode` when OpenCode is outside the GUI launcher's `PATH`.

## Verification

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
npm run version:check
```

The desktop runtime has no Node.js or network dependency. Markdown browser assets and their license texts are committed under `ui/vendor`; maintainers can refresh the lockfile-pinned copies with `npm ci && npm run vendor:markdown`.

## Versions and releases

`src-tauri/Cargo.toml` is the single manually maintained application-version source; Tauri inherits it automatically. The About panel shows the same compile-time value.

Release notes live in [CHANGELOG.md](CHANGELOG.md). Maintainers add notes under **Unreleased**, run `npm run version:bump -- <semver>`, verify with `npm run version:check`, commit, and push a matching `v<semver>` tag. The tag workflow validates all version metadata and creates a draft GitHub Release with Linux and macOS bundles.

See [Maturity roadmap](docs/maturity-roadmap.md), [Architecture](docs/architecture.md), [Product and acceptance criteria](docs/product.md), [Development](docs/development.md), and [Troubleshooting](docs/troubleshooting.md).

## Current lifecycle boundary

The first implementation owns one child process for each backend used during the Studio run. Session history is persisted by Codex/OpenCode, but closing Studio can interrupt active work.

## License

MIT. This is an unofficial client and is not an OpenAI product. See [Third-party notices](THIRD_PARTY_NOTICES.md) for vendored UI dependencies.
