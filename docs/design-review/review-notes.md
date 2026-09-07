# Chat / Router / Map design review — 2026-09-07

Status: UI proposal only. Repository hygiene changes were approved and applied;
no application interaction changes in this design revision.
See [SVG mockup](chat-map-controls.svg).

## Controls

- Put low-frequency supervision in Chat's existing Plus menu: “Supervise this
  task”. No permanent switch. Only show a status chip while enabled; clicking it
  offers Stop supervision. Support a one-shot pre-send choice and explicit
  post-send activation. Ordinary sessions show no redundant destination/task title.
  Router exposes individual dispatches in the Plus submenu (destination, send time,
  optional short snippet on demand), with independent start/stop controls. Consume
  pre-send intent only after acknowledgement; preserve it on rejection. Do not
  attach one shared supervision flag to all concurrent Router work.
  Router must identify the source task before enabling; never implicitly supervise
  every routed conversation. Disabling supervision does not interrupt the agent.
- Replace the two always-visible Router instruction lines with a compact target
  selector: Automatic / Send to [session]. Show recommendations inside the picker;
  ask for clarification only on an actual uncertain route.
- Keep session and file groups in the @ picker. Expand the existing paperclip into
  the unified attachment picker; do not duplicate file actions in Plus. Distinguish
  project file references from client-local uploads in a remote browser. Bind
  files to their project, show removable attachment chips, and
  revalidate file access if the destination changes. Automatic routing must not
  silently assume a filesystem root or grant a target access to another project.
- Put Map Update and Delete in the header; show Generate from conversation in the
  empty state. Confirm deletion. Remove Undo from the visible menu. Internal change
  history may remain for diagnostics. Update carries node IDs, hierarchy, states,
  current node, revision and unsynchronized conversation, returning incremental
  operations. Preserve explicit user completion and resolve revision conflicts.

## Gray session indicator investigation

Current indicator rendering uses catalog thread status directly (`renderThreadList`
in `ui/app.js`). Router target-history loading calls `ensureSessionModel`, which
reads history, hydrates a view model, merges native metadata into the catalog, and
caches the model. Reading history does not imply a native thread resume.

A read-only local reproduction using the actual hydration code produced:

```json
{"catalogStatus":"notLoaded","modelStatus":"idle","activeTurnId":null,"loadedTurns":1}
```

Thus a loaded, completed conversation can be rendered in the Router while its
catalog indicator remains gray. Metadata merge also does not itself refresh the
sidebar, so stale DOM is another condition to distinguish. These are demonstrated
code paths, not proof of the exact live incident.

The running desktop writes stdout/stderr to its launch terminal, not a log file;
today's lifecycle entries were not available from the user journal. The identified
session's most recent persisted native completion observed during this review was
2026-09-06 21:53:01 Asia/Shanghai. This does not establish the current WebView state.

Proposed indicator semantics: green steady = valid history loaded and idle;
green pulse = running; gray = not loaded or disconnected. Keep native readiness
separate and retain prepare-before-send. Do not resume all Router targets merely
to make the indicator green. Confirm live catalog/cache/DOM status before changing
the implementation. Diagnostic records should contain IDs/statuses, not message
contents, credentials or raw prompts.

## Public-repository hygiene (applied)

- Removed the obsolete patch and sanitized repository settings; live profile untouched.
- Moved six acceptance images to `docs/design/acceptance-2026-08-13/` and updated links.
- Removed three unreferenced legacy images showing personal paths and two duplicate
  search/archive PNG mockups; retained and sanitized their editable SVG originals.
- Removed root screenshots directory and added runtime credential/database ignore rules.
- Replaced personal paths in documentation/demos with fictional `/home/user` examples.
- Preserved Wayland prototypes: `src-tauri/src/bin/wayland-webview_prototype.rs`
  embeds both HTML files, so deleting them would break that optional build target.

Initial findings and rationale:

| Item | Finding | Proposed action |
| --- | --- | --- |
| Root workaround patch | Targets generated JS; `git apply --check` now fails | Remove obsolete patch; preserve rationale and regression tests in maintained sources |
| Root settings.json | Contains machine-specific directories, session IDs, and a globally shared temporary directory | Replace with a sanitized example; do not grant shared directories by default |
| screenshots/ | Nine tracked PNGs, about 1.1 MB; acceptance document links to several | Retain a small sanitized public demo set; move/remove obsolete acceptance captures and update links together |
| docs demos/mockups | Some contain personal paths or real session identifiers | Replace with fictional fixtures; consolidate obsolete proposals rather than blindly deleting docs |
| prototypes/ | Two tracked Wayland prototype HTML files | Check references before archiving/removing |
| ui/vendor/, generated ui/*.mjs | Runtime/build assets, not incidental garbage | Keep until build/packaging is deliberately changed |
| Ignore rules | Build output and logs covered; runtime database/credential patterns incomplete | Add narrow runtime-secret/database ignores while preserving intentional test fixtures |

One environment acceptance screenshot was visually inspected: it shows test values
and a saved secret *name*, not a visible credential value. This is not a full image
or Git-history secret audit. Removing tracked files in a future commit will not
remove them from old commits; confirmed exposed credentials require rotation and
a separately approved history-cleanup plan.
