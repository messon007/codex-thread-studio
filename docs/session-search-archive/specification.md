# Session Search and Archive Center

Status: approved and implemented.

## Goals

- Find content inside the currently selected session without taking over the shared right workspace.
- Make archived Codex-compatible sessions discoverable, readable, restorable, and deletable.
- Preserve the current compact toolbar, session grouping, shared rail width, and narrow-window behavior.

## 1. Search within the current session

### Entry points

- Add one 32 x 32 search icon as the first item in the middle session toolbar.
- `Ctrl+F` / `Cmd+F` opens the same search surface when focus belongs to the chat workspace.
- When focus belongs to a document, PDF, editor, or browser surface, that surface keeps ownership of its existing find behavior.

### Layout

- Opening search inserts a 46 px search strip immediately below the middle session header.
- The strip is bounded by the middle workspace only. It does not open, close, resize, or replace the shared right workspace.
- The input is followed by a result count, previous/next buttons, an optional result-type filter, and close.
- While typing, a floating result list opens below the input. It overlays the transcript and therefore does not change message width or scroll position.

### Result model

- Search user messages, final assistant responses, persisted progress messages, and tool/activity summaries.
- Each result shows source type, turn position, and a short highlighted excerpt.
- Clicking a result closes the floating list, keeps the search strip open, jumps to the item, and highlights the active occurrence.
- Previous/next continues navigation without reopening the list.
- Empty queries show recent search terms only if such history is introduced later; the first implementation can show no popup.

### Keyboard behavior

- `Enter`: open the selected match.
- `Shift+Enter`: previous match.
- `Up` / `Down`: move through the open result list.
- `Escape`: close result list first, then close the search strip.

## 2. Archived sessions

### Entry point

- Add `Archived sessions` to the existing `Local Workspace` menu, directly below `Global favorites`.
- The entry carries a small archived count badge and otherwise uses exactly the same plain row layout, height, typography, hover state, and alignment as `Global favorites` and `Browser`.
- Do not add archive controls or counts to the normal session filter row.

### Archived list

- Entering Archived switches the sidebar into a dedicated library mode with a compact `Back to sessions` action and an `Archived sessions` title.
- The normal All, Active, and Attention filters are hidden in this mode.
- The sidebar search placeholder changes to `Search archived sessions`.
- Archived sessions remain grouped by working directory and show backend, title, directory, and archived date.
- Archived sessions are fetched lazily only after the workspace-menu entry is opened.
- Backends without archive support contribute no items and do not create disabled UI.

### Read-only preview

- Selecting an archived session renders its transcript in the normal middle workspace.
- Do not add an archived-status banner below the session header; the transcript starts at the normal header boundary.
- The composer is replaced by a quiet read-only footer that states the archived status and contains the only `Restore` action. Opening an archived session never restores it implicitly.
- Comments, favorites, and resources remain readable. Mutating session actions are limited to Restore and Delete.

### Restore and delete

- Restore removes the item from Archived, switches back to All, selects the restored session, and re-enables the composer.
- Delete always requires the existing destructive confirmation pattern.
- Archiving the current live session returns the sidebar to the nearest remaining active session and updates the Archived count.

## 3. Loading and scale

- Search should use backend occurrence search when available and fall back to the loaded session model only for adapters that lack it.
- Do not hydrate every session to implement current-session search.
- Archived listing should use cursor pagination and load more near the end of the list.
- Search requests are debounced and stale responses are discarded by query generation.

## 4. Visual rules

- Reuse existing 32 x 32 toolbar icons, search fields, 8 px radius, border colors, and focus rings.
- Search suggestions reuse the existing composer suggestion-menu surface and option states: shared padding, border, shadow, selected background, primary label, secondary excerpt, and keyboard selection behavior.
- Do not add a separator before the search, comments, or favorites icons.
- Search uses the current brand green; text matches use the existing amber highlight token.
- Archived state uses neutral slate styling rather than warning or error colors.
- Header and search-strip divider lines must align with the shared right workspace header boundaries.

## 5. Acceptance criteria

- Opening search never changes the width or open state of Files, Terminal, Git Review, Browser, Comments, Favorites, Resources, or Map.
- Search can jump to matches in user, assistant, progress, and activity content.
- Archived sessions are absent from All and present in Archived without restarting the app.
- An archived session cannot send a message until explicitly restored.
- Restoring a session makes it immediately available for normal conversation.
- `Archived sessions` remains reachable from the `Local Workspace` menu when the application is approximately half-screen width.
