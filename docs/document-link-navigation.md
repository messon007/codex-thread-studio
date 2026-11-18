# Linked-document Back navigation

[Studio #82](https://github.com/messon007/codex-thread-studio/issues/82) adds a
four-entry return history to the host document shell. The current document is
additional to those four entries. This is an in-memory visit history, not a
file-name set: repeated visits retain separate reading positions.

For `A → B → C → D → B → C → D → A`, Back returns `D, C, B, D`. The oldest entries
have been evicted; the next action returns to the original Files/Review/Resources
surface, or closes a document opened directly. The existing top-right button
uses its back arrow and a translated `Back to {file}` accessible label/tooltip.
Same-file anchors and external links do not add cross-file entries. Reload keeps
the chain. Opening another document from a conversation link, Files or another
host entry starts a new history. Closing the reader or switching sessions also
drops the previous chain. Cancelled dirty-document switches preserve it.

Each entry contains only root/path, source-session ownership, entry surface,
view and the reader's public position. New frames, grants and leases are created
through the normal file-opening path. No content buffers, frames or live reader
services are retained for history. Each departed service is released as before.

Position restoration uses the fixed SDK's public `state` and `navigate` protocol.
Preview/Source/Edit and supported format positions reuse the existing view setup.
Only a matching revision restores saved coordinates; changed documents open
without obsolete coordinates. This is source-location restoration, not a claim
of exact pixel-offset restoration. Local link fragments lose the URL `#` prefix
and are decoded before reaching the public fragment API.

The existing busy and unsaved-edit guards run before a back entry is consumed.
Cancelling the discard prompt preserves the current document and history. Async
link capture checks the originating document and selected session before opening
a new file. This is Studio host navigation; the SDK inventory and its temporary
compatibility assets are unchanged.

Validation: `scripts/smoke-document-back.mjs --webkit` uses an isolated offline
backend and synthetic Markdown, actual links and the production host controls.
Chromium covers multi-step Back, the cyclic four-entry limit, reading positions,
reload, dirty cancellation/confirmation, changed revisions, unrelated opens and
owned-cache cleanup. Native Wayland WebKit exercises the same cyclic return chain
and positions through the full app with DOM clicks. Unit tests cover revision
matching, view-before-location restoration, dirty cancellation and stale session
ownership. The paused platform-native branch and standalone Compare are excluded.
