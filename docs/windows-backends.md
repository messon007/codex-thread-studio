# Windows backend modes

The desktop UI is a native Windows application in both modes. The distinction is
where Codex, configured Codex backends, and OpenCode execute.

## Default: WSL

The default build and default runtime use WSL. Existing `wslDistribution`,
`wslUser`, `wslCodexBinary`, and `wslOpencodeBinary` settings remain supported.
Install/authenticate the backend CLIs inside WSL. No native fallback occurs
silently when WSL fails.

## Optional fallback: native Windows

For machines without WSL, build on Windows with:

```powershell
npm ci --ignore-scripts
npm run build:ui
cargo build --release --locked --features windows-native
$env:CODEX_THREAD_STUDIO_WINDOWS_BACKEND = 'native'
.\target\release\codex-thread-studio.exe
```

To package installers, use `npm run tauri -- build --features windows-native`.
An installer built with this feature still uses WSL unless explicitly selected.
Remove the environment variable or set it to `wsl` to return to WSL mode.
A WSL-only build rejects a request for native mode with a clear startup error.

Native discovery, npm `.cmd`/`.bat` shims, and Windows Job Object cleanup live in
`src-tauri/src/windows_native.rs`, excluded from ordinary builds. Batch arguments
use Rust's standard escaping; no manually assembled `cmd.exe /C` string is used.

Install and authenticate Windows-native versions of the backend CLIs. Discovery
uses PATH and common npm, NVM, Bun, Cargo, and WinGet directories. Optional
`CODEX_THREAD_STUDIO_CODEX_BIN` and `CODEX_THREAD_STUDIO_OPENCODE_BIN` overrides
select native executables. Configured backend commands must also exist on Windows.

### Limitations

- Windows and WSL installations have separate CLI configuration, authentication,
  session storage, and filesystem paths. Switching modes does not migrate sessions
  or rewrite persisted session links, favorites, model selections, or Router IDs.
  Use separate Studio profiles (`XDG_CONFIG_HOME`) when evaluating both modes.
- A Linux project path or shell script is not automatically translated or run
  through WSL. Projects, shared-document directories, CLI wrappers, Git, and shell
  commands must work on native Windows.
- Tool availability and sandbox behavior depend on each upstream Windows CLI.
  Enabling this feature does not make Linux-only agent commands portable.
- Batch arguments that Rust cannot safely escape fail rather than being passed
  to a shell unescaped. Prefer an `.exe` or Node entry point for complex wrappers.
- The first historical `pre-typescript` release uses WSL only; it does not contain
  this later integration.

CI runs both the default and `windows-native` Rust suites on Windows, including
native shim startup and managed-process shutdown. Linux/macOS retain their existing
runtime and UI code paths.
