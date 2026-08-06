# Changelog

All notable changes to Codex Thread Studio are recorded here. The project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

- Add switchable Codex App Server and OpenCode Server backends with structured session history, streaming events, native CRUD, file/skill discovery, permissions, and isolated UI state.
- Add a native Windows client that runs Codex and OpenCode exclusively inside a configurable WSL2 distribution, including managed Linux process-group cleanup and WSL file review.

### Fixed

- Keep an active transcript pinned to the latest streaming output across Item completion and Markdown layout changes, while still respecting deliberate upward scrolling.

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

[Unreleased]: https://github.com/messon007/codex-thread-studio/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/messon007/codex-thread-studio/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/messon007/codex-thread-studio/releases/tag/v0.1.0
