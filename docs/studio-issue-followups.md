# Studio reader issue followups

Functional delivery for Studio #38, #52, #53 and #54, following PR #55. Shared
changes are tracked by MarkNoria #8, #9, #11 and #12. No platform-native UI or FFI
code changed. Studio changes use the public reader protocol and shared components;
the original Studio checkout remains untouched.

## Delivery and behavior

The fixed SDK contains 815 inventory files from source revision
`5e5d4de6e2ffec368494bc52d0ecffe5068b765a`. Two clean offline exports match:

- Archive SHA-256: `dc5c491b5d6b87673333c733a7bd4cee927cee516b90cfd9bf8d2a882d736f8e`
- Tree SHA-256: `724a1fe22ea458d49f870658cf3079f399ff5b5c932a430b27904a72b53da8c4`

Studio imports this inventory without source patches. API and bridge remain v1.

| Issue | Result |
| --- | --- |
| Studio #54 / MarkNoria #8 | SQL help explains single-quoted literals, LIKE/instr substring matching and IN membership. The original invalid query reports `no such table: c3`; prior results survive and valid queries recover. No SQL rewriting or new dialect. |
| Studio #52 / MarkNoria #9 | The adopted reader handles structured files beyond the legacy 5 MiB limit and fresh documents open in Preview even after the preceding document used Source. |
| Studio #38 / MarkNoria #9 | A thin Studio toolbar control mounts the unchanged shared jq component/provider. It captures document identity and cancels obsolete loading/controls on replacement. Large files retain explicit Run behavior. |
| Studio #53 | The existing attachment removal action uses a centered SVG in a compact rounded square, without the unwanted circle/padding. |
| MarkNoria #11 | Compare's static allowlist includes its existing outline/reference modules so panes initialize normally. No API or authority change. |
| MarkNoria #12 / Studio #25 dependency followup | resvg/usvg 0.48.1 replaces the rustybuzz/ttf-parser dependency chain with maintained font dependencies. Root/public manifests agree. An SVG text pixel regression verifies rendering; existing resource restrictions remain. |

## Verification

Ubuntu 26.04: required locked desktop tests pass with diagram rasterization enabled
(143 library tests plus integration checks); assets build, bounded translations,
Compare browser suite and SQL browser suite (116 assertions) pass. SQL and jq
components pass in Chromium and mapped native Wayland WebKit. The independently
imported SDK's locked workspace/all-target tests pass. Studio workspace tests,
14 public contracts, 793 UI tests, generated assets, type checks and strict import
verification pass. Fresh-database security audit passes using Studio's existing
policy, with no new exceptions.

`scripts/smoke-reader-issue-regressions.mjs --webkit` in the Studio consumer runs
the unmodified application and public reader protocol. It checks 7,800,013-byte
XML, 2,327,791-byte JSON, 50,000-record / 2,413,890-byte JSONL, one 900,020-byte
JSONL record and a Markdown SQL table. Both engines verify Preview, visible jq
controls, whole-input jq results, source availability and dialog bounds. Chromium
compares bounded Source bytes exactly with the original UTF-8 range; WebKit checks
the source prefix. Originals remain unchanged and reader caches are removed.
SQL error/recovery and LIKE/instr/IN results are checked in the actual Chromium app.
Attachment geometry invokes the production renderer with synthetic attachment
data and actual CSS in both engines; it is not an authenticated chat/session test.

The font dependency update may change glyph metrics slightly. No new performance
guarantee is inferred from these correctness tests; prior cold-start and first
Chinese-locale observations remain recorded in `web-parity.md`. Current remote
CI/merge evidence is recorded in the consuming PR and linked issue closures.

## Deferred work

Studio #51 / MarkNoria #10 requested a measured size proposal. The user selected
functional delivery first and proposal only. See `release-size-plan.md`; no asset
deduplication, stripping or release compression change is implemented, and no
50 MB portable-package result is claimed. Studio #25's runtime upgrade decisions
are distinct from resolving the reader dependency warnings; this delivery does
not install or accept new Codex/OpenCode runtime versions.
