# Embedded image data URLs

- Upstream: https://github.com/servo/rust-url/tree/main/data-url
- Package: `data-url` 0.3.2, pinned in `crates/markdown-native/Cargo.toml`.
- Crate SHA-256: `be1e0bca6c3637f992fc1cc7cbc52a78c1ef6db076dbf1059c4323d6a2048376`.
- License selected: MIT; unmodified `LICENSE-MIT` copied as `LICENSE`.
- Source is consumed unmodified through Cargo, not vendored in this directory.

The shared Rust Markdown parser uses its Fetch-standard data-URL parser and
streaming percent/base64 decoder. Canonical base64 uses the existing `base64`
block decoder after standards MIME parsing; other forms retain the streaming
standards path. Callbacks enforce decoded byte budgets; only
image MIME types are admitted. The existing image service validates the actual
format and bounded pixel decode, with private/remote SVG resources disabled.
