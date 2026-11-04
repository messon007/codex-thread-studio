# Native regular-expression engine

- Upstream: https://github.com/iwillspeak/rust-onig
- Package: `onig` 6.5.3, selected by Syntect's `regex-onig` feature.
- Crate SHA-256: `0cc3cbf698f9438986c11a880c90a6d04b9de27575afd28bbf45b154b6c709e2`.
- Rust wrapper license: MIT; unmodified `LICENSE.md` copied as `LICENSE`.

Native syntax preparation uses this engine on a bounded Rust worker. The
regex backend is a build-time choice; it is not a runtime fallback.
