# Build-time syntax profiles

The default `syntax-full` profile retains all 110 Arborium grammars. The
`syntax-reduced` profile links 97 and removes exactly these 13 grammars and their
compiled parser tables: COBOL, D, HLSL, F#, OCaml, Idris, Odin, C#, Perl, Ruby,
Elixir, Visual Basic and PHP. Lean and Verilog remain supported. This is a
build-time choice, not a runtime registry filter.

```sh
# Default server / fixed reader package: full language support.
cargo build --locked --release --bin marknoria-server
# Smaller server / SDK consumer; explicitly disable default full support.
cargo build --locked --release --no-default-features \
  --features web,syntax-reduced --bin marknoria-server
# A toolkit-free reduced Rust reader, without HTTP:
cargo build --locked --release --no-default-features --features syntax-reduced
```

A consuming workspace uses `marknoria = { path = "vendor/marknoria",
default-features = false, features = ["web", "syntax-reduced"] }`. No Studio
source changes are supplied here. `diagram-png` works with either profile.
Browser/WebView and all native products retain full support by default; desktop,
GNOME, FFI and macOS/Windows worker build scripts explicitly select full support.

Cargo features are additive. Selecting `syntax-reduced` together with any of the
13 excluded grammar features fails compilation with an explicit diagnostic,
including when another workspace dependency re-enables defaults. Inspect the
whole consumer graph rather than assuming a path dependency controls it. An
executable `--all-features` build is intentionally invalid; use the explicit
profile commands. `cargo metadata --all-features` remains valid for inventory.
`--no-default-features` alone also selects the 97 base grammars, but explicit
`syntax-reduced` is recommended because it enables the conflict guard.

Excluded languages preserve their canonical names, filename/fence aliases,
escaping, original text and exact source anchors. They render as plain text;
missing injected grammars also fall back safely. Remaining languages and retained
injections keep highlighting. Search and document parsing are unchanged. The
single cfg-aware grammar registry drives both lookup and inventory, preventing a
language list from advertising absent parsers.

## Validation and measurements

```sh
cargo test --locked --no-default-features --features syntax-reduced \
  --lib core::highlight
cargo test --locked --no-default-features --features syntax-reduced \
  --test syntax_profiles --test reader_api
cargo test --locked -p marknoria-syntax --no-default-features --features reduced
npm run check:syntax-features
```

The feature check compares actual Cargo dependency sets (110 vs 97) and verifies
that accidental feature unification fails. The source/fence contract covers all
13 exclusions, BOM/CRLF, Unicode, inert markup and original UTF-8/UTF-16 anchors.
Existing retained-language/injection tests remain active in both configurations.

For Linux ELF measurement, build each server with identical locked release flags,
changing only `syntax-full` / `syntax-reduced`. Keep symbols in the *final binary*
with `cargo rustc ... -- -Cstrip=none`; do not change global RUSTFLAGS or rebuild
dependencies with different optimization. Copy each output before the next build.
`tests/server/syntax-binary-check.py FULL REDUCED --build target/release/build
--output NEW_DIRECTORY` fingerprints the actual `ts_parse_table` bytes against
compiled parser objects, proves the exact 13-table difference, strips copies
identically and creates deterministic single-server gzip archives. It reports
ELF sections and file sizes; source/download/cache size is not a substitute for
linked binary evidence.

Build `--example syntax-profile-probe` once per profile and copy each executable.
`tests/server/syntax-profile-timing.py FULL_PROBE REDUCED_PROBE --output FILE`
alternates 12 fresh processes per profile, measuring process-to-ready, first
production Rust highlight and mean of 32 subsequent highlights. Run without
concurrent builds. OS caches are not purged; this is not cold-disk startup, whole
reader indexing, Studio startup or an AppImage measurement. Close timings are not
evidence of a reliable speed improvement. See the release acceptance record for
actual host/tool versions, fixed source revision and results. macOS/Windows need
their own executable/link validation; target dependency graphs alone do not prove
those builds.
