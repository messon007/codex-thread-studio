# CommonMark reference-label matching

- Upstream: https://github.com/seanmonstar/unicase
- Package: `unicase` 2.10.0, pinned in `crates/markdown-native/Cargo.toml`.
- Crate SHA-256: `357cc3acc6a036009fd6c973ed009037c732d60d0b4f6c673e9041497482a28f`.
- License selected: MIT; unmodified `LICENSE-MIT` copied as `LICENSE`.
- Source is consumed unmodified through Cargo.

The reference context and snapshot metadata use the same Unicode case-folding
key type as pulldown-cmark. Label whitespace is normalized by pulldown-cmark,
not by a separate Markdown/reference parser.
