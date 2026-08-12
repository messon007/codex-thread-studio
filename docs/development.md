# Development guide

## Toolchain

- Rust stable with `rustfmt`
- Tauri 2 native dependencies
- Node.js for browser-state unit tests and deliberately refreshing vendored Markdown/Mermaid assets; the runtime UI has no bundler or package install step
- A current Codex CLI with `codex app-server`
- A current OpenCode CLI with `opencode serve` when testing OpenCode mode
- WebKitGTK on Linux, WKWebView on macOS, or WebView2 on Windows 11

## Commands

```bash
./scripts/install-linux-dev-deps.sh
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
npm run version:check
cargo run -p codex-thread-studio
```

### Capturing the running Studio window

Linux debug builds expose a user-private Unix socket for capturing Studio's own window. This does
not request global GNOME screenshot access and cannot capture any other application. It works in
both the ordinary Tauri WebView window and the native Wayland embedded-browser window. The ordinary
window uses WebKit's visible-page snapshot API; the embedded composition captures the complete GTK
window. Start Studio normally, then run:

```bash
target/debug/codex-thread-studio --dev-screenshot
target/debug/codex-thread-studio --dev-show-browser
target/debug/codex-thread-studio --dev-hide-browser
target/debug/codex-thread-studio --dev-exit-browser
target/debug/codex-thread-studio --dev-new-browser-tab https://example.org
target/debug/codex-thread-studio --dev-show-browser-menu
target/debug/codex-thread-studio --dev-show-browser-info
target/debug/codex-thread-studio --dev-show-browser-downloads
target/debug/codex-thread-studio --dev-crash-browser-tab
target/debug/codex-thread-studio --dev-open-browser https://example.com
```

The command prints the PNG path beneath `$XDG_RUNTIME_DIR/codex-thread-studio-dev-captures/`.
The socket and generated files are accessible only to the current user and are cleared with the
runtime directory. Release builds reject all developer-control commands.
`--dev-show-browser` is idempotent, `--dev-show-browser-menu` opens the overflow menu,
`--dev-show-browser-info` opens the browser information panel,
and `--dev-open-browser` both shows the embedded browser and
navigates its active tab after applying the normal URL policy. These controls let automated
development checks open the browser before capturing Studio without desktop-wide input access.

`--dev-crash-browser-tab` deliberately terminates the active page renderer and then enters the
same recovery path used by an unexpected WebKit failure. The first failure in a 30-second window
must replace the page renderer; a second failure must stop automatic recovery and show the crashed
Tab state. Use an isolated `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` and
`XDG_RUNTIME_DIR` when running this test so it cannot attach to a normal Studio instance.

The Markdown and Mermaid stack is pinned in `package-lock.json` and copied into `ui/vendor`, which Rust embeds at compile time. To deliberately update it:

```bash
npm ci
npm audit --audit-level=moderate
npm run vendor:markdown
git diff -- package-lock.json ui/vendor THIRD_PARTY_NOTICES.md
```

Never replace the vendored modules with runtime CDN imports. Keep their license files in `ui/vendor/licenses/` and review sanitizer advisories before an upgrade.

CodeMirror and xterm.js are also pinned and split into lazy browser bundles. Refresh them only after
reviewing the dependency and generated-code diff:

```bash
npm ci
npm audit --audit-level=moderate
npm run vendor:workspace
git diff -- package-lock.json ui/vendor/workspace-editor.mjs ui/vendor/workspace-terminal.mjs ui/vendor/xterm.css
```

## Versioning and releases

The project follows SemVer. `src-tauri/Cargo.toml` is the only manually maintained version source. `tauri.conf.json` intentionally omits `version`, so Tauri reads Cargo's package version; `Cargo.lock` is generated metadata and is kept in sync by the version script.

Prepare a release as follows:

1. Add user-visible changes beneath `CHANGELOG.md` → **Unreleased**.
2. Run `npm run version:bump -- 0.3.0` with the intended SemVer value.
3. Run `npm run version:check` and the normal verification suite.
4. Commit the release preparation.
5. Create and push an annotated matching tag, for example `git tag -a v0.3.0 -m "Codex Thread Studio v0.3.0"` and `git push origin v0.3.0`.
6. Review and publish the draft GitHub Release produced by `.github/workflows/release.yml`.

`version:check` rejects mismatches between Cargo and `Cargo.lock`, a duplicated Tauri config version, a missing changelog section, or a release tag that differs from `v<Cargo version>`. Linux, macOS, and Windows 11 desktop bundles are built automatically. The Windows client requires WSL2 for its Codex and OpenCode backends.

Windows runs the Studio client natively while both AI backends run inside WSL2. It does not discover or launch Windows `codex.cmd`, `codex.exe`, or `opencode.exe`. Configure the WSL distribution, Linux user, and backend commands in Studio settings, restart Studio, and use Linux project paths such as `/home/user/project`.

```powershell
wsl --list --verbose
wsl -d Ubuntu -- bash -lc 'command -v codex; codex --version; command -v opencode; opencode --version'
cargo run --package codex-thread-studio
```

## Protocol development

Generate bindings matching the installed Codex version when protocol fields change:

```bash
codex app-server generate-ts --out /tmp/codex-app-server-ts
codex app-server generate-json-schema --out /tmp/codex-app-server-schema
```

Stable APIs are used by default. Do not add experimental fields without setting `capabilities.experimentalApi` during initialization and documenting the compatibility cost.

For OpenCode protocol work, inspect the exact installed schema at the child server's `GET /doc`. Cross-project listing uses `/experimental/session`; all project-scoped requests must include `directory`. Never expose `OPENCODE_SERVER_PASSWORD` to the WebView.

Protocol responsibilities are split as follows:

- `src-tauri/src/codex_app_server.rs`: executable resolution, process lifecycle, initialization, JSONL transport, WebSocket bridge, limits.
- `src-tauri/src/opencode_server.rs`: OpenCode resolution, password-protected process lifecycle, health checks, and streaming HTTP/SSE proxy.
- `ui/codex-native.mjs`: provider event normalization and unit-testable render state.
- `ui/opencode-native.mjs`: OpenCode session/history/event normalization into the shared render state.
- `ui/app.js`: RPC correlation, Thread flows, structured rendering, approval UI, comments, native favorites, settings.
- `ui/favorites.mjs`: pure helpers for Turn question association, source identities, titles, tags, and copy output.
- `src-tauri/src/favorites.rs`: validated global favorites persistence and search summaries.
- `ui/vendor`: lockfile-pinned Marked, DOMPurify, Mermaid, GitHub Markdown CSS, and license texts for offline rendering.

## Compatibility checks

Before accepting a Codex CLI upgrade:

1. Generate the installed schema.
2. Confirm initialization and `thread/list`.
3. Resume a disposable persisted Thread.
4. Run one Turn that executes a harmless command and changes a disposable file.
5. Exercise accept and decline approval paths.
6. Interrupt a Turn and verify the final status.
7. Select output, create multiple comments, restart Studio, and verify draft persistence.

## Generated files

Do not commit `target/`, `src-tauri/target/`, `node_modules/`, generated Tauri schemas, or temporary generated App Server schemas unless a deliberate version-pinning decision is made. The reviewed `ui/vendor/` Markdown assets are intentionally committed.
