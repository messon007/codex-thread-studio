# Development guide

## Toolchain

- Rust stable with `rustfmt`
- Tauri 2 native dependencies
- Node.js for browser-state unit tests and deliberately refreshing vendored Markdown assets; the runtime UI has no bundler or package install step
- A current Codex CLI with `codex app-server`
- WebKitGTK on Linux or WKWebView on macOS

## Commands

```bash
./scripts/install-linux-dev-deps.sh
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
cargo run -p codex-thread-studio
```

The Markdown stack is pinned in `package-lock.json` and copied into `ui/vendor`, which Rust embeds at compile time. To deliberately update it:

```bash
npm ci
npm audit --audit-level=moderate
npm run vendor:markdown
git diff -- package-lock.json ui/vendor THIRD_PARTY_NOTICES.md
```

Never replace the vendored modules with runtime CDN imports. Keep their license files in `ui/vendor/licenses/` and review sanitizer advisories before an upgrade.

## Protocol development

Generate bindings matching the installed Codex version when protocol fields change:

```bash
codex app-server generate-ts --out /tmp/codex-app-server-ts
codex app-server generate-json-schema --out /tmp/codex-app-server-schema
```

Stable APIs are used by default. Do not add experimental fields without setting `capabilities.experimentalApi` during initialization and documenting the compatibility cost.

Protocol responsibilities are split as follows:

- `src-tauri/src/codex_app_server.rs`: executable resolution, process lifecycle, initialization, JSONL transport, WebSocket bridge, limits.
- `ui/codex-native.mjs`: provider event normalization and unit-testable render state.
- `ui/app.js`: RPC correlation, Thread flows, structured rendering, approval UI, comments, settings.
- `ui/vendor`: lockfile-pinned Marked, DOMPurify, GitHub Markdown CSS, and license texts for offline rendering.

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
