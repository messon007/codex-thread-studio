# Session Map specification

Status: proposed  
Target: Codex Thread Studio  
Document version: 0.2  
Last updated: 2026-08-04

Normative words **MUST**, **SHOULD**, and **MAY** express required, recommended, and optional behavior.

## 1. Product definition

Session Map is a compact companion to a long conversation. It keeps the original goal, current location, remaining scope, and overall progress visible while the chat moves into details.

It MUST let a user answer, at a glance:

1. What am I trying to finish?
2. Where am I now?
3. What remains?
4. Where should the conversation go next?

Session Map is not a second task manager, a report generator, or a technical trace viewer. Conversation remains the primary workspace.

## 2. Design principles

1. **Quiet by default.** Show goal, location, structure, and progress. Hide setup, history, and uncommon actions until requested.
2. **Generic structure.** Built-in structures describe shape, not a business domain. The same structure can serve a book, package inventory, course, investigation, or codebase.
3. **Direct manipulation.** Select a node to focus it; expand it to inspect children; drag or use Move to reorganize it.
4. **No surprise messages.** Map interaction never sends a chat message automatically.
5. **Automatic but reversible.** Safe changes from a completed Turn apply quietly and offer Undo. Ambiguous or destructive changes are skipped.
6. **One model, many views.** Tree, outline, path, and board are views over the same items.
7. **Human completion.** The system may mark something visited or active. Done/understood/accepted remains a user action unless a deterministic completion rule exists.

## 3. Scope

### 3.1 V1

- One Map per backend + Thread.
- A persistent goal with optional definition of done.
- Ordered, nested items with state, progress, notes, and optional dates.
- A current item and breadcrumb.
- Abstract built-in structures: Hierarchy, Path, Flow, and Blank.
- Compatible tree, outline, path, checklist, and board views.
- Automatic synchronization after a completed Turn.
- One-step Undo plus an unobtrusive local change history.
- User-defined structures through a visual builder or declarative JSON.
- Versioned templates, import/export, validation, and migration preview.
- Local SQLite persistence and offline operation.

### 3.2 Deferred

- Arbitrary JavaScript renderers or executable template plugins.
- Cross-Thread global maps.
- Collaborative editing.
- Visible autonomous messages or provider-history Turns triggered by Map changes. Initial generation and explicit AI repair may use one hidden reusable ephemeral worker per Map.
- Rewriting provider-owned chat history.

## 4. Vocabulary

| Term | Meaning |
|---|---|
| Map | The whole navigation workspace attached to a Thread |
| Structure | Reusable behavior preset: Hierarchy, Path, Flow, or custom |
| Item | A generic unit such as a chapter, package, concept, feature, or step |
| Group | An Item that contains other Items |
| State | Not started, Active, Visited, Done, Paused, or a custom equivalent |
| Current item | The Item the conversation is presently focused on |
| View | A visual projection such as tree, outline, path, checklist, or board |
| Sync | Updating the Map from the latest completed conversation Turn |

“Template” remains the technical term for a saved Structure definition. Product UI SHOULD use the shorter word “Structure.”

## 5. Built-in structures

Built-in names MUST remain domain-neutral and short.

| Structure | Shape | Typical use, not part of its name | Default views |
|---|---|---|---|
| Hierarchy | parent → children | chapters, packages, concepts, components | Tree, Outline |
| Path | ordered stages and current position | reading, onboarding, investigation, migration | Path, Checklist |
| Flow | items moving through states | development, review, research, content | Board, Tree |
| Blank | user chooses capabilities | unusual or mixed scenarios | Outline |

A Map can later enable another compatible view without converting or copying its Items.

## 6. Data model

```text
Thread
└─ Map
   ├─ goal
   ├─ definitionOfDone?
   ├─ structureId + pinnedVersion
   ├─ items[]
   │  ├─ parentId?
   │  ├─ order
   │  ├─ title
   │  ├─ state
   │  ├─ progress?
   │  ├─ note?
   │  └─ properties{}
   ├─ currentItemId?
   ├─ views[]
   └─ changeHistory[]
```

Required stored entities:

| Entity | Required fields |
|---|---|
| `map_workspace` | `id`, `backend`, `threadId`, `goal`, `structureId`, `structureVersion`, `revision` |
| `map_item` | `id`, `workspaceId`, `parentId?`, `type`, `title`, `state`, `sortKey`, `properties` |
| `map_relation` | `id`, `fromItemId`, `toItemId`, `type` |
| `map_view` | `id`, `renderer`, `config`, `isDefault` |
| `map_change` | `id`, `workspaceId`, `baseRevision`, `revision`, `source`, `operations`, `inverseOperations`, `createdAt` |
| `template_snapshot` | `templateId`, `version`, `manifest` |

Containment MUST remain a tree. Optional relations MAY form a graph without changing breadcrumbs. IDs are stable and opaque; titles are never identity.

## 7. State model

The platform semantics are deliberately small:

- `notStarted`: known but untouched;
- `active`: current work;
- `visited`: discussed or inspected;
- `done`: explicitly completed or satisfied by a deterministic rule;
- `paused`: intentionally set aside.

Custom structures MAY rename or hide states, but MUST map them to these semantics. The default progress is completed in-scope leaf weight divided by total leaf weight. A visited Item does not count as done unless the structure explicitly says so.

## 8. Interface

### 8.1 Optional entry

A Thread without a Map keeps Studio's existing layout unchanged. It has no goal row, empty rail, onboarding card, or placeholder. Map creation lives under the Thread More menu.

After a Map exists, its goal, definition of done, current path, and compact progress live in the trailing rail. Closing the rail returns to the unmodified conversation layout; Thread More reopens it.

Goal source follows an explicit priority: use the active provider `/goal` text when present; otherwise create an editable short-title suggestion from the Thread's opening message when the user creates a Map; otherwise ask for a goal. A generated suggestion MUST NOT silently become final. The short title and definition of done are separate fields, so a title remains scannable while completion details remain available on demand.

### 8.2 Map rail

On wide screens, Map is a resizable trailing rail around 320–380 px. On narrow screens it becomes an overlay or full-screen sheet.

The rail contains only:

1. `Map` title;
2. Add, View, and More icon buttons;
3. a thin progress summary;
4. current breadcrumb;
5. the selected view;
6. a small sync/undo status at the bottom.

Low-frequency commands live under More: Structure, Sync, History, Export, and Close Map. Icon buttons MUST have tooltips and accessible labels. Repeated explanatory text, uppercase section labels, badges, and permanent action bars SHOULD be avoided.

### 8.3 Items

An Item row displays state, title, and optional compact progress/count. Selecting an Item sets it as current. It does not send anything. Repeated labels such as “Current,” “Visited,” and “Paused” SHOULD be omitted from rows; state uses color plus a distinct shape/icon and exposes a tooltip and accessible name. Color MUST NOT be the only state signal.

Selection MUST NOT insert an action bar between rows because that shifts the tree and interrupts hierarchy scanning. On hover or selection, the row exposes one trailing More button. Its anchored context menu contains compact local actions for current position, state, adding a child, editing, and removal. Conversation-draft actions are deliberately absent from the experimental rail.

### 8.4 Structure picker

Structure selection appears only when creating a Map or choosing More → Structure. It uses four visual cards: Hierarchy, Path, Flow, Blank. Domain examples appear as quiet supporting text, never as structure names.

## 9. Interaction contract

| Action | Map result | Composer result | Automatic send |
|---|---|---|---|
| Select Item | Changes current Item | Map rail shows the new path | Never |
| Expand, collapse, change view | Presentation only | None | Never |
| Add, rename, move, note | Local edit | None | Never |
| Change state | Local edit | None | Never |
| Delete/merge/change goal | Confirms and applies local edit | None | Never |
| Send composer | Starts or steers a Turn | Sends visible user text; bounded Map context is attached separately | Yes, explicit |

Map context contains goal, current path, current Item summary, nearby Items, and unfinished children. Limits are defined by the active Structure. Hidden context MUST be inspectable from the composer disclosure.

## 10. Automatic synchronization

### 10.1 Timing

Where Dynamic Tools are available, safe Sync operations may arrive during a Turn. Otherwise the same Turn appends a versioned response-tail update block after the user-facing answer. Studio hides it while streaming, parses it after completion, and never starts an automatic repair request. Sync MUST NOT run for every streaming token. The transcript remains usable while Sync is pending.

### 10.2 Input

- goal and definition of done;
- current path;
- a bounded nearby Map slice;
- the completed Turn;
- permitted operations and state transitions from the Structure.

### 10.3 Output

```json
{
  "schemaVersion": 1,
  "workspaceId": "map_…",
  "baseRevision": 12,
  "sourceTurnId": "turn_…",
  "idempotencyKey": "map_…:turn_…:sync:v1",
  "operations": [
    {"op": "setState", "itemId": "item_dpkg", "value": "visited"},
    {"op": "addItem", "parentId": "item_apt", "item": {"id": "item_solver", "title": "Dependency solver", "state": "active"}},
    {"op": "setCurrent", "itemId": "item_solver"}
  ]
}
```

V1 operations: `addItem`, `updateItem`, `moveItem`, `setState`, `setCurrent`, `addRelation`, `removeRelation`, and `archiveItem`. Unknown operations are rejected.

### 10.4 Simple policy

| May apply automatically with Undo | Never automatic |
|---|---|
| mark Active/Visited | mark Done/Accepted/Understood |
| move current position | change goal or definition of done |
| add a child clearly introduced in the Turn | delete, merge, or move a subtree |
| update a short Item summary | send a message or start another Turn |

If an update is ambiguous, Sync does nothing. The user sees only a small “Updated · Undo” status. Details remain available in History but are not shown by default.

`baseRevision` prevents stale writes, and `idempotencyKey` prevents duplicate Sync after reconnect. Each update stores inverse operations. A Sync never triggers another Turn or recursively triggers itself.

## 11. Views

Views are built-in, schema-validated renderers:

| View | Use |
|---|---|
| Tree | nested structure with compact state |
| Outline | dense ordered hierarchy |
| Path | ordered stages and current position |
| Checklist | explicit review or completion |
| Board | Items grouped by state |

Changing View preserves identity, selection, state, order, and progress. Graph and timeline MAY follow after V1, but are not required for the first polished release.

## 12. Custom structures

Users can clone any built-in Structure and customize:

- Item labels and allowed nesting;
- fields;
- states and transitions;
- enabled Views;
- progress calculation;
- Sync permissions;
- composer context limits;
- Continue and Back draft text.

Definitions are declarative, versioned, validated, and pinned per Map. They cannot execute JavaScript, shell, SQL, remote requests, or unsafe HTML. Full details are in [template-system.md](template-system.md).

## 13. Persistence

Studio SHOULD use a dedicated set of SQLite tables inside a shared `studio.sqlite3`, with independent migrations. A separate `session-map.sqlite3` is acceptable for rollout isolation if documented in an ADR.

Writes are transactional. Rust validates all structures and Sync updates before storage. Export produces a portable JSON bundle plus an optional Markdown outline. Provider chat history is not copied into the Map database.

## 14. Empty and error states

- No Map: render no Map UI; expose only Thread More → Create Map.
- Creating: choose Structure, enter goal, preview, create.
- No current Item: show the goal and invite Item selection.
- Sync failed: keep chat and Map usable; show a small retry icon.
- AI worker: at most one reusable ephemeral Codex Thread exists per Map and App Server generation; initial generation and explicit AI actions are serialized through it. Automatic completed-Turn Sync does not use the worker.
- Conflict: preserve both versions and offer Keep mine / Apply update.
- Missing template: open with the pinned snapshot and generic Outline.

## 15. Keyboard and accessibility

- `Ctrl/Cmd+Shift+M`: toggle Map.
- Arrow keys: move through visible Items.
- Left/Right: collapse/expand.
- Enter: select current Item.
- Space: open Item actions.
- State is communicated by shape/text as well as color.
- All icon-only controls have tooltips and accessible labels.
- Focus, 200% zoom, high contrast, screen readers, and reduced motion are supported.

## 16. Performance

- Open a 500-Item local Map: p95 under 250 ms on the reference Linux x64 machine.
- Select an already rendered Item: under 50 ms.
- Apply a 20-operation Sync transaction: p95 under 100 ms after the update is available.
- First meaningful render never waits for Sync.
- Views SHOULD virtualize above 300 visible Items.

## 17. Acceptance criteria

| ID | Acceptance |
|---|---|
| SM-001 | A user creates Hierarchy, Path, Flow, and Blank Maps without seeing domain-specific template names. |
| SM-002 | Goal and current location remain visible in one compact row above the composer. |
| SM-003 | Selecting an Item updates location without sending or invoking AI. |
| SM-004 | A completed Turn can mark Items Active/Visited, add a clear child, and move current position in one idempotent Sync. |
| SM-005 | Sync shows only “Updated · Undo” by default; Undo restores the prior revision transactionally. |
| SM-006 | Done/understood/accepted, goal changes, deletion, merge, and subtree moves never occur automatically. |
| SM-007 | Ambiguous, malformed, stale, oversize, and unknown-operation updates leave the Map unchanged. |
| SM-008 | Continue and Back create editable drafts and never auto-send. |
| SM-009 | Tree, Outline, Path, Checklist, and Board retain the same Item identities and state. |
| SM-010 | A custom Structure validates, previews, versions, exports, imports, and creates a Map. |
| SM-011 | Existing Maps remain pinned when a Structure publishes a newer version. |
| SM-012 | Restart restores goal, structure version, selected View, current Item, Items, states, and change history. |
| SM-013 | The rail remains usable at 200% zoom and with keyboard/screen reader only. |
| SM-014 | A 500-Item Map meets performance budgets and works offline. |
| SM-015 | Rust persistence/validation, JavaScript reducer/view, Undo/conflict, and end-to-end interaction tests pass. |

## 18. Delivery order

1. SQLite model, revisions, Undo, goal row, Hierarchy, Tree/Outline, and manual edits.
2. Current Item, composer context, Continue/Back, and responsive rail.
3. Post-Turn Sync with the simple automatic/never policy.
4. Path/Flow structures, Path/Checklist/Board views.
5. Custom builder, versioning, import/export, and migration preview.

Automatic Sync MUST not ship before persistence and Undo are reliable.

## 19. Open decisions

Resolved for the experimental branch:

1. Use rollout-isolated `session-maps.sqlite3`.
2. Creation remains manual. Once the user explicitly creates a Codex Map, Studio generates its initial Items from bounded recent history without adding a visible conversation message.
3. A created Map opens immediately and reopens when its Thread is selected unless the user closed it in the current app run.

Still open: choose a bounded change-history retention policy before release.
