# Embedded queries and table interactions

The base reader remains pinned to the revision in UPSTREAM.md. The SQL query
engine and dialogs and the jq engine/dialog were copied from the local MarkNoria
working tree on 2026-11-04 (base commit c9a502b). Those additions were not yet
committed upstream. They are vendored here; no sibling checkout is needed at
build time or runtime. MarkNoria's existing license/provenance applies.

`scripts/marknoria-queries.patch` records the additions and adaptations against
Studio's formatted base snapshot. `vendor-marknoria.mjs` reapplies it after the
base embedding adapter, making subsequent base refreshes reproducible.

SQL adapts input extraction to this reader's cached HTML fragments using
Kuchikiki. It loads all rows, including their original source line, into a
private disk-backed SQLite database. Named columns use `data`; positional names
`c1`, `c2`, etc. use `t`. Statements are read only, authorized, timed and bounded;
results stay on disk and are paged. No process-wide SQLite heap limit is changed.
The default projection lists every normalized, quoted field on a separate line,
including `_row` and `_line`. Tables with 255–256 data fields use the positional
view to stay within the result-column limit. If the named query would exceed the
16 KiB statement limit, the schema supplies `SELECT * FROM data` instead.
Small Markdown documents reuse Studio's existing table query UI and bounded
SQLite endpoint after DOM cell extraction.

jq uses jaq's Rust evaluator in a supervised child process, with no environment,
filesystem loader, shell or network extensions. Output stays in a private result
directory with bounded pages. Runtime and output are capped; Unix also applies
CPU/address-space limits, and Linux observes RSS. Document revision and reader
token checks apply to both query APIs. Studio's proxy explicitly allows only
these routes and their methods. JSON/JSONL/NDJSON open in the paged source reader
with a jq action; Markdown retains its existing size-based reader choice.

Validation:

- `cargo test -p marknoria --lib --locked`
- `cargo build -p marknoria --example reader-service --locked`
- `python3 scripts/smoke-reader-queries.py`
- `node scripts/smoke-reader-browser.mjs` (optional Playwright; see script header)

The HTTP smoke check covers all rows, pagination, source lines, stale revisions,
SQL write/file-access rejection, jq filters/slurp, disabled environment access,
and termination of an infinite jq filter. The browser check exercises SQL/jq
forms and the host iframe selection bridge. These are correctness checks, not
end-to-end performance measurements.

JSON, JSONL and NDJSON indexing now uses bounded inert source pages instead of
the Markdown parser. Continuous JSONL records and long minified JSON strings
therefore do not hit the indivisible-Markdown-block budget. This path preserves
UTF-8 boundaries, original bytes and source lines and does not interpret literal
HTML, diagram fences or headings as Markdown. The page writer and index
finalization are shared with Markdown. The fix is included in the reproducible
query adapter patch; both browser and native consumers use it.

GTK clients reuse the same jq evaluator, supervised worker, snapshot/revision
checks, result cache and cleanup through the authenticated native
`/studio/native/markdown/{reader}/jq-query` POST alias. They do not require a
separate MarkNoria installation, a system `jq` executable or Python at runtime.
`scripts/smoke-gnome-reader-queries.py` checks that API with private files and
also reads the source while an infinite filter is being supervised.

GTK Markdown tables use authenticated `table-query`, `table-query-schema` and
`table-query-close` aliases to this same SQLite implementation. Their native
controls share the jq query window and the existing source editor, SQL formatter,
column picker, cell styling, theme and language bindings. SQL result cells are
literal text; paging and hiding columns do not change the Markdown source.

The document table controller and column resizing styles were also ported from
that local working tree for Studio issues #39 and #43. Ordinary Markdown uses
the same layout module through a gateway asset route. Widths belong to the open
document/revision, survive table paging and offscreen remounts, and do not edit
the source file. Complex HTML tables with spans or explicit colgroups retain
their original layout. Small-document layouts mount near the viewport; width
measurement samples at most 200 rows and 16,384 characters per cell. DOM geometry
and pointer/keyboard input stay in the frontend; backend table projections and
source anchors are unchanged. Vertical wheel chaining uses browser overflow
behavior; Shift-wheel keeps horizontal table scrolling.

The browser smoke check additionally covers pointer/keyboard resizing, widths
across paged rows, native wheel chaining in both directions, small-document
mount/unmount persistence, and disposal of table controls.

The local patch also pins the security-reviewed parser/runtime fixes and narrows
Syntect to built-in syntax/theme loading, HTML and fancy regex; see
`docs/ecosystem-review-2026-11-04.md` for the dependency applicability review.
