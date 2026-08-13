# Windows native implementation

The Windows client is a native Tauri application. Its AI backends still run inside WSL2; “native”
does not mean that Studio discovers or launches Windows Codex/OpenCode executables.

## Windows-specific code

- `src-tauri/src/embedded_browser_windows.rs` owns the same-window WebView2 Browser Workspace:
  child WebViews, HWND layout, native splitter and menu, downloads, permissions, renderer recovery,
  and WebView2 callbacks.
- `src-tauri/src/main.rs` selects that module only under `cfg(windows)`. Linux continues to select
  `embedded_browser.rs`; other platforms use the ordinary Tauri WebviewWindow path.
- `src-tauri/Cargo.toml` keeps `tao`, `webview2-com`, `windows-sys`, and Windows WRY usage under
  `[target.'cfg(windows)'.dependencies]`. GTK/WebKitGTK remain Linux-target dependencies.
- `backend_runtime.rs`, `codex_app_server.rs`, and `opencode_server.rs` implement the existing
  Windows-to-WSL backend boundary. Browser support does not change that process model.
- `ui/windows-browser-platform.test.mjs` and the Windows assertions in the toolbar/sidebar tests
  protect platform presentation and layout contracts without making Linux execute Win32 code.

Shared files such as `browser_runtime.rs`, `ui/app.js`, and the Browser HTML contain platform-neutral
state, validation, and actions. Windows-only branches must remain compile-time guarded or driven by
the trusted `hostPlatform` value supplied by Rust.

## Dependency policy

The repository uses the published crates.io WRY version selected by the Tauri dependency graph. It
must not vendor the WRY repository or add a workspace-wide `[patch.crates-io]`: that silently changes
the Linux runtime too and leaves the project responsible for maintaining a browser engine fork.

The reviewed Windows code defers actions out of WebView2 IPC, popup, shortcut, and failure callbacks
before it creates another child WebView. WRY still has an unresolved upstream ARM64 deadlock when
creating a second WebView2 controller. Therefore the current support claim is Windows 11 x64 only;
ARM64 must not be marked supported until an upstream release contains a reviewed fix and native
acceptance passes. See [WRY #1665](https://github.com/tauri-apps/wry/issues/1665) and the still-open,
changes-requested [WRY PR #1666](https://github.com/tauri-apps/wry/pull/1666).

## Native verification

Run Windows verification on Windows, not by changing the Linux development machine into a Windows
cross-compilation environment:

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm test
npm run tauri build
```

Then repeat the interaction checklist in
[`browser-workspace/windows-acceptance.md`](browser-workspace/windows-acceptance.md), especially the
first lazy open, multiple tabs, popup conversion, repeated splitter drags, IME input, Hide/Exit,
renderer cleanup, and restart persistence. Record CPU architecture alongside every result.
