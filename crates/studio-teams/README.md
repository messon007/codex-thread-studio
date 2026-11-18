# studio-teams

Private, optional Rust domain and queue components for Studio Agent Teams. This crate has no network runtime, Tauri/MarkNoria dependency, native RPC client, MCP endpoint or UI. Adding it to the workspace does not enable Teams in the application.

`Store` commits task changes, incremental audit, compact idempotent receipts and logical effects in SQLite transactions. Delegation, task-local waiting, questions, results and review are explicit commands. Native turn completion never means task completion. Only the trusted host constructs `Caller` and supplies turn observations; these are not tool parameters.

`Queue` is an independent SQLite component for stable admission, FIFO, reservations, one-shot dispatch permission, fences and uncertain recovery. `prepare_send` permits **one** native queue/add call. It never calls Codex itself. Unknown outcomes are not retried. An explicitly abandoned unknown item releases ordinary queued input but still blocks strict supervision/new scope activation until native outcome is known.

The host integration must own the worker exclusively before `Queue::recover` and `Store::recover_turns`, reconcile fresh observations, and connect domain invalidation to queue fences. Do not expose either low-level store as a public network API. `effect_ready` is a candidate check, not an atomic cross-database dispatch authorization. Group pause/cancel currently changes domain state only; no production queue is wired to this crate.

The internal Rust command enum and receipt types are not the final flat MCP wire contract. Read APIs expose host-scoped monotonic cursors, not yet signed/filter-bound external pagination. Archive/member editing, full control-plane reconciliation and the optional orchestrator's root-goal endpoint remain integration work. Failed/unknown claims intentionally remain blocked until that control plane supplies evidence.

Tests use only temporary SQLite databases and synthetic host events. For the current worktree:

```bash
mkdir -p /media/rui/codex-thread-studio-agent-teams/validation/rust-tmp
TMPDIR=/media/rui/codex-thread-studio-agent-teams/validation/rust-tmp \
CARGO_TARGET_DIR=/media/rui/codex-thread-studio-agent-teams/target \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
cargo test -p studio-teams --offline --locked
```

The dependencies reuse existing workspace versions. No desktop build or network download is needed for these tests. See [implementation status](../../docs/agent-teams/implementation-status.md) for the explicit milestone and remaining work.
