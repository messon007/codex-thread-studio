# Studio interaction and interface guidelines

## Scope and maintenance

Project-wide baseline consolidated on 2026-11-19 from locally accessible project
conversations spanning July–November 2026: initial Studio design, workspace tools,
Settings, readers, Router, Session Map, native UI reviews and Agent Teams.
This is a record of user requirements and review criteria, not a claim that every
current surface already complies. Historical conversations are evidence, not new
authorization to implement unrelated work.

Apply these rules to new and changed surfaces. Current explicit user instructions
take precedence; update superseded decisions here and in the affected feature
specification. Do not redesign unrelated screens merely to enforce this document.
The [typography standard](../typography.md) owns exact font rules; feature documents
own detailed state and navigation contracts. Do not duplicate their numeric tokens.

The overall description "flat, restrained, moderately compact, reading-oriented
desktop workspace" is a synthesis of repeated feedback. It does not prescribe a
new palette, font, radius or theme. Reuse the application's established components.

## 1. Design the journey before the controls

- Identify entry point, object, primary action, next step, return path and failure
  recovery before implementation. A backend operation is not a navigation category.
- Separate lists, details, editing and settings. Do not show every capability,
  explanation and recovery control on the first screen.
- Use focused forms for one task or object. Complete the necessary lifecycle:
  creation must have usable viewing, editing and removal paths where applicable.
- Reduce manual handoffs and repeated data entry. Optional features must not force
  setup or permanent controls into unrelated workflows.

## 2. Preserve the workspace structure

| Area | Contract |
| --- | --- |
| Left | Collapsible session list; unified backend presentation with compact CX/OC-style identifiers rather than separate backend navigation systems. |
| Center | Conversation and Composer remain the primary workspace; readers and tools do not replace or cover the chat. |
| Right | Shared workspace for documents, browser, Files, Terminal, Git Review, Comments, Favorites, resources, Map and management views. |

Right-side tools use the established mutual exclusion, header, close action,
divider and resize behavior. Share the width across tools: opening a document
from Files must not change it. Respect usable width limits and keep controls
reachable in narrow windows. Resizing must not accidentally select surrounding
text. Use focused dialogs for creation/editing and the established Settings
surface for global configuration; do not add another independent rail.

## 3. Give content space and controls hierarchy

- Prefer flat grouping over nested bordered cards. Avoid redundant outer panels,
  oversized menus, repeated headings and large gaps between related controls.
- Keep reader tools in the first header row at normal widths. Group actions by
  purpose and place related actions together; e.g. table Chart/Copy/Wrap controls.
- Compact controls must not hide meaningful content. SQL may need multiple lines;
  offer a compact presentation without making the query unreadable.
- A document TOC may overlay the reader rather than permanently narrow the text;
  it should be wide enough to read and allow explicit dismissal after navigation.
- Distinguish primary content, secondary metadata and on-demand details. Avoid
  internal IDs, transport/protocol explanations and duplicate model/status labels
  in the ordinary user flow.

## 4. Reuse one visual and interaction language

Use existing menu, dialog, field, button, icon, focus and theme components/tokens.
Equivalent actions retain their geometry, placement and behavior across tools,
backends and reader modes. Match existing search fields and candidate interactions.
Small and large documents should retain familiar tool placement.

Reference designs such as Apple, Codex App and GNOME can inform a design, but must
not introduce an unrelated style into Studio. Native-branch experiments do not
change the main Web desktop product's layout or feature scope.

Alignment is judged from the rendered result:

- Align titlebars, dividers, content edges, table columns and action rows.
- Center glyphs within buttons, not merely their SVG boxes or text line boxes.
- Align adjacent icons and labels according to their content: vertically center
  single-line controls; top-align multi-line titles and associated badges where
  appropriate. There is no universal "center everything" rule.
- Keep user and assistant text left-aligned to the same content edge. Background
  padding must not push one message type off that edge.
- Align progress/error icons and expand arrows with the conversation's icon and
  text columns. Use shared SVG icons when character arrows do not align reliably.

## 5. Keep typography clear and configurable

Follow [typography.md](../typography.md): Interface for controls/metadata,
Reading Content for messages/documents/readable activity, Code for actual code,
source and terminal output. Technical subject matter alone does not imply Code.
Derive secondary sizes and emphasis from the configured profile; do not bypass
settings with arbitrary small, faint or bold text. Apply configuration consistently
in both languages and themes, including menus, counts and secondary panels.

The user prioritizes clear text over thin/light styling. No single font family or
fixed weight was accepted as a universal preference; an isolated approval of a
particular weight is not a new global default.

## 6. Put actions where they are useful

- Common object actions belong near their object. Low-frequency actions belong
  in established More (`…`), Composer `+`, or Local Workspace menus as appropriate.
- Use consistent compact icons for recognizable actions, with accessible names,
  tooltips and keyboard access. Retain text where an icon would be ambiguous.
- Avoid duplicate entry points, repeated Copy buttons, and multiple equivalent
  Close controls within a dialog. Remove redundancy without removing capability.
- Popovers should follow shared open/dismiss behavior; do not add a separate close
  control when the existing trigger and dismissal pattern already suffice.
- Use concise, behaviorally accurate labels: Query, Continue, and
  `Message {backend}...`. Do not call a one-shot action Enable if that implies a
  persistent setting. Put short prerequisite/effect hints next to their field.

## 7. Use compact tables for comparable objects

Prefer semantic tables for objects with common fields. Separate name, count,
status and actions; align headers and cells. Allow sufficient width for short
labels and counts to stay on one line. Keep action icons consistently sized and
spaced. Let long descriptive content wrap without misaligning the action column.

Simple status changes can be performed directly in the status column when their
effects are clear. Avoid routing every small change through a full edit dialog.
Counts should use a consistent nearby treatment, not ad hoc punctuation or a
separate stack of statistics cards. Keep horizontal overflow local when needed.

## 8. Preserve context through transitions

- Restore each session's relevant right-side tool/content when returning to it.
  Preserve Composer drafts across session switches.
- Preserve shared workspace width, reading/scroll position, selection and focus
  where the transition permits. Opening a link must not jump the central chat.
- Table paging and background refresh must not steal focus or reset position.
- Keep form input on failed saves or unrelated updates; show actionable errors.
- Preview/Source navigation should use source mapping to retain the reading
  location, with a nearest-heading fallback where supported.
- Document navigation needs a meaningful return destination. Follow the bounded
  history and new-entry rules in [document-link-navigation.md](../document-link-navigation.md).

## 9. Make state and feedback trustworthy

Send/Continue/Stop/Queue availability must match the real session/turn state;
offscreen sessions must also update. Attention ordering and status indicators
must help the user find current work, not require reopening sessions to refresh.

Separate intermediate activity from the final response. Use the established
Activity presentation for intermediate results and display the final answer
directly. Explicit user command output should be readily visible.

Aggregate unseen completion reminders near the Composer, aligned with its content
edge. Provide navigation to the original response and a return path. Distinguish
the conditions for adding an unseen-result reminder from explicit acknowledgement
that clears it; do not discard reminders merely because a layout changed. Follow
the [Router specification](../router-inline-workspace.md) for detailed behavior.

## 10. Treat readers and responsiveness as complete interactions

Opening, scrolling, searching, navigating, selecting and commenting form one
reading workflow. Wheel scrolling must work immediately after opening a file.
Comment markers must lead to editing; comment lists must locate the source.
Tables need readable cells, adjustable widths, wrapping, column selection and
paging as appropriate. SQL can unify advanced CSV selection/filtering/sorting
instead of adding a growing set of separate controls.

Large files may have explicit capability limits, but must not freeze chat,
document closing, resizing or ordinary input. Judge performance through actual
interaction as well as loading time. Shared Web reader behavior follows the
[MarkNoria maintenance plan](../marknoria-maintenance.md); Studio owns its host
integration and approved menu customizations.

## 11. Reduce manual work without interrupting it

Router should expose the destination and offer assisted selection, without
requiring full session names or constant session switching. Session Map should
favor AI generation/maintenance over manual forms. Keep the user's control over
targets and actions. Background collaboration queues work for busy sessions;
waiting on another party must not prevent unrelated work.

## Scoped decisions and superseded alternatives

These are feature-specific constraints, not global bans. Agent Teams decisions
below describe the approved feature-branch design, not availability in main or
authorization to merge it.

| Area | Accepted scope |
| --- | --- |
| Document history | At most four return steps with position restoration; a new document opened from chat starts a fresh navigation chain. |
| Reader comments | One comment per passage; direct edit and source navigation remain. Hide the separate View comment menu entry. |
| Favorites | Omit the specified upstream reader selection-menu Favorite entry. Studio session/global favorites remain valid. |
| Compare | Exclude standalone side-by-side reader Compare from Studio. Git Review/diff remains in scope. |
| Archives | No Archive group action. Session archive/restore remains supported with its low-frequency entry in Local Workspace. |
| Agent Teams | Shared right-workspace group list; create group before adding members, one member per add dialog, at most five members. Optional single orchestrator, default none, selected in Edit group. |
| Group controls | View uses an eye SVG in Actions rather than a chevron after the name; Edit also belongs in Actions. Collaboration has its own status column with a direct toggle. Group count follows the Members count style; the temporary footer-count proposal was superseded. |
| Group task entry | Use the orchestrator's existing chat; omit the separate Tasks UI and redundant Set up collaboration entry. Role/enable changes take effect through the next ordinary message as specified by the feature. |
| Session Map | Prefer AI generation/update; simplify node actions to current/completed, with only one current node. Do not infer that all navigation should use a map. |

## Design review and acceptance

For substantial new or redesigned interactions, show a representative layout or
prototype for user review before implementation unless the current task already
approves it. Incorporate a requested design reviewer when available; review does
not replace the user's visual acceptance. Routine consistency fixes do not need
a new approval ceremony.

Use realistic synthetic fixtures: long titles, multi-line messages, tables, empty
and populated lists, busy/disabled/error states. Inspect actual rendered screens
and exercise the relevant journeys, including return navigation, failed edits
and deletion/cancellation where applicable. Check narrow widths, both themes and
languages, configured font sizes/weights, keyboard behavior and resize/scroll
responsiveness. Reuse existing checks; scale verification to the change.

Code review and passing source/unit tests alone do not prove visual correctness.
Report what was actually inspected and any unverified platform behavior. Do not
commit private session captures; follow the [design asset policy](README.md).

Keep future decisions reviewable: update this baseline for durable conventions,
record exceptions and exact behavior in feature documents, and link them rather
than creating competing preference files. Do not retain rejected alternatives as
equally valid rules.
