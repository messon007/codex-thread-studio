# Fixed-source reader delivery

Export reviewed immutable reader sources/resources independently of any active
checkout. The archive includes its exact inventory, derived locks, public build
entry points and original third-party notices. This workflow supports a fixed
consumer pin; it does not certify downstream UI or operating-system acceptance.

## Export from immutable source

This section runs in the upstream MarkNoria repository, which owns the exporter
and distribution policy. Package consumers start at [Consume and verify](#consume-and-verify);
their packaged policy is `DISTRIBUTION.md`.

The owner approved MIT for allowlisted first-party delivery code. The repository
as a whole remains private/unlicensed for redistribution outside that allowlist.
See `distribution/POLICY.md`. `distribution/reader-allowlist.json` lists exact
source-to-package paths, rather than copying a working tree or broad directories.
Review/update it when adding a required module, fixture or notice.

```sh
# Use a full immutable commit that contains the matching export script/templates.
python3 scripts/export-reader.py --revision FULL_40_CHARACTER_SHA \
  --output target/reader-deliveries/reader.tar.gz \
  --npm-cache target/reader-deliveries/npm-cache
# Repeat from the same revision/tool versions using only cached public packages:
python3 scripts/export-reader.py --revision FULL_40_CHARACTER_SHA \
  --output target/reader-deliveries/reader-repeat.tar.gz \
  --npm-cache target/reader-deliveries/npm-cache --offline
python3 tests/server/reader-export-check.py \
  target/reader-deliveries/reader.tar.gz \
  target/reader-deliveries/reader-repeat.tar.gz
```

The script reads source blobs using `git show`, checks its own bytes against the
requested revision and builds in a new private directory beside the output.
No active node_modules/vendor/build output or sibling checkout is used. For an
older revision whose tool differs, obtain that revision's script with `git show`
and execute it from the repository; it still reads every template/blob from the
specified commit. Output packages are never overwritten.

The delivery manifest substitutes explicit Web-only Cargo/lib/package templates.
Their dependency versions/options must match the original source. Cargo performs
full resolution (not `metadata --no-deps`), canonically pruning the locked graph;
every resulting package/version/checksum must exist in the original lock.
Locked headless/default/all-feature metadata resolution is checked (metadata
does not compile mutually exclusive profiles). Executables use an explicit full
or reduced syntax profile; see [syntax profiles](highlighting.md#build-time-syntax-profiles). No GTK/AppKit/WinUI,
Tao/Wry shell, FFI or shared native controller is exported. Npm uses the exact
source lock, empty user/global configs and disabled install scripts; all registry
URLs must be public. The resource build cannot modify allowlisted source files.

Exports retain source mappings/hashes, original/derived lock hashes, API/bridge/
capability versions, resource/tool versions, platform/feature scope and license
inventory. Installed locked npm LICENSE/COPYING/NOTICE/README files and declared
license/author/provenance are retained. Rust registry dependencies retain their
locked public sources/notices when fetched. Generated resources come only from
the locked isolated build. Private documents/fixtures/screenshots/caches and
credentials are excluded. The archive is sorted, normalized to fixed ownership/
modes/commit mtime, gzip timestamp zero, and streamed with an exclusive atomic
output. Inventory is limited to 8,192 files, 128 MiB/file and 512 MiB total.

## Consume and verify

A fresh extraction contains `reader-manifest.json`, a strict verifier and a
consumer README. Verify before creating target/node_modules/git artifacts:

```sh
python3 scripts/verify-reader-package.py .
cargo test --locked --offline --no-default-features \
  --test reader_api --test markdown_profile
cargo build --locked --offline --example reader-host \
  --example embedded-host --bin marknoria-server
```

`--offline` requires cached public Cargo packages. Otherwise allow Cargo to fetch
those exact locked public dependencies. No private token or MarkNoria checkout is
required. Generated Web resources are included, so normal Rust builds need no
npm step. Rebuilding resources uses the included npm lock/build scripts. Native
UI/FFI/features are absent from this package rather than optional hidden product
requirements. See the consumer README and public API/component docs.

The verifier checks exact file inventory/content and rejects symlinks, changed
files and unexpected extras. An external consumer must pin the archive SHA-256
from its reviewed update; an internal manifest is an integrity inventory, not a
signature or proof of trusted authorship. Ordinary Studio builds must use the
reviewed fixed package and never fetch private main/latest or another checkout.

The package owns a scoped `.gitattributes` with `* -text`: consuming Git checkouts
must preserve its reviewed bytes, including original third-party notice line
endings. The export contract stages every packaged file with `core.autocrlf=true`
and checks all Git blob hashes against the inventory. Formatting applies to host
code separately; reformatting the vendor tree invalidates its manifest.


## Consuming workspaces

The public library permits compatible Cargo patch releases of `serde_json` and
`sha2`; standalone/export locks still fix their own resolution. A consumer keeps
and validates its own Cargo.lock. Path dependencies do not inherit their own
Cargo configuration into the consumer: preserve the public package's target
MSVC `/utf-8` C/C++ flags in the consuming workspace as described by its README.

Verify before generating target/node_modules files, pin the reviewed archive
SHA-256 externally, and import its contents unchanged. Update consumers only after
running the relevant shared contracts and their own entry-point/platform tests.
Keep the source revision, hash and results with the release/PR and generated
artifacts; do not create a new documentation file for each export or issue.
