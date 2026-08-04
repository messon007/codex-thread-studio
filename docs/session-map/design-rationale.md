# Session Map visual and interaction rationale

Status: design reference  
Last reviewed: 2026-08-04

## Direction

Session Map should feel like a native part of a conversation, not a dashboard inserted beside it. The refined design uses content-first hierarchy, compact controls, generous spacing, adaptive layout, and progressive disclosure.

It does not copy another product’s visual assets. It applies interaction principles from current OpenAI product documentation and Apple’s Human Interface Guidelines.

The current Demo isolates the proposal to the trailing Map rail and deliberately reuses Codex Thread Studio's existing sidebar, toolbar, transcript, and composer unchanged. Composer integration remains a later implementation decision.

## OpenAI product alignment

Current OpenAI guidance for long-running work places the goal progress row above the composer and lets the user pause, resume, edit, or clear it there. Session Map extends that same compact row with a current-location subtitle and a disclosure into the Map rail. The chat remains the primary surface.

OpenAI’s project guidance recommends separate chats for distinct outcomes and keeps project/chat organization in the sidebar. Session Map therefore belongs to one Thread; it does not become a second global project hierarchy.

Relevant official references:

- [Long-running work](https://learn.chatgpt.com/docs/long-running-work)
- [Projects and chats](https://learn.chatgpt.com/docs/projects)
- [ChatGPT desktop app](https://learn.chatgpt.com/docs/app)

## Apple HIG alignment

- A trailing rail is used for contextual navigation while the existing leading sidebar continues to navigate top-level chats. [Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars)
- The rail toolbar contains only Add, View, and More. Frequent actions remain one click away; uncommon commands use a menu. [Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars), [Menus](https://developer.apple.com/design/human-interface-guidelines/menus)
- Detail and customization appear only when selected or requested. [Disclosure controls](https://developer.apple.com/design/human-interface-guidelines/disclosure-controls)
- Typography uses a small number of weights, strong contrast, short labels, and whitespace for hierarchy instead of many boxes and headings. [Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
- The rail adapts to an overlay or full-screen sheet rather than compressing chat below a usable width. [Layout](https://developer.apple.com/design/human-interface-guidelines/layout)

## Applied decisions

| Previous concept | Refined design |
|---|---|
| large permanent objective card | one compact goal row above the composer |
| domain templates | abstract Hierarchy, Path, Flow, Blank Structures |
| visible template/view/customize fields | icon toolbar; Structure only under More |
| status labels on every row | status shape plus optional compact count |
| action bar inserted below a selected Item | one trailing More button with an anchored context menu |
| technical update review dialog | quiet “Updated · Undo” status |
| user-facing confidence and evidence | removed |
| many explanatory sections | whitespace and progressive disclosure |

## Visual rules

1. One accent color is used sparingly for current position, selection, and progress.
2. Borders separate major regions; cards are reserved for transient dialogs and selected content.
3. Primary text is at least 10–13 px in the desktop rail/demo scale, with adequate weight and contrast.
4. Icon-only controls always have tooltip and accessible labels.
5. A row has one primary click target. Secondary actions use one trailing More button on selection/hover and never shift surrounding rows.
6. User-visible vocabulary is short: Map, 结构, 同步, 历史, 关闭.
7. Technical terms such as revision, inverse operation, schema, and idempotency remain in documentation/diagnostics, not everyday UI.
