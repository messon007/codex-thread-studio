# Studio interaction and interface guidelines

Status: project-wide baseline, established 2026-11-19 from the existing Settings,
Session Search/Archive, Router and typography conventions. Apply to new and
changed surfaces; do not interpret this document as approval to redesign every
existing feature. [Typography](../typography.md) remains authoritative for fonts.

## Start with the user's task

Before coding, write down entry point, primary action, object being acted on,
next step, return path and failure recovery. A backend operation is not itself a
navigation category. Separate daily work from setup, history and diagnostics.
Each view needs one purpose and one visually dominant action. Show labels people
recognize (member names, task titles); IDs and transport details belong in details.

## Fit the existing application

Keep features in their established workspace or main-menu entry. Management entry points should reuse the shared right workspace for object lists
and navigation, with its existing header, close button, width and resizer. Use
focused dialogs for creation, editing and confirmation. Global settings may use
the existing Settings-style dialog. Follow existing mutual exclusion when switching right-side tools. Do not add a
second adjacent rail, independent resize state, or overlay unrelated controls on
top of Files/the reader. Opening a referenced
session leaves the management surface and selects that existing session without
closing another tool restored by that session.

Menus must reuse Studio's `.action-menu` / `.thread-action-menu`, icon-button
triggers, separators and danger styles, with shared host dismissal behavior.
Do not simulate menus with unrelated details elements or stacked bordered buttons.
Common object actions must be discoverable from their list. Member/person editing
uses one person's form, with an explicitly labelled edit entry and visible more
menu per row. Keep object focus stable during periodic refresh and after saves.

Use existing `.dialog`, `.dialog-header`, `.primary-button`, `.subtle-button`,
`.field`, theme tokens, spacing and focus rings. All feature selectors must be
scoped. Do not inject an independent theme or inline stylesheet. Reading content
uses the Reading profile; technical labels/paths are still Interface, not Code.

## Hierarchy and navigation

- Object selection, object sections and object details are distinct levels.
- Persistent navigation shows the current selection. Use semantic navigation
  buttons/aria-current; reserve tabs with keyboard tab semantics for true tabs.
- Lists show title, meaningful status and one secondary line. Full forms, output,
  recovery controls and installation parameters never accompany every row.
- Details have a labelled Back action that restores selection, filter and scroll.
- Secondary/destructive actions belong in a labelled More actions menu or the
  object's settings. Low-frequency advanced recovery has its own destination.
- Archived objects are excluded by default and reachable explicitly. Empty lists
  explain what is missing and offer the next relevant action.

## Changes, destructive actions and feedback

Forms use explicit Save/Cancel, labels and inline actionable errors. Preserve
inputs on validation/network failure and while refreshing. Warn before leaving
an edited form, including Escape/close; confirmation cancellation retains focus
and drafts. Loading, submitting, empty and failed are distinct states. Automatic
refresh must not steal focus, reset a filter, or move the user's scroll position.

Deletion confirmation names the object and distinguishes it from archive. State
what is deleted, what survives, and whether it can be undone. Default focus goes
to Cancel; use a destructive action style. A deletion endpoint must revalidate
revision and live-work constraints; never infer safety from a disabled button.
If blocked, list the actual work and link to the task or recovery destination.
Never silently interrupt running agents or delete their sessions/files to remove
a group. Retained operational receipts must be disclosed rather than presented
as a complete data purge.

## Accessibility, responsive layout and acceptance

Keyboard: visible focus, logical order, Escape returns one level/handles dirty
forms before closing, and focus returns to the opener. Navigation must remain
usable at narrow widths; collapse the sidebar into a compact picker rather than
squeezing the content. Use min-height and wrapping, not clipped fixed-height text.
Status needs text, not only color. Respect light/dark themes and Interface bases
11, 14 and 20; translate user-facing labels in both supported languages.

Verify actual interaction paths in a browser: list → detail → back, setup separate
from daily work, empty and blocked states, create/edit/remove, confirmation cancel,
failed save, keyboard navigation, long names, narrow width, both themes/languages
and font extremes. Capture synthetic fixtures only. Unit/source checks alone do
not establish usable interaction. Record deviations and their rationale.
