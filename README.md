# Codex Thread Studio

An independent, unofficial Tauri desktop client for structured Codex and OpenCode sessions. It connects to [`codex app-server`](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) or a local [`opencode serve`](https://opencode.ai/docs/server/) process without scraping a terminal.

It lists Codex Threads directly, renders structured Turns and Items, handles approvals, and sends messages through the App Server v2 API. It is not an Agent Deck frontend and does not depend on tmux. Its optional project Terminal is a user-controlled workspace tool, never a transport fallback for Agent events.

[简体中文](README.zh-CN.md)

## Features

- Groups Threads by project directory; lists, starts, automatically resumes, reloads, renames, forks, archives, and deletes persisted Codex Threads; and can hide configured directory subtrees from the sidebar without changing backend history. See [session directory filtering](docs/session-directory-filtering.md).
- Renders user and agent messages, reasoning summaries, plans, commands, command output, file changes, tool calls, turn status, errors, and usage. Agent prose uses locally vendored, sanitized GitHub-flavored Markdown with readable code blocks, tables, configurable reading density, and fenced Mermaid diagrams. See [rendering configuration](docs/rendering-configuration.md).
- Streams long responses efficiently by batching App Server deltas and updating only the active Item; completed history is not reparsed for every token.
- Sends new turns, steers an active turn, interrupts work, and answers command/file/permission approvals.
- Renders structured `requestUserInput` questions and MCP elicitation forms, reconciles the selected Thread after reconnect, and can notify when work completes or needs input.
- Opens project files in a local artifact rail: text/Markdown/HTML, images, EPUB, searchable PDF with text or region comments, and CSV/XLSX grids with a compact chart view.
- Provides lazy project Files and a PTY Terminal. Per-project environment profiles support variables, redacted Secrets, Codex/Terminal environment injection, restricted/enabled network policy, allowed-host metadata, and configurable cache environment variables.
- Adds a session-scoped Git Review rail for code, Markdown, CSV, and other text changes: filter all/staged/unstaged files, inspect line-numbered unified diffs, switch between index and working-tree changes, and safely stage or unstage one file without exposing a discard action.
- Provides `@` project-file search, direct `$skill-name` discovery with structured App Server skill inputs, `!command` local shell mode, and a keyboard-first `/` command palette for models, reasoning effort, permissions, status, compact, review, diff, skills, MCP servers, and Thread operations.
- Creates and reuses one visible system-managed Router session on Codex or OpenCode: its backend is selected in the configuration file and takes effect after restart. Session responsibilities are edited or AI-generated in Session information; Router settings hold only up to three conditional fallback targets, while every other session is a regular target. It sends the request as a normal user message to the selected Codex/OpenCode target and links to the completed response. It does not depend on Agent Deck, tmux, or a separate conductor daemon.
- Provides a compact in-session Turn navigator: the current interaction is highlighted, hover reveals user-prompt previews, and selecting a marker scrolls directly to that Turn.
- Lets the user select structured output, attach comments anchored to the originating Turn and Item, assemble repeated annotations, and insert the result into the composer without sending it.
- Saves any complete AI message directly from its structured Item, optionally includes the user question from the same Turn, and provides a searchable cross-backend, cross-thread favorites library with source navigation.
- Stores favorites in SQLite, automatically migrates the legacy JSON library, and exports the full global library as Markdown.
- Provides persistent Chinese/English/system-language selection, light/dark themes, typography, contrast, Comfortable/Wide/Full content width, selected thread, comment drafts, and language-specific annotation prompt templates.
- Switches between isolated Codex and OpenCode session lists, remembers the selected session for each backend, and namespaces comment drafts per backend.
- Uses installed CLIs and their existing authentication; Studio stores no model credential. The Rust layer starts OpenCode with an ephemeral password that never enters browser storage.
- Includes a global Embedded Browser Workspace. Linux uses its native GTK/WebKitGTK container and
  Windows uses same-window WRY/WebView2 child views; remote pages are isolated from the Studio
  gateway and Tauri capabilities. It is lazy by default and does not depend on either AI backend.
  See the [Windows native implementation boundary](docs/windows-native.md).

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
- A supported system WebView (WebKitGTK on Linux, WKWebView on macOS, or WebView2 on Windows 11)
- WSL2 with Codex and/or OpenCode installed inside the selected distribution when running the Windows client

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

On Windows, Studio itself is native but its AI backends run only inside WSL2; Windows `.cmd` and `.exe` backend installations are not used. Configure the distribution, Linux user, and optional backend paths in Settings, restart Studio, and enter project paths in Linux form such as `/home/user/project`.

```powershell
wsl --list --verbose
wsl -d Ubuntu -- bash -lc 'codex --version; opencode --version'
cargo run --package codex-thread-studio
```

## Verification

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
npm run version:check
```

The desktop runtime has no Node.js or CDN dependency. Browser assets and license texts are committed under `ui/vendor`; maintainers can refresh lockfile-pinned copies with the `vendor:*` npm scripts.

## Versions and releases

`src-tauri/Cargo.toml` is the single manually maintained application-version source; Tauri inherits it automatically. The About panel shows the same compile-time value.

Release notes live in [CHANGELOG.md](CHANGELOG.md). Maintainers add notes under **Unreleased**, run `npm run version:bump -- <semver>`, verify with `npm run version:check`, commit, and push a matching `v<semver>` tag. The tag workflow validates all version metadata and creates a draft GitHub Release with Linux, macOS, and Windows bundles.

See [Maturity roadmap](docs/maturity-roadmap.md), [Architecture](docs/architecture.md), [Thread Router specification](docs/thread-router/specification.md), the experimental [Browser Workspace specification](docs/browser-workspace/specification.md), [Product and acceptance criteria](docs/product.md), [Development](docs/development.md), and [Troubleshooting](docs/troubleshooting.md).

## Current lifecycle boundary

The first implementation owns one child process for each backend used during the Studio run. Session history is persisted by Codex/OpenCode, but closing Studio can interrupt active work.

## License

MIT. This is an unofficial client and is not an OpenAI product. See [Third-party notices](THIRD_PARTY_NOTICES.md) for vendored UI dependencies.
