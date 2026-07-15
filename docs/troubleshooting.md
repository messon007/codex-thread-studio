# Troubleshooting

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
