# Session Map structure system

Status: proposed  
Companion: [Session Map specification](specification.md)

## 1. Goal

The template system makes Session Map adaptable without exposing implementation complexity in everyday use. Product UI calls a template a **Structure**.

A Structure describes behavior, not a domain. “Hierarchy” can represent a book, package inventory, organization, concept map, or software architecture. Domain-specific configurations are user presets built on top of abstract Structures.

## 2. Five independent layers

1. **Shape:** containment, ordering, optional relations.
2. **Items:** generic types, fields, and nesting rules.
3. **States:** transitions and progress calculation.
4. **Views:** Tree, Outline, Path, Checklist, or Board.
5. **Sync and context:** safe automatic changes and bounded composer context.

Separating these layers prevents a View from becoming a data format and lets one Map switch presentation without conversion.

## 3. Built-in family

```text
studio/base@1
├─ studio/hierarchy@1
├─ studio/path@1
├─ studio/flow@1
└─ studio/blank@1
```

- **Hierarchy:** nested Items, manual order, optional progress.
- **Path:** ordered stages, one current position, forward/back movement.
- **Flow:** states and transitions, optional grouping and dependencies.
- **Blank:** minimal Item model; user enables capabilities explicitly.

Domain presets MAY extend these templates but are not shipped as top-level product concepts.

## 4. Manifest

JSON is the storage and interchange format. Most users interact with a visual builder.

```json
{
  "$schema": "https://codex-thread-studio.local/schemas/session-map-structure-v1.json",
  "schemaVersion": 1,
  "id": "user/rui/research-map",
  "version": "1.0.0",
  "extends": "studio/hierarchy@1.0.0",
  "display": {
    "name": {"zh-CN": "研究结构", "en": "Research Map"},
    "icon": "hierarchy",
    "accent": "teal"
  },
  "shape": {
    "containment": true,
    "ordering": "manual",
    "relations": ["relatedTo", "dependsOn"],
    "maximumDepth": 8
  },
  "itemTypes": {
    "group": {
      "label": {"zh-CN": "分组", "en": "Group"},
      "allowedChildren": ["group", "item"]
    },
    "item": {
      "label": {"zh-CN": "条目", "en": "Item"},
      "allowedChildren": ["item"],
      "fields": {
        "weight": {"type": "number", "minimum": 0, "default": 1},
        "tags": {"type": "tags", "maximumItems": 20},
        "note": {"type": "multiline", "maximumLength": 4000}
      }
    }
  },
  "states": [
    {"id": "notStarted", "semantic": "notStarted"},
    {"id": "active", "semantic": "active"},
    {"id": "visited", "semantic": "visited"},
    {"id": "done", "semantic": "done", "automatic": false},
    {"id": "paused", "semantic": "paused"}
  ],
  "transitions": {
    "notStarted": ["active", "paused"],
    "active": ["visited", "done", "paused"],
    "visited": ["active", "done", "paused"]
  },
  "progress": {
    "strategy": "weightedLeaves",
    "completedStates": ["done"]
  },
  "views": [
    {"id": "tree", "renderer": "tree", "default": true},
    {"id": "outline", "renderer": "outline"}
  ],
  "sync": {
    "automatic": ["setCurrent", "setState:active", "setState:visited", "addItem", "updateSummary"],
    "manual": ["setState:done", "moveItem", "archiveItem", "changeGoal"]
  },
  "context": {
    "include": ["goal", "currentPath", "currentSummary", "unfinishedChildren"],
    "limits": {"ancestorDepth": 5, "siblingCount": 4, "characters": 5000}
  },
  "actions": [
    {"id": "continue", "kind": "composerDraft", "text": "Continue with {{current.path}}."},
    {"id": "back", "kind": "composerDraft", "text": "Summarize {{current.title}}, then return to {{current.parent.title}}."}
  ]
}
```

## 5. User customization

### 5.1 Simple builder

The default builder asks only:

1. Name;
2. Base Structure;
3. enabled Views;
4. Item labels;
5. visible states;
6. whether safe Sync is enabled.

It provides a live preview. Advanced settings remain collapsed.

### 5.2 Advanced builder

Advanced mode exposes fields, nesting, transitions, progress, relations, context limits, Sync permissions, and draft actions. It also supports JSON editing with schema completion.

Every save validates first. Editing a Structure already used by a Map creates a new immutable version.

## 6. Field types

V1 supports bounded declarative fields: `string`, `multiline`, `number`, `boolean`, `enum`, `date`, `tags`, `checklist`, `url`, `fileRef`, `threadRef`, and `turnRef`.

Fields MAY specify label, help, default, required, range/length, and simple equality-based visibility conditions. Imported definitions cannot supply executable HTML or unbounded regular expressions.

## 7. View registry

V1 uses a closed renderer registry:

```text
tree | outline | path | checklist | board
```

A View only receives schema-validated data and options. Unknown renderers fall back to generic Outline so content remains readable.

Third-party renderer code is deferred until there is a separate signed, sandboxed plugin model with declared capabilities and no direct database or provider access.

## 8. Sync policy

The policy is binary and understandable:

- `automatic`: safe, reversible changes that may follow directly from the completed Turn;
- `manual`: meaning-changing, destructive, or user-judgment actions.

The platform always forces these operations to manual: Done/Accepted/Understood, goal changes, delete, merge, subtree move, and message send. A child Structure cannot weaken this rule.

When uncertain, Sync produces no operation. There is no user-facing score or technical review dialog. Applied changes show a brief Undo affordance and remain in History.

## 9. Validation

JavaScript validates for immediate feedback; Rust validates authoritatively before persistence.

Validation covers:

- supported schema version;
- unique ID and semantic version;
- inheritance cycles and depth;
- localized-name fallback;
- unique Item, state, View, relation, and action IDs;
- valid nesting and transitions;
- valid renderer options;
- progress-state consistency;
- platform-forced manual operations;
- bounded context, fields, actions, and manifest size;
- allowlisted draft placeholders.

Invalid imports open in read-only quarantine with actionable field paths.

## 10. Versioning and inheritance

Templates are immutable and use semantic versions:

- Patch: labels, help, and visual defaults.
- Minor: additive fields, Item types, Views, and transitions.
- Major: removed/renamed types or states, new required fields, or changed progress meaning.

A child may add capabilities and make Sync stricter. It may not change existing semantic meanings or weaken platform safety.

Each Map pins a fully resolved snapshot. A new version never silently changes existing Maps.

## 11. Migration

Migration uses declarative transformations only:

- `renameType`;
- `renameField`;
- `mapState`;
- `setDefault`;
- `archiveField`.

Before application, Studio shows affected Item counts and a small sample. Migration is one transaction and one undoable history entry. Arbitrary migration scripts are forbidden.

## 12. Import and export

Structure export contains the manifest and optional author/license metadata. It excludes Map content, conversations, paths, and credentials.

Import computes a content hash:

- identical ID/version/hash: reuse;
- same ID/version with different hash: quarantine and require a new ID/version;
- missing parent with resolved snapshot: allow standalone clone;
- newer unsupported schema: read-only preview with upgrade guidance.

## 13. Acceptance criteria

| ID | Acceptance |
|---|---|
| ST-001 | Built-in cards are named Hierarchy, Path, Flow, and Blank, with no domain-specific top-level template. |
| ST-002 | One Hierarchy Structure can represent a book, package inventory, course, and component tree without schema changes. |
| ST-003 | A non-programmer can clone, rename Item labels, enable a View, preview, and save a valid Structure. |
| ST-004 | A Structure cannot execute script, shell, SQL, network fetch, or unsafe HTML. |
| ST-005 | Automatic Sync cannot be configured to mark Done, change the goal, delete, merge, move a subtree, or send. |
| ST-006 | Existing Maps remain pinned when a new Structure version is published. |
| ST-007 | A compatible migration previews, applies transactionally, and can be undone. |
| ST-008 | Unknown renderers fall back to Outline without data loss. |
| ST-009 | Validation returns an actionable JSON path for every rejected rule. |

