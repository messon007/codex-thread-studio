# Router inline workspace

The Router is a chat entry point, not a task supervisor. Session Map and automatic
task continuation are unchanged.

## Interaction

- Automatic routing considers session responsibilities and recent target-response
  excerpts. An ambiguous decision asks a question without dispatching.
- Type a question followed by `@` to see local session suggestions. Partial titles,
  project paths and responsibility text are searchable. Selecting a suggestion
  creates a visible target selection; it does not insert an opaque session ID.
- The `@` menu groups sessions and files separately. File suggestions search only
  the explicitly selected target's project; without a target, the file group asks
  the user to select one. Choosing a file inserts a reference, not a routing target.
- An explicit selection stays selected until cleared. “Reply here” selects the
  response's source session. The exact user prompt is forwarded in explicit mode.
  Currently the controller still creates a structured routing turn first, even for
  an explicit target; this is not a zero-latency bypass of the controller model.
- Responses appear inline with their source identity. Different target sessions
  can run concurrently; a busy target rejects a new request instead of steering
  its existing task implicitly.
- Files, Terminal and Review use the response's source project. Opening those
  tools does not select a different main session or change the Composer target.
  Returning from a file retains its original Files/Review context. Favorites keep
  the source session and turn; chat comment anchors retain the source session key.

## Persistence and recovery

`studio.sqlite3` contains `router_dispatches`, keyed by controller session and
controller turn. It stores delivery metadata and the target turn link, not a
duplicate of complete target histories. Completed replies are loaded when their
Router cards are displayed. Existing running targets are observed, never sent an
automatic continuation prompt.

Duplicate completion notifications cannot dispatch twice. After a restart, an
interrupted routing/delivery record without a confirmed target turn is marked
uncertain and is never replayed automatically. Check the target before resending.
Saving metadata and submitting to an external backend cannot form one atomic
transaction; this explicit uncertain state is intentional.

Old Router turns created before delivery-link persistence cannot reliably recover
their exact target turn. Their existing “Open session” action remains available.

## Validation

- `npm run typecheck` and `npm test`
- `cargo test --workspace --locked`
- `scripts/smoke-remote-studio.mjs`, including browser Router acceptance with fake
  transports: inline response, explicit target, exact prompt, source tools,
  persisted completion, refresh recovery, uncertain delivery and deduplication.

The browser fixture makes no paid model requests and does not edit user settings.
It verifies UI/dispatch behavior, not model routing accuracy on real requests.
