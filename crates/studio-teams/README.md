# studio-teams

Private optional Rust domain, wire-contract and queue components for Studio Agent Teams. The crate has no browser, Tauri, MarkNoria or network runtime dependency. The host adapter and independently mounted UI are implemented separately; adding the crate alone does not enable Teams.

`Store` transactionally persists membership, tasks, dependency checks, execution claims, questions, results, reviews, incremental audit, compact idempotency receipts and logical effects. Waiting belongs to a task, not a session. Native turn completion releases a receipt barrier; it never completes a business task implicitly.

`tools_list` and `call_tool` expose eleven flat MCP tools with schema validation, revision/attempt checks, consistent errors and actor/filter-bound opaque pagination. Only the trusted host constructs `Caller`; caller identities are not tool parameters. Host-only maintenance handles membership/profile changes, archival, group goals, rework budgets and evidence-based reconciliation.

`Queue` owns stable admission, FIFO, exclusive reservations, one-shot dispatch authorization, durable fences and uncertain recovery. `prepare_send` permits one native queue/add call; the crate does not make that call. Unknown outcomes are never retried. Explicitly abandoned unknown sends still block strict supervision and reactivation until their native outcome is known.

The adapter in `src-tauri/src/teams.rs` owns the worker, correlates native receipts and serializes domain/fence/dispatch transitions. A domain `effect_ready` read alone is not send authorization. Ordinary Studio sends share its reservations and a separate durable dispatch ledger; their existing payload storage uses version CAS.

Run isolated tests without a desktop build:

```bash
TMPDIR=/media/rui/codex-thread-studio-agent-teams/validation/rust-tmp \
CARGO_TARGET_DIR=/media/rui/codex-thread-studio-agent-teams/target \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
cargo test -p studio-teams --offline --locked
```

See [implementation status](../../docs/agent-teams/implementation-status.md) and [host integration](../../docs/agent-teams/host-integration.md) for installation, compatibility and shutdown behavior.
