# Native regular-expression engine

- Upstream: https://github.com/rust-onig/rust-onig
- Package: `onig_sys` 69.9.3, selected by Syntect's `regex-onig` feature.
- Crate SHA-256: `1e68317604e77e53b85896388e1a803c1d21b74c899ec9e5e1112db90735edd7`.
- Rust wrapper license: MIT; unmodified `LICENSE.md` copied as `LICENSE`.
- The bundled Oniguruma C sources retain their original copyright/license;
  unmodified `oniguruma/COPYING` is included at the same relative path.

The ordinary build compiles and links the bundled engine statically. No
additional Oniguruma runtime installation or generated bindings are needed.
The normal native build already requires a C compiler.
