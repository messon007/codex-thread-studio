# Shared reader acceptance update — historical PR #58 checkpoint

This records PR #58 (`794a322`) at its original `4c20b45` SDK pin.
PR #59 supersedes that package with `89a3210` and retains its host translations
and regression additions. Current delivery and rerun evidence are in
[marknoria-sync-20261106.md](marknoria-sync-20261106.md); the hashes and validation
claims below describe the earlier checkpoint only.

This adopts the fixed, unpatched 815-file SDK for MarkNoria acceptance issues
#13–#17. Root #18's accessibility-session diagnosis is documented upstream;
its unconfirmed Gdk popup warning remains pending with the user's agreement.

The shared reader provides an explicit SQL substring-correction action, prompts
for the numeric chart axis before requesting data, and shows full-cell selection
actions only for actually selected text. Exact middle snippets retain cell/query
identity and owned-result verification. PDF original-byte ranges now wait in a
bounded queue instead of failing ordinary parallel bursts immediately. Two active
work slots remain; queue capacity is 16 and its deadline is five seconds.

The standalone header puts Open, Notes and all view modes before its separator.
The Studio host toolbar remains owned by Studio. Public API/bridge stay v1; file
authority, strict import verification and existing audit policy are retained.

Source: 4c20b4572f4d57797885264912adf185974448f4

- Archive SHA-256: 109eb3da194b0c9b4832eb9c8f8507bfdaea06e2f82d5ecf8066e0d9847aab6d
- Tree SHA-256: 08f9b33a76a91db38a2a1b4d4f8d92fff8636a04cb901bef93f52d48f2b4c5d6

Two clean offline exports are byte-identical. The reviewed pin is
`scripts/marknoria-release.json`; imported sources are not patched. The independent
SDK tests, Studio workspace and 14 public contracts, 793 UI tests, type/generated
checks and actual-app regression all pass locally. Results and current three-platform CI/merge
transactions are recorded in the adopting PR. Upstream actual Wayland regression
includes the originally failing 145-page PDF, 132 SQL/browser assertions, 47 real
pointer/keyboard table assertions and six viewport sizes. Private documents and
screenshots are excluded from this checkout.

The actual Studio regression adds explicit correction/no-auto-execution, chart
numeric-axis selection and full-cell substring identity checks to the existing
large structured-file/jq checks. Native Wayland coverage retains the large-file
and jq scenes; the new SQL/cell/chart full-app scenes run in Chromium. Shared
cell/query/chart component contracts also pass separately in native Wayland.

No release-size optimization or new runtime-version acceptance is included.
