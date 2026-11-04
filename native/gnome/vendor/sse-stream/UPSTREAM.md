# SSE stream parser

- Upstream: https://github.com/4t145/sse-stream
- Package: `sse-stream` 0.3.0, pinned in the gateway and native Cargo manifests.
- Crate SHA-256: `d385160fc0fba84f7b9cc8ef3263e410d0e3ca7ab4927f1356df33145ace69a4`.
- License selected: MIT; the unmodified `LICENSE-MIT` is copied as `LICENSE`.
- Source is consumed unmodified through Cargo, not vendored into this directory.

`SseByteStream` incrementally parses the existing byte transport. Default
`memchr` and `simdutf8` features provide line scanning and UTF-8 validation.
Studio retains authentication, fixed endpoints, transport/event byte limits,
worker-side JSON projection and explicit connection lifecycle. No automatic
request replay or reconnection is provided by this parser.
