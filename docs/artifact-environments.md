# Artifacts and project environments

## Artifact readers

- PDF files up to 50 MiB are signature-checked and parsed locally with PDF.js. The reader supports page navigation, per-page search highlighting, text comments, and Shift-drag rectangular region comments. Anchors retain the file, content hash, page, and normalized rectangles.
- CSV/TSV are parsed locally with quoted-field handling. XLSX files up to 50 MiB are ZIP-signature-checked and parsed with ExcelJS. Rendering is capped at 10,000 rows and 200 columns per sheet.
- The chart view chooses the first sufficiently numeric column and renders up to 100 bars as local SVG. It is an exploratory preview, not a spreadsheet calculation engine.
- Parser code and licenses are committed under `ui/vendor`; documents do not execute macros or fetch CDN resources.

## Environment profiles

Profiles are keyed by the canonical project root. They contain:

- visible environment variables;
- write-only Secret updates and separately removable Secret names;
- `restricted` or `enabled` network policy and allowed-host metadata;
- cache environment variables such as `CARGO_HOME` or `NPM_CONFIG_CACHE`.

Open a session's **More → Project environment** action to edit its project profile. The dedicated dialog always shows the exact project name and root being edited. Variables and write-only Secret updates are primary controls; network, allowed-host, and cache settings live under the advanced section. A configured indicator appears beside the menu action for the selected project.

The Rust gateway owns storage and returns only Secret names. On Unix the environment store uses mode `0600`. Values are applied to newly started Studio PTYs and to Codex with `thread/resume.config.shell_environment_policy`. The selected profile's network policy sets `sandboxPolicy.networkAccess` on subsequent Codex Turns. Allowed-host entries are preserved for compatible remote/sandbox providers; the current local Codex sandbox exposes network as a boolean and cannot enforce a host allowlist by itself.

Changing a profile does not mutate an already-running PTY; stop and restart the Terminal to receive the new environment.
