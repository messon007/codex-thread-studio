# Native code colors

- Upstream: https://github.com/trishume/syntect
- Package: `syntect` 5.3.0, pinned by the optional `syntax-highlight` feature in
  `crates/markdown-native/Cargo.toml`.
- Crate SHA-256: `656b45c05d95a5704399aeef6bd0ddec7b2b3531b7c9e900abbf7c4d2190c925`.
- License: MIT; unmodified `LICENSE.txt` copied as `LICENSE`.
- Source and bundled default theme dumps are consumed unmodified through Cargo.

Native text flow uses the Oniguruma regex engine, two-face's extended syntax bundle
and InspiredGitHub/base16-ocean.dark themes. The extended syntax notices are
packaged separately under `two-face`. It uses no HTML output or
runtime filesystem syntax/theme loading. Parsing and both theme projections
run on a bounded Rust worker; GTK receives immutable Unicode color ranges.
The existing MarkNoria backend independently uses the same pinned engine for
HTML output. The optional feature leaves the shared core dependency-light.
