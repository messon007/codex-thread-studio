# Long Codex session recovery (2026-11-16)

Tracked in [Studio #76](https://github.com/messon007/codex-thread-studio/issues/76).
A cold selection on main `8fef4b0` requested the entire thread in one resume RPC.
The affected session's read-only full-history response contained 402 turns and
69,420,095 bytes; it returned in 1,187 ms, but exceeded Studio's 64 MiB shared
WebSocket limit. The connector discarded the capacity error and published only
`App Server transport closed`. Pending frontend RPCs then waited for their
30-second timeout. A four-turn page was 114,866 bytes and returned in 5 ms.
These are local diagnostic samples, not a general performance benchmark.
Private session text and paths are not included here or in GitHub fixtures.

## Corrected behavior

Cold selections now request `excludeTurns` and the existing initial four-turn
page. Subsequent `thread/turns/list` responses reconstruct the complete ordered
transcript. Cached selections retain the inclusive tail-anchor reconciliation;
if the anchor cannot be found, full reconstruction also uses pages. Cold
background reads request metadata without turns and then page; they never
resume a session merely to read its history.

The existing four-turn page size, 64 MiB frame/message cap, heartbeat and RPC
timeout are unchanged. Full reconstruction is additionally capped at 10,000
pages and 256 MiB of serialized page data. It rejects malformed pages, repeated
cursors and exhausted budgets explicitly; it never presents a partial transcript
as complete. Duplicate turn IDs on overlapping pages retain the first, newest
copy. This is transfer pagination, not lazy history display: the complete model
is still installed after reconstruction, with existing transcript DOM windowing.
A single oversized turn can still exceed the transport limit; this change does
not silently truncate such a turn. Old servers explicitly lacking pagination
retain their compatibility path and its existing transport limits.

The upstream App Server already performs history indexing and pagination. Studio
uses that reusable API instead of introducing a second parser or database cache.
Frontend work is request orchestration and merging the structured model it
already renders. Serialization for the full-walk byte guard is page-local;
requests are sequential and each RPC keeps its ordinary timeout. No third-party
dependency or protocol schema changes are needed.

The shared Rust connector now preserves receive/write/heartbeat failures in
stream order. Active frontend requests, one-shot background sockets and internal
Rust requests reject on terminal server status with the actual reason. Late
responses cannot settle an already rejected request. Unrelated transport/session
errors do not cause an unbounded full-history retry.

## Regression coverage

- History-loader tests cover a 402-turn response larger than 64 MiB over 101
  bounded pages, full ordering, cold/background loading, cached reconciliation,
  environment forwarding and legacy compatibility. Cursor/byte/page failures
  remain errors, with no full-history retry.
- Frontend RPC tests cover immediate terminal rejection and late responses.
  Rust tests exercise a real WebSocket capacity failure and immediate internal
  request rejection.
- `scripts/smoke-codex-large-history.mjs` uses the production Studio executable
  with `scripts/fixtures/large-history-codex.py`, an offline shared byte-proxy
  fixture. It cannot execute commands or inference. The old binary reproduces
  the failure with a 70,928,575-byte response. The fixed binary must restore all
  402 turns, navigate to both ends and repeat a cold open after browser reload,
  with each response below 1 MiB and no transport stop. Run with the ordinary
  `STUDIO_REMOTE_BINARY`, `STUDIO_PLAYWRIGHT_MODULE`, `STUDIO_CHROMIUM_PATH`
  overrides; `STUDIO_EXPECT_OVERSIZE=1` selects the old-binary negative control.

The adopting PR records final runtime timings, memory observations and required
cross-platform CI. All fixture profiles and processes are isolated from the
user's active daemon and sessions. No release/tag is part of this fix.

Final local candidate validation: 817 UI tests; 159 Rust tests plus 15 integration
tests (one existing ignored). Chromium completed cold selection in 1,610 ms;
native Wayland WebKit 2.52.6 completed its two cold opens in 761/716 ms. Each
engine restored all 402 turns twice and navigated to both endpoints. Maximum RPC
response was 706,176 bytes; four opens transferred 283,742,898 bytes in total.
The Chromium reload sample reported a 107,053,813-byte JS heap and no observed
long tasks; this is one synthetic diagnostic sample, not a peak-memory guarantee.
No transport stops or browser errors occurred. Final binary SHA-256:
`b74f3f8fa493100ca414071bafb775433939a7415ccb2dbac3a032c04d09a033`.
