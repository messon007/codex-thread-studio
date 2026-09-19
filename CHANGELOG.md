# Changelog

All notable changes to Codex Thread Studio are recorded here. The project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Document macOS first launch for quarantined browser downloads, including the `curl` install path that never sets the quarantine attribute.
- Link the macOS first-launch steps from the GitHub release body and document the optional Developer ID and notarization secrets in the release guide.

### Changed

- Ad-hoc sign the macOS bundle so a release ships a sealed app signature without a Developer ID account.
- Export Apple signing and notarization credentials during releases when they are configured, and enable the macOS hardened runtime that notarization requires.
- Search `/opt/homebrew/bin` and `/usr/local/bin` on macOS so a Finder or Launchpad launch finds Codex and OpenCode installed through Homebrew, the Node.js installer, or a similar prefix.

### Fixed

- Restore Open Item Location on macOS, which previously reported that the system file manager was unavailable because the route reused the embedded-browser capability flag.

## [0.3.0] - 2026-09-17

### Added

- Add hierarchical Subagents navigation with parent/child session relationships and direct session opening.
- Add paginated Git commit history with per-commit changed-file lists and read-only unified diffs.
- Add a read-only SQLite workspace for CSV and TSV files, including editable projected columns, filtering, sorting, compact/expanded SQL layouts, and bounded query execution.

### Changed

- Improve delimited-table reading with automatic comma, tab, and semicolon detection, adjustable columns, full-cell viewing, width-aware wrapping, and grouped table actions.
- Improve Router completion reminders, target-session navigation, acknowledgement persistence, backend prompts, and compact attention layout.
- Show manual shell output directly while preserving agent activity grouping.

### Fixed

- Keep right-side Document, Terminal, Git Review, and other workspace panels mutually exclusive during asynchronous loading.
- Keep Markdown links with source line locations in Preview by mapping them to the nearest heading, and use the same mapping when switching manually from Source.
- Restore per-session workspace state and document comment markers across session changes.
- Keep Windows WebView2 and Linux WebKitGTK integration compatible with current dependencies.

## [0.2.1] - 2026-09-07

### Added

- Add inline Router responses with source-aware workspace actions, persistent delivery history, and a compact automatic/session routing selector.
- Add opt-in, running-task supervision for Codex-family sessions, with a task-scoped wand, cancellation, bounded continuation, and independent Router targets.
- Add AI-maintained Session Maps with incremental updates, simplified controls, and one current node.
- Add an optional, independently compiled `windows-native` backend fallback; Windows still defaults to WSL. Native CLI discovery and managed process cleanup live in a separate module, with both build modes covered by Windows CI.
- Add structured Codex user-input questions and MCP elicitation forms, reconnect reconciliation, declared client capabilities, desktop completion/input notifications, and generated-protocol compatibility fixtures.
- Add searchable PDF reading with text and Shift-drag region comments, plus CSV/XLSX grids and local bar charts.
- Add per-project environment profiles with redacted local Secrets, Codex and Terminal environment injection, network policy, allowed-host metadata, and cache variables.
- Add local PDF.js and ExcelJS bundles with license files and dependency audit coverage.
- Add a Git Review rail for code, Markdown, CSV, and other text changes with staged/unstaged filters, line-numbered unified diffs, and safe single-file stage/unstage actions.
- Start a Windows 11 native Embedded Browser Workspace based on WRY child WebViews and system
  WebView2: lazy global runtime, separate trusted Toolbar/page contexts, persistent profile,
  tabs, native splitter, download routing, URL policy and default-deny page permissions. The
  supplied Windows x64 branch's Debug/Release and MSI/NSIS results are recorded in
  `docs/browser-workspace/windows-acceptance.md`; the official-WRY integration still needs a
  native Windows rerun.

- Add switchable Codex App Server and OpenCode Server backends with structured session history, streaming events, native CRUD, file/skill discovery, permissions, and isolated UI state.
- Add a native Windows client that runs Codex and OpenCode exclusively inside a configurable WSL2 distribution, including managed Linux process-group cleanup and WSL file review.
- Render fenced Mermaid diagrams in chat messages, Markdown document previews, and favorites using a locally vendored, strict-mode renderer with configurable appearance and source fallback.
- Add configuration-file-only Markdown reading, technical, and compact presentation modes; render fenced plain text without code-language or copy chrome.

### Fixed

- Refresh Composer controls when backend readiness changes, including cached session restores; guard sending and continuation against stale button state.
- Align Router extension actions after native response actions, restore direct image attachment, and keep supervision controls inside the compact action menu.
- Sanitize repository example settings and design assets; relocate retained acceptance captures under docs/design and remove obsolete screenshots and the temporary EPT patch.
- Align Files, Terminal, Git Review, Document, Map, Comments, Favorites, and embedded Browser on one persisted rail width; centralize WebView rail dragging with a whole-window selection shield; add dedicated Workspace typography settings; and return to the originating workspace tool after closing an opened document.
- Remove phantom CSV rows caused by trailing newlines, keep compact charts usable in the right rail, and make their action button reflect the active Grid/Chart view.
- Keep the project-environment settings section reachable in shorter windows and capture the Studio WebView reliably during Linux screenshot acceptance.
- Keep an active transcript pinned to the latest streaming output across Item completion and Markdown layout changes, while still respecting deliberate upward scrolling.
- Defer Windows Browser actions out of WebView2 IPC, popup, shortcut and renderer-failure callbacks
  before creating lazy child WebViews.
- Restore cross-platform CI by accepting Windows line endings in version metadata, limiting the Linux-specific WSL script test to Linux, and updating DOMPurify.

## [0.2.0] - 2026-07-20

### Added

- Mature structured Codex composer with file search, skill selection, slash commands, permission controls, and local shell mode.
- Sanitized GitHub-flavored Markdown rendering, streaming Item updates, selection comments, and persisted typography preferences.
- In-session Turn navigator, richer Thread lifecycle operations, and App Server process diagnostics.
- SemVer validation, in-app version display, changelog workflow, and tag-driven desktop release builds.

### Changed

- Promoted Codex Thread Studio from an App Server prototype to an independent daily-use Codex client.

## [0.1.0] - 2026-07-15

### Added

- Initial Tauri desktop shell and structured Codex App Server v2 integration.

[Unreleased]: https://github.com/messon007/codex-thread-studio/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/messon007/codex-thread-studio/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/messon007/codex-thread-studio/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/messon007/codex-thread-studio/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/messon007/codex-thread-studio/releases/tag/v0.1.0
