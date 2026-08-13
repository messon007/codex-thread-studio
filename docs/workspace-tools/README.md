# Right Workspace tools

Codex Thread Studio keeps Chat as the primary surface. Files, Git Review, Document, Terminal,
Map, Comments, Favorites, and Browser share one mutually exclusive right-side slot.
Opening a tool replaces the visible tool without destroying its underlying state.

## Interaction contract

| Action | Result |
| --- | --- |
| Select a different session | Hide the visible session tool; do not start Files or Terminal automatically. |
| Open Files | Lazily read only the selected session directory. |
| Open Terminal | Show a disconnected state. A PTY is created only after **Start terminal**. |
| Open Review | Read the selected session's Git status, show staged/unstaged text diffs, and preserve the selected file and scope per session. |
| Open another right-side tool | Hide the previous tool and preserve its state. |
| Close the right rail | Hide it. A running terminal remains alive. |
| Stop inside Terminal | Close the WebSocket, terminate the PTY shell, and dispose xterm. |
| Open a supported file | Reuse Document for text editing, image/EPUB preview, PDF review, or CSV/XLSX data exploration. |
| Close a document opened from Files or Review | Return to the originating Files or Git Review view with its selection and expanded-directory state preserved. |

Files and Terminal are session-scoped. Two sessions that use the same directory do
not share a terminal process.

Files, Terminal, Git Review, Document, Map, Comments, Favorites, and the embedded Browser
use one persisted responsive rail width: 44% of the session workspace by default and up
to 65% when dragged. Switching tools or opening/closing a document preserves the last
user-selected width. A full-window drag shield prevents text selection and embedded-document
input while resizing WebView rails. Browser retains its native GTK divider and synchronizes
the same width through the narrow Studio action bridge.

Settings expose a dedicated Workspace font family and size for Files and Review chrome.
Diffs and Terminal use the separately configurable code/diff/terminal font, size, and
weight. Existing preferences migrate to the new defaults automatically.

## Git Review

- Status uses NUL-delimited Git porcelain records so paths with spaces and rename sources remain unambiguous.
- The file list covers staged, unstaged, overlapping, untracked, renamed, deleted, and conflicted paths. Filters never mutate Git state.
- Unified diffs are capped at 4 MiB, rendered with old/new line numbers, and retain horizontal scrolling for source and CSV rows. Binary changes show a non-text state.
- **Stage file** and **Unstage** operate only on a selected path from the current status response. Commands use argument arrays and literal pathspecs, never a shell.
- The UI deliberately has no discard/revert action. Hunk staging, token syntax highlighting, side-by-side comparison, and line comments remain future work.

## Files and editing

- Directory enumeration is lazy and non-recursive; a response is capped at 500
  entries.
- Every directory and file is canonicalized. Paths and symlinks that escape the
  session root are rejected or omitted.
- Text editing is UTF-8 and limited to 5 MiB. Images remain preview-only.
- Opening a file records an FNV-1a content hash. Save compares the hash with the
  current disk version and returns HTTP 409 on conflict.
- A normal save writes a same-directory temporary file, preserves permissions,
  flushes it, atomically renames it, and synchronizes the parent directory on Unix.
- CodeMirror and its Markdown support are loaded only when Edit is opened.
- PDF.js and ExcelJS are loaded only for matching artifacts. PDF supports search, text selection, and Shift-drag region comments. CSV/XLSX use bounded 10,000-row/200-column grids and a local numeric bar chart.

## Terminal transport

The browser component is xterm.js. The Rust backend uses `portable-pty` and a
gateway-authenticated `/ws/terminal` WebSocket:

- binary client frames: UTF-8 terminal input;
- binary server frames: raw PTY output;
- text frames: start, resize, stop, ready, error, and exit control messages.

The PTY starts in the selected session directory with `TERM=xterm-256color` and
`COLORTERM=truecolor`, then receives the selected project's variables, Secrets, and cache variables from the Rust-owned environment store. Closing the WebSocket terminates the shell; hiding the right
rail deliberately keeps it connected.

Secret values are stored outside general preferences and GET responses expose only names. On Unix the store is written with mode `0600`. This is local file protection, not hardware-backed encryption. Saving a Codex project profile applies the same values through `thread/resume.config.shell_environment_policy`; the UI never receives stored Secret values.

The shared UI and protocol are cross-platform. The provider uses a user shell on
Linux/macOS and ConPTY through `portable-pty` on Windows. Native Windows and WSL
packaging still require platform-specific CI and manual acceptance testing.

## Performance

- Neither a directory request nor PTY process is created at application startup.
- CodeMirror (about 595 KiB minified) and xterm (about 337 KiB minified) are separate
  bundles and are dynamically imported on first use.
- File trees retain only directories the user expanded.
- Each session retains its detached xterm DOM node and WebSocket while hidden;
  **Stop terminal** releases them.

## Acceptance checklist

The Git Review and artifact/environment checks completed on Linux are recorded in the
[2026-08-13 acceptance report](../acceptance-2026-08-13.md). Platform-specific Windows
packaging and interactive terminal/IME checks remain separate release acceptance work.

- [ ] Chat and the session list remain visible while any right-side tool is open.
- [ ] Files and Terminal use only the selected session directory.
- [ ] Expanding a directory does not recursively scan descendants.
- [ ] Unsupported files remain visible but cannot be opened.
- [ ] Markdown supports Preview, Source, Edit, Save, and `Ctrl/Cmd+S`.
- [ ] External modification produces a conflict prompt before overwrite.
- [ ] Terminal starts only after explicit user action and `pwd` reports the session directory.
- [ ] ANSI color, resize, Chinese IME input, and interactive terminal programs work.
- [ ] Hiding Terminal preserves it; Stop terminates it.
- [ ] Switching sessions never displays another session's terminal.
- [ ] Review distinguishes staged, unstaged, overlapping, and untracked files and can switch each available diff scope.
- [ ] Stage/Unstage updates the file badges and diff without discarding working-tree content.

## Third-party components

CodeMirror, xterm.js, its Fit addon, esbuild, and portable-pty are MIT-licensed.
Their versions are pinned by `package-lock.json` and `Cargo.lock`; generated browser
bundles are committed so production startup does not depend on npm.
