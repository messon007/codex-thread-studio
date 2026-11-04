# Native math renderer

The native workspace pins RaTeX 0.1.15 (`ratex-parser`, `ratex-layout`,
`ratex-types`, `ratex-svg` and its locked dependencies), upstream commit
`7d9f9a39218914bead8ffdf39b79028a01d12c22`:
https://github.com/erweixin/RaTeX

Cargo downloads the published source; no modified renderer fork is vendored.
The `embed-fonts` feature retains KaTeX glyph bytes in the native executable,
and the worker emits standalone glyph-outline SVG. System fonts may supply CJK
fallback glyphs. No CSS, JavaScript, browser runtime or TeX installation is needed.

RaTeX is MIT licensed (`LICENSE`). Its embedded KaTeX font binaries use the SIL
Open Font License 1.1 (`OFL.txt` and `FONT_NOTICE.txt` copied unchanged from the
published `ratex-katex-fonts` package). The native bundle carries these notices.

The math worker is a private mode of the native executable. It runs before GTK,
gateway or user-profile initialization. Input is bounded to 16 KiB of formula
source, with a 2 MiB SVG cap, a two-second CPU limit, a three-second parent wall
timeout, a 256 MiB data limit and no core dumps. Environment variables are cleared.
The shared diagram lane/cache owns cancellation and stale-result handling;
SVG image resolvers retain their existing file/network denial.

Selection evidence and a reproducible two-engine comparison are recorded in
`benchmarks/native-math` and `docs/gnome-native.md`. This does not claim full TeX
or browser Markdown/Mermaid math parity. Inline math layout remains pending.
