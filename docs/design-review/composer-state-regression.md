# Composer readiness regression

## Cause reproduced in current code

Backend switching renders cached history while `state.ready` is false. The Codex
ready handler changed the flag to true without refreshing Composer; a fresh cache
can legitimately skip the later full workspace render. Typing or removing an
attachment happens to trigger rendering, masking the missing readiness update.
OpenCode startup had the same dependency on later catalog/history rendering.

The previous browser harness manually rendered after state changes, so it did not
detect this lifecycle omission. New tests run the real Codex ready/stopped handler
and the actual OpenCode startup function with a no-op cached catalog. They assert
button changes without simulating user input or manually rendering after ready.

## Changes

- Render Composer on readiness changes; no full transcript rebuild/history reload.
- Clear readiness on starting, stopped, error and selected socket errors.
- Running status without a native Turn ID blocks Send/Continue and disables Stop
  until its required identity arrives.
- Submission and Continue handlers check live state independently of stale DOM.
- Queue does not start an independent message in the running-without-ID interval.

## Browser captures

Set `STUDIO_SCREENSHOT_DIR` when running `scripts/smoke-remote-studio.mjs`. For each
of Codex, EPT Codex and OpenCode, captures cover connecting, cached-ready, waiting
for Turn identity, draft, file attached, attachments removed, running, completed
and disconnected states. Captures are focused controller fixtures using the real
render function, not claims of full native-desktop or live-model acceptance.
Assertions check enabled/visible state; screenshots provide visual evidence too.
No real user messages are sent. Store generated captures outside the repository.

The supervision **＋** menu and direct paperclip upload are now covered by
`browser-supervision-acceptance.mjs`, using the served HTML, CSS, actual renderers
and event bindings with fake transports. Checks include running-only enable/stop,
two Router targets with independent controls, the unified SVG dimensions, and
direct image upload. Router tests cover selecting without editing the draft,
inserting a named reference only on confirmation, and appending extension actions
after the native toolbar.
These are browser component tests, not an end-to-end test of a model's judgment.

## Verification (2026-09-07)

- Type checking and all 72 generated module checks passed.
- JavaScript: 649 tests passed (obsolete pre-send intent cases removed). Rust: 121 passed; the parent-death helper is
  intentionally ignored by the direct runner and invoked by its owning test.
- The release-binary browser smoke test passed, including three-backend Composer
  readiness, running-task supervision, continuation response
  following, old-completion isolation, and history restoration without resend.
- Focused screenshots are in `/tmp/studio-controls-screenshots/` on the test host;
  they are generated artifacts, not committed repository content.
- No live LLM tasks were sent, no production SSH service was restarted, and no
  claim is made that an evaluator's completion judgment is infallible.
