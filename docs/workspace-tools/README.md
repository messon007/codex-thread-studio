# Right Workspace tools

Codex Thread Studio keeps Chat as the primary surface. Files, Document, Terminal,
Map, Comments, Favorites, and Browser share one mutually exclusive right-side slot.
Opening a tool replaces the visible tool without destroying its underlying state.

## Interaction contract

| Action | Result |
| --- | --- |
| Select a different session | Hide the visible session tool; do not start Files or Terminal automatically. |
| Open Files | Lazily read only the selected session directory. |
| Open Terminal | Show a disconnected state. A PTY is created only after **Start terminal**. |
| Open another right-side tool | Hide the previous tool and preserve its state. |
| Close the right rail | Hide it. A running terminal remains alive. |
| Stop inside Terminal | Close the WebSocket, terminate the PTY shell, and dispose xterm. |
| Open a text file | Reuse Document with Preview/Source/Edit where applicable. |

Files and Terminal are session-scoped. Two sessions that use the same directory do
not share a terminal process.

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

## Terminal transport

The browser component is xterm.js. The Rust backend uses `portable-pty` and a
gateway-authenticated `/ws/terminal` WebSocket:

- binary client frames: UTF-8 terminal input;
- binary server frames: raw PTY output;
- text frames: start, resize, stop, ready, error, and exit control messages.

The PTY starts in the selected session directory with `TERM=xterm-256color` and
`COLORTERM=truecolor`. Closing the WebSocket terminates the shell; hiding the right
rail deliberately keeps it connected.

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

## Third-party components

CodeMirror, xterm.js, its Fit addon, esbuild, and portable-pty are MIT-licensed.
Their versions are pinned by `package-lock.json` and `Cargo.lock`; generated browser
bundles are committed so production startup does not depend on npm.
