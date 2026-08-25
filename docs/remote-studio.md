# Remote Studio over SSH

Status: experimental prototype on `feature/ssh-remote-studio`. This is a test-only branch and is
not intended to be merged into `main` until the remote workflow is evaluated further.

## Goal

Use a Windows browser to operate sessions and workspaces that remain on a Linux machine. The
prototype follows the remote-instance model: the complete Studio gateway runs beside the Linux
workspace, while SSH provides authentication, encryption, and port forwarding.

```text
Windows browser
  └─ http://127.0.0.1:38080
      └─ SSH local port forwarding
          └─ Linux 127.0.0.1:38080
              └─ codex-thread-studio --serve
                  ├─ Codex App Server processes
                  ├─ OpenCode Server
                  ├─ workspace file readers and writers
                  ├─ Git operations
                  └─ PTY terminals
```

No Linux project directory is mounted or synchronized on Windows. Text responses use the
existing JSON APIs. Images, PDF, EPUB, and spreadsheet bytes use the existing bounded document
responses. Terminal and Codex traffic use the existing WebSocket routes.

## Choose a runtime mode

The same executable supports both modes. Start the normal local desktop UI without server
options:

```bash
./target/release/codex-thread-studio
```

Start the headless remote server with `--serve`:

```bash
./target/release/codex-thread-studio --serve --listen 127.0.0.1:38080
```

`127.0.0.1:38080` is also the default when `--listen` is omitted. Do not change the listener to
`0.0.0.0`; SSH is the only intended network boundary.

Do not run both modes concurrently with the same user configuration. They share Studio JSON
preferences, which do not yet have a cross-process lock.

## Connect from Windows

Build and start the Linux executable:

```bash
cargo build -p codex-thread-studio --release
./target/release/codex-thread-studio --serve --listen 127.0.0.1:38080
```

The Linux terminal prints the tokenized browser URL. Leave that process running. From Windows
PowerShell, create the SSH tunnel in a second terminal:

```powershell
ssh -N -L 38080:127.0.0.1:38080 user@linux-host
```

After authentication succeeds, leave the PowerShell window running and open the URL printed by
the Linux server in the Windows browser. It looks like:

```text
http://127.0.0.1:38080/?token=0123456789abcdef0123456789abcdef
```

Keep the SSH process running and open that URL on Windows. The query supplies the random gateway
credential to the page and is removed from the address bar immediately after bootstrap. A
`#token=...` fragment is also accepted when opening the URL manually.
Closing PowerShell stops the tunnel but does not stop a server that was started separately on
Linux. Stop the Linux server with `Ctrl+C` when it is no longer needed.

If Windows reports `bind [127.0.0.1]:38080: Permission denied`, check its reserved port ranges:

```powershell
netsh interface ipv4 show excludedportrange protocol=tcp
```

Choose a port outside those ranges and use exactly the same port in the Linux `--listen` argument,
the Windows `-L` mapping, and the browser URL.

The remote login shell must be able to find `codex` and `opencode`. If it cannot, configure their
absolute paths with the existing `CODEX_THREAD_STUDIO_CODEX_BIN` and
`CODEX_THREAD_STUDIO_OPENCODE_BIN` environment variables in the remote command or shell profile.

## Security boundary

- Server mode accepts only IP loopback listeners. It rejects `0.0.0.0`, public addresses, and
  LAN addresses.
- SSH owns host authentication and transport encryption. Normal OpenSSH host-key verification
  remains in effect.
- Studio still requires its random bearer credential for HTTP and its authenticated WebSocket
  subprotocol. Knowing the forwarded port alone is insufficient.
- Existing workspace-root confinement, file-size limits, UTF-8 checks, Git path validation, and
  terminal root validation continue to run on Linux.
- Use the same forwarded port on Windows and Linux. Studio rejects unexpected Host and Origin
  values to reduce local gateway attacks.

## Prototype limitations

- This slice uses the system browser on Windows; it does not yet add an SSH connection dialog to
  the native Tauri client.
- The native Embedded Browser Workspace is unavailable. URL resources open in a normal browser
  tab; document and workspace resources remain inside Studio.
- Remote deployment, version negotiation, reconnect, SSH ControlMaster reuse, and saved host
  profiles are not automated yet.
- Do not run desktop and server modes concurrently against the same preference files. SQLite
  stores tolerate multiple processes, but JSON preference updates do not yet have a cross-process
  lock.

## Next slice

After this transport and workspace behavior is accepted, the Windows Tauri client can add a
compact Remote workspace dialog. It should start the same SSH tunnel, launch or update the Linux
server, capture its one-time URL, and load it without exposing SSH or gateway secrets to the web
content.
