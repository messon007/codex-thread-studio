# Session Map experimental implementation

Status: implemented on `experiment/session-map`; not yet a release commitment.

## Product boundary

- A new or existing Thread has no Map by default.
- Studio does not backfill Maps and does not create an empty Map while loading a Thread.
- When `(backend, thread_id)` has no database row, the Map rail and placeholder are absent. The only entry point is `••• → Create Map`.
- Creating or deleting a Map never changes provider-owned Thread/Turn history.
- One Map belongs to exactly one backend + Thread pair.
- Creating a Codex Map starts one ephemeral structured request to generate its initial Items from the explicit goal and recent conversation. If that request was interrupted and the Map is still empty, selecting the Thread retries once per app run. The Map menu also offers manual AI Generate/Complete.

## Storage and API

Studio stores Maps in `session-maps.sqlite3` beside `settings.json` and `studio.sqlite3` under the normal platform configuration directory. Rust owns four tables: Maps, Items, Relations, and revisioned change snapshots.

The loopback gateway exposes:

- `POST /studio/session-map`
- `GET|DELETE /studio/session-map/{backend}/{threadId}`
- `POST /studio/session-map/{backend}/{threadId}/operations`
- `POST /studio/session-map/{backend}/{threadId}/undo`

Operation batches carry `baseRevision`. A stale revision returns HTTP 409 without changing the Map. Rust validates size limits, depth, cycles, anchors, state values, operation permissions, and the full resulting snapshot inside one SQLite transaction.

## Unified operations

Hierarchy, Path, Flow, and Blank use the same data model and protocol:

`setGoal`, `addItem`, `updateItem`, `moveItem`, `reorderItem`, `setState`, `setCurrent`, `archiveItem`, `addRelation`, and `removeRelation`.

Assistant-originated batches are intentionally narrower. They may add Items, update Item text, set the current Item, and use only `notStarted`, `active`, or `visited`. They cannot change the goal, mark completion, archive, move/reorder, or modify Relations.

## Codex synchronization

Before a Codex Turn starts, Studio attaches a bounded Map snapshot as developer context. When the installed App Server supports experimental Dynamic Tools, the same configuration exposes `update_session_map`, and Studio applies its operation batch during the Turn.

When Dynamic Tools are unavailable, the same visible Turn appends a versioned machine-readable Map block after its normal Markdown answer. Studio filters that block from streaming, completed rendering, copy, and favorites; then it parses and sends the operations through the same Rust validation path. Missing, malformed, stale, or unsafe blocks leave the Map unchanged and show a sync error. Studio does not automatically invoke another model to repair the block.

The raw provider history still contains the fallback block even though Studio hides it. This is an accepted compatibility cost of avoiding an additional model request.

Initial generation, manual AI completion, and goal suggestions use one reusable ephemeral structured Thread per Map. Those jobs are serialized through the same worker. A new worker is created only when the Codex App Server generation changes, because the previous ephemeral Thread can no longer exist there. Deleting a Map releases its worker.

OpenCode Maps can currently be created and edited, but automatic synchronization is disabled until a same-provider structured worker/tool adapter is implemented. Studio never silently sends OpenCode content to Codex for reconciliation.

Goal editing is always explicit. “AI suggestion” fills the edit form using an ephemeral Codex worker; the user must confirm Save before the goal changes.

## Verification

```bash
npm test
cd src-tauri && cargo test
```

Manual acceptance:

1. Select several old Threads and confirm no Map rail appears.
2. Create a Map for one Thread and confirm initial Items appear automatically; switch away and confirm only that Thread restores the rail.
3. Add nested Items, set current/visited/done states, edit the goal, then undo each update.
4. Start a Codex Turn and verify the visible reply never shows the response-tail block while the Map updates without a second model request.
5. Delete the Map and confirm the original no-Map layout returns while chat history remains intact.
