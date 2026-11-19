# Agent Teams interaction design

Current design: the table prototype reviewed by GPT-6.2-Astra and accepted by the
user with “基本可用，你先照着开发吧”. This replaces the earlier card/section layout.
Follow [Studio interaction guidelines](../design/interaction-guidelines.md) and
[typography](../typography.md). Historical UI review manifests remain historical.

## Daily flow

The main menu opens Studio's existing shared right rail. Reuse its title bar for
Back, current group name, group menu and Close; do not repeat that heading in the
body. The same resizer and peer-tool exclusivity remain in effect. All Back
entries (members, task list/detail, Activity and settings) use the same centered
SVG path and button geometry. A zero-size inline marker locates the real text
baseline; browser font glyph metrics place the arrow at the visible title
glyph center. Line-box centering alone is insufficient for asymmetric fonts.
Measurements are batched in one animation frame and refreshed on heading resize,
font loading, title text mutations and interface font changes; the 32px button
hit area stays fixed. For ellipsized names, measure only the visible grapheme
prefix plus ellipsis, using DOM Range widths. Acceptance compares actual painted
pixels from screenshots, not line rectangles or the production metric formula.

- Group list: a compact table with Group name, Member count, Status and Actions
  columns. Group names are plain text. The eye icon in Actions opens members; the
  adjacent pencil opens Edit group and the add-member icon opens a single-member
  form. Status contains a directly operable, labelled switch (Archived remains
  read-only). Enablement requires two members and global Teams enablement; disabled
  controls explain why through hover/accessibility help. Switching only changes
  paused state using the current revision and existing host checks; it never
  starts or interrupts a turn. Wait for acknowledgement before changing the switch;
  on error refresh authoritative state without automatic resend. Keep focus and
  horizontal table position after updates. Inline requests have independent per-group
  pending state; their receipts never close or lock a subsequently opened editor,
  and their errors stay in the panel rather than an unrelated form. Headers/cells,
  counts and all three action icons align; the name column uses remaining space. Member count
  has a wider, font-responsive column and stays on one line. The name retains at
  least eight compact-font ems plus padding; if four columns cannot fit, only the
  focusable table region scrolls horizontally, without clipping actions or
  expanding the surrounding rail. The toolbar shows
  Groups N in the same label/number styling as Members N/5, without a middle dot
  or a table footer. It reports all groups even during search. Create group is
  the page's primary action.
  Collaboration/tool setup is in the header menu, not a duplicate list shortcut.
  Search appears when there are more than seven groups or an active search.
- Group home: immediately show the Members / Responsibility / Actions table.
  The toolbar contains member count and Add member. The optional Orchestrator
  label beneath a member name opens that existing session's chat. Pencil edits
  this member; the adjacent Studio more menu contains Open session and Remove
  member. Both icons have accessible labels and hover titles.
- There is no Tasks entry or manual task-dispatch form. Users give requests in
  the orchestrator's normal chat. The group menu contains Activity, Edit group
  and Delete group. Activity retains current work, blocked/recovery actions and
  collapsed completed history; task details return to Activity.
- Edit group is one compact dialog with name, optional Orchestrator dropdown
  (default none), and Enable collaboration checkbox. Save applies these together.
  Help sits directly beneath the field it explains: optional role and chat entry
  below Orchestrator, and the current activation requirement/effect below the
  enable checkbox. Empty conflict containers take no space.
  Create/add/edit/review/delete dialogs use Cancel and one primary action.
  Display name and responsibility are group-specific; native session identity,
  working directory, files and history remain unchanged.

## Membership and lifecycle

Create group asks only for a name. Add member binds exactly one existing Codex
session and its responsibility; it has no role selector. A group has at most five
members and one optional orchestrator. Choose, replace or clear that role only
in Edit group. Peer collaboration remains available without an orchestrator.
Empty or single-member groups remain disabled; adding members does not enable
a group automatically. Successful addition opens the roster and focuses the
new member's edit action; cancellation stays at the origin.

Enabling a group does not send a message or interrupt an existing turn. The next
ordinary user message sent from the orchestrator chat (including an ordinary
queued message when it is actually sent) receives host-labelled role context.
Disabling, replacing or removing that owner retires its previous role with the
next ordinary user message. Auxiliary sends and active-turn Steer do not consume
this update. Changes before that message coalesce to the latest effective state.
See [chat coordination](chat-coordination.md) for receipt and restart semantics.

There is no Archive group action or Archived groups navigation. Persisted archived
data from earlier development remains visible in the ordinary group list, marked
Archived and read-only, with deletion still available; no migration destroys or
reactivates that data. Legacy storage/domain compatibility is retained.

Removal confirmation uses the existing host preview from the actual Rust removal
predicates: requester/assignee/question references, unsettled execution/review
receipts, uncertain ordinary submissions and exact-session deliveries/reservations. It explains owner clearing and
automatic pause below two members. The host rechecks on commit. Blocker links return
to the original member and scroll position. A race refreshes the same confirmation;
network failure remains visible, and a lost response reconciles current membership
without an automatic resend. The original session is retained.

Group deletion requires revision checks, settled
native work and explicit confirmation. Sessions/files and anti-replay receipts
survive. It never cancels active work implicitly.

## Interaction and acceptance

Tables use native thead/th/tbody cells, fixed count/action columns, wrapping names
and responsibilities, Studio tokens and no independent visual theme. Menus share
Studio's action-menu classes and host dismissal. Keyboard navigation, outside click,
Escape, viewport placement and stable focus survive background snapshots.

Concurrent member edits preserve drafts and reload the latest revision. Changed
saved values require explicit use-latest or keep-mine resolution before saving.
No roster/list redesign may remove these existing recovery paths.

Validate the real Studio table, not only the prototype: alignment of every header
and cell edge, first-screen visibility of the first two members at narrow widths,
icon hit targets and accessible labels, both themes/languages and font extremes.
Exercise actual creation, editing, owner selection, blocked removal, conflict and
lost-response recovery, session navigation and return focus. Validation uses an
isolated synthetic model; it does not establish real-model planning quality.
