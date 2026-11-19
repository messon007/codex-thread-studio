# Member management interaction review

Date: 2026-11-19. Reviewer: GPT-6.2-Astra, explicitly requested by the user.
Scope: `feat/agent-teams`; source, interaction paths, actual Chromium screenshots
and the isolated Studio/Codex browser probe. Main is not changed or merged.

**Final conclusion: approved, no blocking findings.**

The review required the following before acceptance:

- Studio's shared action-menu, icon buttons, separators, danger style and segmented
  controls; group rows expose View members/Add member; member rows expose Edit and
  a visible More menu, rather than repeating many equally weighted buttons.
- One member per edit form, complete name/responsibility editing, one optional
  group owner, and clear removal confirmation preserving the native session.
- Removal diagnostics from actual backend predicates, including unanswered
  questions, unsettled execution/review receipts and exact-session delivery holds.
  Blocker links select the correct group and return to the original member.
- Menus placed above bottom-edge triggers, keyboard navigation, stable focus and
  scroll restoration, and protection against background refresh replacing a menu.
- Concurrent edits recovered in the same form. Drafts remain; differing saved
  values require explicit use-latest or keep-mine selection before saving.
- Network failure remains visible. Lost responses reconcile current roster state;
  missing members and archived/deleted groups provide a return path without an
  automatic resend or retry loop.

The reviewer inspected light/dark, English/Chinese, narrow 720px and 20px interface
screenshots in `validation/member-management-03`, and independently checked the
successful 29-scenario `member-management-04` report, including failure before
sending and response loss after successful removal. The final independent label
nodes for immediate language switching also passed source review; their final
browser run is recorded in [validation](member-management-validation.json).

Tests use real Studio, Codex 0.170.0 App Server and Chromium, with an isolated
synthetic Responses provider. This is UI/protocol acceptance, not a claim about
real-model planning quality or Windows rendering.
