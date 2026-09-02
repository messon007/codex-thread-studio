# Ecosystem monitoring

The repository checks its external ecosystem every Monday. It intentionally does not make Studio
query GitHub or show an in-app prompt. A single open GitHub Issue is the review queue when action is
needed; a healthy run closes that Issue after the reviewed baseline catches up.

## What is monitored

| Area | Weekly signal | Result |
| --- | --- | --- |
| Codex CLI | Latest published version and the JSON schemas for App Server methods Studio uses | The workflow creates or updates the review Issue when the version or used contract differs from the reviewed baseline, or when a required method disappears. |
| OpenCode | Latest official Linux baseline CLI version and the generated OpenAPI operations/events Studio uses | The same Issue records version drift, contract drift, missing endpoints/events, and probe failures. |
| JavaScript packages | Registry updates and `npm audit --audit-level=moderate` | Dependabot opens one grouped UI dependency PR; an audit failure is also reflected in the review Issue. |
| Rust crates | Registry updates and `cargo audit` | Dependabot opens one grouped Rust dependency PR; a new vulnerability, informational warning, or yanked crate is also reflected in the review Issue. |
| GitHub Actions | New action revisions | Dependabot opens one grouped workflow PR. |
| Current toolchains | Node 22, Rust stable, browser tests, formatting, and Rust tests | A weekly regression is reflected in the review Issue even when no dependency version changed. |

The App Server check projects only the methods Studio consumes, and the OpenCode check projects only
the routes and event types Studio consumes. Unrelated additions to either upstream schema therefore
do not create noise. A unit test compares literal UI RPC calls with the configured Codex method list
so a newly used method cannot silently escape monitoring.

The workflow is deterministic and does not call an LLM. A human should use the attached contract
projections, release notes, existing integration tests, and a normal session smoke test to decide
whether a runtime update is safe and whether Studio needs an adapter change.

Machine-local/private Codex-compatible launchers do not have a public package feed that GitHub can
discover. Their shared App Server surface is covered by the public Codex contract probe, while their
provider-specific behavior still needs local testing before an upgrade.

`.cargo/audit.toml` records the reviewed warnings already present in the locked Tauri/WebKitGTK
dependency graph and denies every new RustSec warning. When an upgrade removes an ignored advisory,
remove that ID from the file; adding a new ignore requires an explicit applicability review.

## Review lifecycle

`.github/upstream-watch.json` is the last reviewed state, not a minimum supported version. A weekly
run compares the latest official CLIs with that state and uploads `report.json`, `report.md`, and the
two projected contracts as a workflow artifact.

The Issue distinguishes these cases:

- Version changed but the used contract did not: consider the runtime update and run a smoke test.
- Used contract changed but every requirement is present: inspect the projection and decide whether
  the adapter or fixtures need an update.
- A required contract is missing or a probe fails: do not accept the runtime until the incompatibility
  or monitor is understood.
- Tests or security audits fail: inspect that workflow run even when upstream contracts are unchanged.

After reviewing and testing the installed versions, update the baseline from the repository root:

```bash
npm run monitor:upstreams -- accept
git diff -- .github/upstream-watch.json
```

Commit the accepted baseline together with any required adapter, fixture, or documentation changes.
The next healthy scheduled run closes the existing review Issue. Do not edit a hash by hand unless
it was generated and compared from the exact accepted binaries.

To inspect without accepting anything:

```bash
npm run monitor:upstreams
```

Use `CODEX_THREAD_STUDIO_CODEX_BIN` and `CODEX_THREAD_STUDIO_OPENCODE_BIN`, or the corresponding
`--codex-bin` and `--opencode-bin` options, when the desired executables are not on `PATH`.

GitHub Actions, Issues, and Dependabot must be enabled for the repository. The workflow grants write
access to Issues only to its final notification job; jobs that install or execute upstream packages
have read-only repository permissions and receive no repository credential.
