# Studio comment interaction policy

The user requested no `View comment` hover chooser and one comment per owned
selection. Studio keeps direct underline-click editing; native double-click,
drag and modified clicks remain text gestures. New selections matching an
existing owned range open that saved comment for editing instead of appending a
new ID. Save also checks for a duplicate introduced while the dialog was open,
retaining the unsaved text and displaying an error rather than overwriting a note.

Identity is scoped to the current session's comment collection and uses provider,
file/root, document revision and rendered textRange or exact typed source/PDF/EPUB
coordinates. Reader session IDs are transient and excluded. Old file references
need a content hash and complete offsets; chat needs turn/item and complete
offsets. Equal quotes at different positions, changed revisions, approximate
line-only references, transient SQL snapshots, and unproven cross-view mappings
are not merged. Existing records are never automatically deleted. Clicking a
legacy duplicate or distinct overlapping marks opens the host Comments list.

The immutable MarkNoria SDK stays at `18d122e`. Its public presentation API has
no hover-chooser switch yet. `src-tauri/build/comment_policy.rs` temporarily
derives only `comment-marks.mjs`, guarded by revision, source SHA-256, exact
replacement counts and output SHA-256. The two changes remove hover scheduling
and route multiple-hit clicks to the existing host-list event. Pointer/selection
handling, rendering and budgets remain upstream. Both Studio asset routes use
the same stateless adapter, after existing origin/lease/path/method authorization;
no child DOM mutation or document/file endpoint is added.

This is Studio-requested interaction policy, not a revival of the retired #38/#39
bug fixes. Track [Studio #88](https://github.com/messon007/codex-thread-studio/issues/88)
and the revised [MarkNoria #42](https://github.com/messon007/marknoria/issues/42).
Remove the derived module, generator and asset override when a verified fixed SDK
exposes equivalent public configuration and direct-click/list routing. Retain
identity tests and full Studio Chromium/native WebKit regressions for no popup,
direct edit, same-selection reuse, persistence, exact source return and old
overlap access. An SDK upgrade must fail closed until this adapter is reviewed.
