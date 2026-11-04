# Ecosystem review — 2026-11-04

This review addresses issue #25 after the document table issues #39 and #43.
Official CLIs are installed into temporary directories. No system installation,
user configuration, existing daemon or active session is upgraded or restarted.

## Runtime decision and contract comparison

| Runtime | Previous reviewed version | Tested candidate | Used-contract SHA-256 |
| --- | --- | --- | --- |
| Codex CLI | 0.154.0 | 0.166.0 | `2c18e816d892` (prefix) |
| OpenCode | 1.18.31 | 1.19.5 | `20b00baf4b30` (prefix, unchanged) |

The old official binaries reproduce both previous accepted hashes. The current
binaries retain all monitored methods, notifications, requests and operations.
The candidate versions are recorded as reviewed after the session checks;
this is a compatibility baseline, not a request to upgrade running installations.

Codex's projection differs in the following ways:

- Optional request fields were added to initialization, MCP status, thread
  listing/start/fork and turn start/steer. Existing requests remain valid.
- `review/start.delivery` no longer accepts `detached`. Studio sends `inline`
  (`ui/app.js`), which remains supported.
- Image input and image tool output can now use file IDs as well as URLs. Studio
  still sends the supported URL/local-image variants. Optional generated-image
  fields and MCP app UI metadata do not change existing text handling.
- Item timestamps, partial-answer phases, root turn IDs, error variants and
  elicitation metadata were added. Studio's reducers preserve agent text without
  requiring a fixed phase enum; existing completion/error handling still applies.
- Resume history has additional response item variants. Studio resumes by thread
  ID and does not construct raw upstream response history.

No wire adapter change is needed for the requests Studio currently sends. See
[Codex release notes](https://developers.openai.com/codex/changelog) and
[OpenCode 1.19.5 release notes](https://github.com/anomalyco/opencode/releases/tag/v1.19.5).

Session validation uses actual official binaries with local mock model providers:

- `python3 scripts/smoke-codex-shared.py` uses the production Rust connection
  source. Two Studio clients and Codex CLI submit turns to the same private
  daemon, receive completions, and reconnect after the private daemon restarts.
- `STUDIO_OPENCODE_BIN=/path/to/opencode node scripts/smoke-opencode-session.mjs`
  uses Studio's actual protocol adapter. It checks authenticated health, create,
  rename, asynchronous prompt submission, SSE messages, model completion,
  history/resume, fork and delete in a private temporary directory.

These prove transport/adapter behavior against deterministic local model replies;
they do not test live provider credentials, model quality or external MCP servers.

## Project health and security decisions

The issue's November 2 workflow failed at the JavaScript audit (eight moderate
findings in the older archive dependency tree). Main already contains the
minimatch/glob/unzipper fixes and archive/XLSX regression checks from PR #40.
The current `npm audit --audit-level=moderate` succeeds; two low-severity findings
in the Mermaid/KaTeX tree remain below the configured failure threshold.

The Rust audit, previously masked by the failed npm step, requires these fixes:

| Dependency | Change | Advisory |
| --- | --- | --- |
| pulldown-cmark | 0.13.5 → 0.13.6 | RUSTSEC-2026-0433, quadratic wikilink parsing |
| zip | 8.6.0 → 9.0.0 | RUSTSEC-2026-0429/0369/0368/0367, unsafe extraction cases |
| Tokio | 1.53.1 → 1.53.3 | RUSTSEC-2026-0386, AsyncFd I/O safety |
| Syntect features | built-in syntaxes/themes, HTML and fancy regex only | removes unused yaml-rust loader and RUSTSEC-2024-0320 |

Studio reads ZIP members rather than extracting arbitrary archive trees, and
MarkNoria does not enable wikilinks; patched versions are still used to eliminate
the affected dependencies. The dependency lock and reproducible MarkNoria patch
must capture these changes. Highlighting tests retain HTML escaping and output
limits with the smaller feature set. ZIP 9 filename decoding is fallible; EPUB
and XLSX validators now return an error for invalid names. Their four existing
archive/state tests pass against the new version in a headless source harness.

Validation completed locally: 748 UI tests, TypeScript/version/format checks,
36 MarkNoria tests, unchanged generated browser assets, the reproducible vendor
patch, reader browser selection/query/table interactions, the four archive tests
and both actual CLI session checks. Full platform Rust checks run in CI because
the local system lacks GTK development packages.

### Remaining bincode maintenance warning — upstream investigation

[RUSTSEC-2025-0141](https://rustsec.org/advisories/RUSTSEC-2025-0141) classifies
bincode 1.3.3 as unmaintained, with no patched release. Syntect 5.3.0 requires this
version for its built-in serialized syntax/theme dumps, including through its
`parsing` and `default-syntaxes` features. The currently published Syntect version
has no bincode-free replacement feature.

`crates/marknoria/src/highlight.rs` only calls
`SyntaxSet::load_defaults_newlines` and `ThemeSet::load_defaults` for those assets.
It never deserializes document bytes as syntax/theme data. The optional external
YAML/plist grammar loaders are disabled. Documents supply source text to the
bounded HTML highlighter, not bincode-encoded grammar objects.

This is a maintenance risk, not a disclosed bincode vulnerability. The current
maintenance decision is tracked by [MarkNoria #1](https://github.com/messon007/marknoria/issues/1):
benchmark alternative highlighters fairly and adopt the best verified implementation;
waiting for Syntect's upstream dependency strategy is conditional on Syntect winning
that evaluation. Studio follows the tested upstream result through issue #25.

No audit exception was added. The existing audit configuration continues to deny
this warning until a reviewed upstream solution is adopted. This is an upstream
follow-up decision, not a pending request to approve an ignore. Broader shared
reader maintenance follows [the MarkNoria-led sync plan](marknoria-maintenance.md)
and [Studio #48](https://github.com/messon007/codex-thread-studio/issues/48).
