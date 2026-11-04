# Source projection candidate search

- Upstream: https://github.com/BurntSushi/memchr
- Package: `memchr` 2.8.3, pinned in `crates/markdown-native/Cargo.toml`.
- Crate SHA-256: `cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98`.
- License selected: MIT; unmodified `LICENSE-MIT` copied as `LICENSE`.
- Source is consumed unmodified through Cargo.

`memchr2_iter` performs the candidate byte scan before parsing a source fragment
for deferred data images. Exact ASCII-insensitive prefix checks retain mixed-case
URI behavior. Fragments without candidates keep their original source window.
