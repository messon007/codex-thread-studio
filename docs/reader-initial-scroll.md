# First-wheel scrolling after opening Files

On native Wayland WebKitGTK 2.52.6, opening a document through Studio's Files UI
can deliver trusted, non-cancelled wheel events to the reader while its scroll
position never changes. Dragging the scrollbar is the user-reported workaround.
Chromium does not exhibit this failure in the retained repeated-open fixture.

Studio made the mapped reader iframe `inert` during the public initialization
handshake, then cleared `inert` on readiness. The failure survives clearing it.
Controlled tests against the same normal main binary and synthetic Markdown:

| Native WebKit control | Result over repeated Files opens |
| --- | --- |
| Original host, first wheel | 5/5 failed: scrollTop 21 → 21 despite three deltaY 99 events |
| Suppress only the initial iframe `inert=true` in the test | 3/3 passed: 21 → 318 |
| Add pointer motion before the wheel | 3/3 failed |
| Focus the frame before the wheel | 3/3 failed |
| Change scrollTop by 100 before the wheel | 3/3 failed |
| Replace inert with temporary iframe `pointer-events:none` | Failed too |

These controls identify the iframe input gating as the trigger. They do not
identify an exact internal WebKit defect or equate this with an older upstream
bug report. The test does not rely on DOM-dispatched `WheelEvent` (which cannot
exercise default scrolling): GTK scroll events enter the native WebView, and
JavaScript verifies trusted wheel delivery and the actual scroll position.

## Host correction

Keep the existing opaque loading cover in front of the iframe, `tabIndex=-1`
and `aria-hidden=true` until the initialization/settings/reference handshake
completes. Restore the tab order/accessibility state and hide the cover as before.
Do not toggle iframe inert or pointer-events. The mapped child can complete its
normal layout without accepting pointer or sequential keyboard interaction.
Do not hide/suspend the frame, synthesize document scrolling, grab focus, change
virtualization, or access the reader's private DOM in production.

This belongs to Studio's loading shell. The pinned MarkNoria SDK and its existing
#38/#39 compatibility assets remain unchanged.

## Regression

`node scripts/smoke-reader-wheel.mjs --webkit` runs an isolated, offline Studio
server with synthetic Markdown and the real Files UI. It checks a deliberately
held document response, cover hit-testing at all four frame corners, exclusion from
Tab navigation, restored controls after initialization, and first downward/upward
wheel scrolling without a content click, scrollbar drag or focus workaround.
Chromium opens five times; native WebKit opens three times. Source bytes and
fixture-owned cache cleanup are checked. DOM inspection is confined to QA.

Use `STUDIO_REMOTE_BINARY`, `STUDIO_PLAYWRIGHT_MODULE` and
`STUDIO_CHROMIUM_PATH` to select a normal built executable and installed browser.
The native helper requires PyGObject GTK3/WebKit2 4.1 and a graphical session.

Validated on 2026-11-17 with the ordinary rebuilt Studio binary: five Chromium
opens and three native Wayland WebKit opens pass both scroll directions, the
held-document input gate passes, and the existing comment persistence/selection
smoke passes in both engines. The final UI suite has 817 passing tests.
