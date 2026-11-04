# Embedded SQL and jq query port

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
