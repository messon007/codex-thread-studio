# Agent Teams interaction design

Current design: the table prototype reviewed by GPT-6.2-Astra and accepted by the
user with “基本可用，你先照着开发吧”. This replaces the earlier card/section layout.
Follow [Studio interaction guidelines](../design/interaction-guidelines.md) and
[typography](../typography.md). Historical UI review manifests remain historical.

## Daily flow

The main menu opens Studio's existing shared right rail. Reuse its title bar for
Back, current group name, group menu and Close; do not repeat that heading in the
body. The same resizer and peer-tool exclusivity remain in effect.

- Group list: a compact table with Group name, Member count and Actions columns.
  Group name plus chevron opens its members. The row's add-member icon opens a
  single-member form. Counts and action icons align vertically; the name column
  uses remaining space. Create group is the page's primary action. Archived groups
  and collaboration/tool setup are in the header menu, not persistent extra cards.
  Search appears when there are more than seven groups or an active search.
- Group home: immediately show the Members / Responsibility / Actions table.
  The toolbar contains member count, Tasks and Add member. Each member's optional
  owner label appears only beneath their name. Pencil edits this member; the
  adjacent Studio more menu contains Open session and Remove member. Both icons
  have accessible labels and hover titles. No owner summary card or four-section
  navigation precedes the roster.
- Tasks remains an explicit toolbar entry, including when empty. A paused group
  explains the blocking condition and resume action here. Task detail returns to
  its task list; the task list returns to members. The group menu exposes owner
  selection, Activity, Rename, Pause/Resume, Archive and Delete directly.
- Creation, member editing and owner selection use compact dialogs with Cancel
  and one primary action. One backend is not shown as a redundant selector.
  Display name and responsibility are group-specific; native session identity,
  working directory, files and history remain unchanged.

## Membership and lifecycle

Create group asks only for a name. Add member binds exactly one existing Codex
session and its responsibility, optionally assigning the unique owner atomically.
A group has at most five members and at most one orchestrator/owner. A second owner
is rejected without adding a member. Explicit owner replacement/clearing is in the
group menu. Empty or single-member groups remain paused; adding members does not
automatically resume work. Successful addition opens the roster and focuses the
new member's edit action; cancellation stays at the origin. Archived rosters are
read-only and explain this visibly. Chinese uses “负责人” consistently for the role.

Removal confirmation uses the existing host preview from the actual Rust removal
predicates: requester/assignee/question references, unsettled execution/review
receipts and exact-session deliveries/reservations. It explains owner clearing and
automatic pause below two members. The host rechecks on commit. Blocker links return
to the original member and scroll position. A race refreshes the same confirmation;
network failure remains visible, and a lost response reconciles current membership
without an automatic resend. The original session is retained.

Group deletion remains distinct from archive and requires revision checks, settled
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
