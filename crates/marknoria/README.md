# MarkNoria fixed shared Web reader package

This package contains the toolkit-free Rust reader, optional HTTP service,
shared Web components, generated locked Web resources, public contracts and
third-party notices from the exact commit in `reader-manifest.json`.
MarkNoria's repository remains private; only the explicit delivery allowlist is
licensed/distributed here. First-party allowlisted code uses MIT. Third-party
licenses and provenance remain in `third-party/`, `licenses/` and bundled notices.

Supported package features: `--no-default-features` for the public Rust reader;
default `web` for the scoped HTTP service. Linux/macOS/Windows consume these same
sources. Platform-native GTK/AppKit/WinUI, FFI and the standalone Tao/Wry shell are
maintained in the original project and intentionally absent from this Web-reader
delivery. This package does not declare those unsupported Cargo features.

Verify a fresh extraction before generating target/node_modules artifacts:

```sh
python3 scripts/verify-reader-package.py .
```

Generated resources are already included. Build without npm or private source:

```sh
cargo build --locked --no-default-features --example reader-host
cargo test --locked --no-default-features --test reader_api --test markdown_profile
cargo build --locked --example embedded-host --bin marknoria-server
cargo run --locked --example embedded-host -- /authorized/document.md
```

Host code calls `marknoria::reader::worker_entry()` before normal startup so its
own supervised workers work without another MarkNoria installation. See
`docs/reader-api.md` and `docs/embedding.md` for grants, defaults, capabilities,
read-only/query/write separation, cancellation and lifecycle. The service grants
one canonical file; the source's parent folder is never implicitly authorized.

To rebuild Web resources from the included locks, use Node/npm and:

```sh
npm ci --ignore-scripts
npm run build
```

`reader-manifest.json` records exact tool/resource versions, source mappings,
derived Cargo lock provenance, file hashes, package feature/platform scope and
license inventory. The verifier checks an exact fresh extraction, including unexpected files.
No package build fetches a private repository or a floating main/latest revision.
Cargo/npm may fetch public locked dependencies; ordinary consumers need no
MarkNoria checkout, AI account or private GitHub token.

Browser component contracts use generated public strings and isolated localhost
hosts. Install Playwright explicitly in your own test tools directory and set
`STUDIO_PLAYWRIGHT_MODULE`/`STUDIO_CHROMIUM_PATH`; see exported test entry points.
`--webkit` uses Python GI/WebKit and the inherited native Wayland session on Linux.
Synthetic fixtures/screenshots/caches are generated under ignored `target/`.

This is a reproducible delivery mechanism, not a claim that downstream Studio has
adopted the package or that every feature-parity/remote-CI acceptance has passed.
Use the final release's compatibility evidence before upgrading a consumer.
