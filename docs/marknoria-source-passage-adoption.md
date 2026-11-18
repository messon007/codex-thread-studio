# Source and saved-passage SDK adoption

Studio adopts MarkNoria `18d122e5ea37cfc754b2d79b58d91334176c87e3` from a clean,
fixed public export, including #38/#39 layout/comment fixes and #40/#41 embedded
Source and exact passage navigation. The archive has 866 inventory entries;
`scripts/marknoria-release.json` pins its archive and tree hashes. Ordinary builds
need neither private credentials nor a sibling checkout. Dependency versions and
the 97-language reduced syntax profile are unchanged.

Source defaults now match the shared Web reader: eligible files up to 128 KiB,
with each table at most 50 rows / 512 cells, use one bounded continuous buffer.
Other files retain 200-line / 128-KiB windows; line jumps retain up to 24 preceding
lines within that budget. Long Unicode lines continue without dropping bytes.

New Studio comments retain the public state's stable resource ID after verifying
the captured selection's document ID and revision. The existing persisted typed
reference keeps this ID through edit/save/reload. Exact supported references call
`navigatePassage`, then focus the revealed iframe and consume `focusPassage` only
after navigation succeeds and the shell has removed its loading cover. The shared
reader validates the fingerprint/range, preserves format precision, and waits for
focus rendering before confirming selection. A fingerprint, revision or ownership
failure is surfaced; it never silently falls back to a coarse target.

Old comments without a stable resource ID retain their existing typed-location
navigation across sessions. An original session ID is usable for exact navigation
only in that same reader session. No IDs are fabricated and no quote search is used.
Coarse references still use `navigate`; query snapshots retain the existing Studio
snapshot flow. This does not promise relocation across changed file revisions.

Studio continues to own comment storage/editor/AI integration, Copy/Comment/
Translate actions, file authorization and the four-previous-visit Back history.
No Notes/Favorite entry or standalone Compare surface is enabled. Native desktop
UI/FFI and the paused Studio native branch are not part of this import.

The #38/#39 temporary build generators and both asset overrides are removed.
Both authorized document routes and `/reader-assets` use the upstream stateless
asset router; origin, method, lease and path checks remain in place. The full-app
geometry and comment regressions are retained and now verify upstream bytes.

Validation is recorded with the adopting PR and build provenance. Focused tests
cover resource identity persistence, legacy compatibility, refusal of mismatched
resources, no coarse fallback on exact failures, and state-bound selection capture.
The runtime gate includes full Studio Chromium/WebKit comments and exact reopen,
layout, first-wheel and four-entry Back regressions, plus independent SDK checks.
