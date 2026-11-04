# CSS font-stack parser

The shared Rust gateway uses the unmodified `cssparser` 0.27.2 crate to validate
CSS font family lists before producing native font names. It is distributed
under MPL-2.0; the accompanying LICENSE is copied from its crates.io package.

Source: https://github.com/servo/rust-cssparser
Package: https://crates.io/crates/cssparser/0.27.2

No CSS engine, network fetcher, or application frontend is embedded by this dependency.
