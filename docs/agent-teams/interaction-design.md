# Agent Teams interaction design

This replaces the initial single-page implementation. It follows the project
[interaction guidelines](../design/interaction-guidelines.md), the Settings
navigation and the existing [typography standard](../typography.md).

| Level | User intent | Surface |
| --- | --- | --- |
| Team picker | Choose which collaboration to work on | Shared right workspace group list; active by default, archived opt-in |
| Tasks | See what needs action and delegate | Default group section; status filters and compact task rows |
| Members | Understand responsibility and open a session | Member cards; optional orchestrator and editing belong here |
| Activity | Understand waiting or delivery problems | Pending delivery/holds, readable member labels, receipt details on demand |
| Group settings | Rename, pause, archive or delete | Explicit lifecycle actions; no global MCP config mixed in |
| Task detail | Read requirements, answer, verify | Summary / Discussion / Results; Recovery on a separate page |
| Collaboration settings | Enable feature, install/remove tools | Separate destination from the group list |

The main-menu entry activates Studio’s existing shared right workspace. The first
view lists groups; selecting a group enters its sections and selecting a task
enters its details. No management modal opens on entry. The existing right-rail
header, close button, resizer and peer-rail exclusivity remain in effect.
Back returns to the same group/filter/scroll. Draft edits survive refresh and failed
saves. Session links leave the Teams rail and select the existing session. Creation,
editing, review and deletion confirmation use focused dialogs. On narrow screens
the rail follows the existing overlay behavior; sections wrap without horizontal
document scrolling.

Deletion is separate from archive. Empty groups can be deleted directly after
confirmation. For other groups, completed/cancelled tasks and their visible history
are removed with the group; original sessions/files stay intact. The host rejects
unfinished tasks, active/unknown claims, pending native delivery or held dispatch.
The confirmation explains blockers and links to Tasks/Activity. It never cancels
or interrupts work implicitly. Native delivery and idempotency receipts remain to
prevent replay; deletion is not a purge of all operational records. A stale page
cannot delete a changed group, and repeating the same confirmed request is safe.

## Acceptance

The implemented flow was exercised in the real application with synthetic groups.
The initial single-page-modal screenshot is superseded. [Validation manifest](ui-redesign-validation.json)
records 23 integrated checks, including confirmation cancellation, deletion with
original sessions retained, invalid/dirty draft retention, peer-rail exclusivity
and restoration, both UI languages/themes and font size/weight extremes.

## Group creation and joining

Create group asks only for a name and returns to the group list. The new group is
empty and paused. Each non-archived row below the five-member limit exposes **Add
member** directly. That dialog selects exactly one existing session, its name and
responsibility. An optional checkbox assigns that member as orchestrator/owner;
these are two names for the same single slot, not separate roles. If already set,
the checkbox is disabled and the current owner is identified. Users may explicitly
replace/clear the owner later in Members. Joining and assigning ownership commit
atomically with the expected group revision. Concurrent/stale requests cannot
create two owners or partially add a member on an ownership conflict.

Empty and single-member groups remain paused; at least two members are required
to resume collaboration. Adding members does not implicitly resume a paused group.
Removing the owner clears that slot; removing all members returns to an empty group.
