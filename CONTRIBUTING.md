# Contributing

Thanks for helping improve Codex Thread Studio.

## Principles

- Keep Codex App Server as the source of truth for Threads, Turns, Items, approvals, and execution state.
- Keep the WebView unprivileged: local process and state access belongs in the Rust broker.
- Preserve unknown provider payloads and degrade visibly instead of silently inventing state.
- Keep model Markdown behind the reviewed Marked → DOMPurify pipeline. Continue escaping user, tool, command, diff, and unknown payloads; do not bypass sanitization or add runtime CDN assets.
- Keep comment insertion non-submitting so users can inspect assembled feedback.
- Do not introduce terminal scraping or a tmux/session-manager dependency into this structured client.

## Before a pull request

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
npm test
npm run version:check
```

For protocol changes, manually check App Server initialization, thread list/resume, one streamed Turn, approval accept/decline, interruption, and comment persistence. State which Codex CLI version was tested.

Never commit credentials, private Thread content, generated build output, or `$CODEX_HOME` state.

Do not edit `Cargo.lock` or `tauri.conf.json` to change the app version. Add release notes under `CHANGELOG.md` **Unreleased**, then use `npm run version:bump -- <semver>`; see `docs/development.md` for the tag and release workflow.
