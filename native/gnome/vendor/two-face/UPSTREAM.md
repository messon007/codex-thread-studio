# Extended syntax definitions

- Upstream: https://codeberg.org/CosmicHarper/two-face
- Package: `two-face` 0.5.2+bat-0.26.1; pinned through `syntax-highlight`.
- Crate SHA-256: `915be7adc2ff6f4338acbf71f3eda0a146f46003b992a1669c674308610efdac`.
- License selected: MIT; unmodified `LICENSE-MIT` copied as `LICENSE`.
- The bundled definitions are consumed unmodified through Cargo, using the
  `syntect-onig` feature. No runtime grammar downloads are used.

The bundle adds TypeScript, TypeScriptReact, TOML, Dockerfile and other syntax
definitions curated by bat. The Oniguruma bundle includes the Babel/JSX and
other grammars omitted by its pure-Rust regex variant.

`ACKNOWLEDGEMENTS.md` contains the upstream syntax attribution/license texts,
reproduced from the pinned package with its `License::write_md` formatter:

```bash
cargo run -p studio-markdown-native --features syntax-highlight --example syntax_notices --locked > native/gnome/vendor/two-face/ACKNOWLEDGEMENTS.md
```

CI regenerates and compares these notices. Extra theme data is not used; the
reader retains Syntect's InspiredGitHub and base16-ocean.dark themes.
