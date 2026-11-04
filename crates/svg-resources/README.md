# Bounded SVG bitmap resources

`Resources` supplies an embedded-only `usvg::ImageHrefResolver` for existing
`resvg` callers. Run SVG parsing and rendering on a bounded Rust worker. Keep
your own SVG input, XML/element, output-pixel and admission limits.

```rust
let resources = studio_svg_resources::Resources::default();
let options = resvg::usvg::Options {
    image_href_resolver: resources.resolver(),
    ..Default::default()
};
let tree = resvg::usvg::Tree::from_str(svg, &options)?;
resources.check()?; // Reject a resource-budget failure before painting.
```

PNG, JPEG, GIF and WebP data resources are decoded with the `image` crate's
allocation/dimension limits. JPEG orientation is applied. A bounded streaming
PNG encoder supplies canonical RGBA PNGs to resvg; the renderer never receives
the original compressed resource or its metadata. GIF/WebP use still images.
Identical resources share canonical bytes within one parse; every occurrence
still counts toward rendering work.

Limits per SVG parse are 64 resource attempts, 16 megapixels across bitmap
occurrences, 32,768 pixels per bitmap side, and 8 MiB of canonical PNGs across
unique admitted resources. Each decoder receives a 128 MiB allocation limit;
codec-specific internal allocations remain outside that accounting. These
limits are not a total-process RSS bound. Invalid or
unknown image bytes are omitted; a resource-budget breach rejects the SVG.
Call `check()` even when parsing succeeds.

String hrefs (files, relative paths, URLs), nested SVG resources and SVGZ are
not resolved. The caller retains the original SVG bytes and its source identity
for Save; normalization affects only temporary rendering resources. The crate
does not read files, open sockets, start processes, or depend on GTK.

The implementation uses the pinned
[usvg resource hook](https://docs.rs/usvg/0.48.1/usvg/struct.ImageHrefResolver.html)
and [resvg renderer](https://docs.rs/resvg/0.48.1/resvg/), with the existing
`image` 0.25.10 decoders.
