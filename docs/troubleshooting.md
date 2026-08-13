# Troubleshooting

## Windows Rust build says `link.exe not found`

Install Visual Studio 2022 Build Tools with the C++ desktop workload and Windows SDK, then open a
new PowerShell so the Visual Studio environment can be discovered:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --source winget --accept-package-agreements --accept-source-agreements --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

For slow Cargo downloads, use a temporary local Cargo mirror configuration rather than committing
a source replacement to this repository. `rsproxy.cn` and Tsinghua's sparse registry are common
options. Delete the temporary `CARGO_HOME` configuration after diagnosing the network issue.

## Windows Browser Workspace does not open

Confirm that Microsoft Edge WebView2 Runtime is installed and up to date. Studio must still open
its main WebView before Browser can be selected; an unavailable Codex/OpenCode WSL backend must
show its own backend error and must not prevent Browser Workspace from opening. Browser is hidden
on cold start, while **Exit Browser** deliberately releases Toolbar and page renderers.

The repository intentionally uses the published crates.io WRY release and does not carry a local
WRY fork. Browser actions leave IPC and other WebView2 callbacks before allocating child
controllers. If the first Browser click still appears to do nothing, collect stderr and confirm
the CPU architecture. Native Windows x64 is the currently reviewed target; Windows ARM64 has an
unresolved upstream WRY/WebView2 second-controller deadlock and is not claimed as supported yet.

## Windows installer tool download is slow

`npm run tauri build` produces both MSI and NSIS installers. Tauri verifies downloaded tool hashes,
but a slow GitHub route can still time out before verification. For a local build, configure a
temporary mirror template and clear it afterwards:

```powershell
$env:TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE = 'https://ghproxy.net/https://github.com/<owner>/<repo>/releases/download/<version>/<asset>'
npm run tauri build
Remove-Item Env:\TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE
```

## Windows client cannot start a WSL backend

Studio does not launch Windows Codex/OpenCode installations. Confirm WSL2 and the selected distribution from PowerShell:

```powershell
wsl --list --verbose
wsl -d Ubuntu -- bash -lc 'command -v codex; codex --version'
wsl -d Ubuntu -- bash -lc 'command -v opencode; opencode --version'
```

If a version manager keeps the commands outside the normal login `PATH`, enter their Linux absolute paths in Studio Settings. Studio also checks common NVM and FNM installation directories. Save, close Studio completely, and reopen it. Project directories must be Linux paths such as `/home/rui/project`, not `C:\project` or `\\wsl.localhost\...`.

OpenCode uses Windows-to-WSL localhost forwarding. Update WSL with `wsl --update`; if localhost forwarding is disabled by local policy, enable mirrored networking or restore WSL localhost forwarding before retrying.

## Codex executable is not found

An interactive shell and a desktop launcher often have different `PATH` values. Check:

```bash
command -v codex
codex --version
codex app-server --help
```

Then launch with an explicit path:

```bash
CODEX_THREAD_STUDIO_CODEX_BIN=/absolute/path/to/codex cargo run -p codex-thread-studio
```

Studio also searches NVM, FNM, `~/.local/bin`, and `~/.cargo/bin`.

## App Server starts but initialization fails

Run `codex app-server --stdio` from the same environment and inspect stderr. Confirm that the Codex state directory is writable and the normal CLI is logged in:

```bash
codex login status
ls -ld ~/.codex
```

Sandboxed launchers that make `~/.codex` read-only cannot run the native backend.

## A Thread cannot be resumed

Confirm the Thread is returned by App Server itself. Studio deliberately does not scan rollout JSONL files:

```text
Open Studio backend details → verify connection → refresh the Thread list
```

If the same Thread is actively being written by another Codex CLI process, stop that process before using the native client. Avoid two independent Codex processes controlling one persisted Thread.

## A Turn is waiting forever

Look for an inline approval card. App Server pauses command/file changes until the client responds. Unsupported server-initiated request types are reported as an error instead of being silently ignored.

## Chinese input does not work

The native composer and dialogs are ordinary WebKitGTK textareas; there is no xterm input layer. Test another GTK/WebKit application launched from the same desktop session. If all fail, verify Fcitx5/IBus GTK integration and environment variables. If only Studio fails, capture `compositionstart`, `compositionupdate`, `compositionend`, and `input` events before changing key handling.

## Closing Studio interrupted work

This is a known first-version lifecycle boundary: the stdio App Server child belongs to Studio. Reopen Studio and resume the persisted Thread. Moving to the App Server daemon/control socket is tracked as the next lifecycle milestone.
