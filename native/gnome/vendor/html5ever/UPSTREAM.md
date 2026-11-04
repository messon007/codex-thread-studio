# html5ever text tokenizer

The Rust gateway's EPUB book search links `html5ever` 0.26.0, pinned to the
version already present in the workspace lockfile. It uses the streaming
HTML tokenizer to decode character references and collect bounded section text;
no browser, JavaScript runtime or DOM tree is constructed for book search.

Upstream: https://github.com/servo/html5ever
API: https://docs.rs/html5ever/0.26.0/html5ever/tokenizer/index.html
Crate: https://crates.io/crates/html5ever/0.26.0

The crate is offered under MIT or Apache-2.0. This distribution chooses MIT;
`LICENSE` is the unmodified notice shipped in that crate. The implementation is
compiled through Cargo, not copied into this directory.
