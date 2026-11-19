# WebKit internal resource-load error investigation

2026-11-19, feat/agent-teams. The user initially noticed this log when closing
the titlebar after normal operation, then clarified that it may have been printed
earlier during use. Timing is therefore unconfirmed:

```
ERROR: WebKit encountered an internal error. This is a WebKit bug.
WebLoaderStrategy.cpp: internallyFailedLoadTimerFired()
```

The preceding session.lifecycle thread/status/changed notification is an ordinary
background session event, not an exception. Temporal adjacency does not establish
that it caused the WebKit message.

Upstream [WebKitGTK 2.52.6 resource loading source](https://github.com/WebKit/WebKit/blob/webkitgtk-2.52.6/Source/WebKit/WebProcess/Network/WebLoaderStrategy.cpp)
uses this callback to fail pending resource loaders. NetworkProcess IPC scheduling
failure and network-process termination can reach it. The callback name is not a
root-cause diagnosis. A teardown ordering issue was an initial hypothesis, but neither shutdown timing
nor a specific cause has been established. Running-time failures remain in scope.

The desktop starts its local Axum gateway in the Tauri runtime and uses Tauri's
normal window lifecycle. On native Wayland the embedded-browser workspace owns WRY
views. No production exit behavior, rendering flags or error logging was changed
without a reproducible cause. In particular, this investigation does not mark the
reported error fixed and does not claim that the log proves data loss or its absence.

## Diagnostic and result

A Linux debug-only `--dev-close-window` command now closes the application's own
GTK toplevel through `gtk_window_close`, exercising the same delete-event path as
the titlebar. It uses the existing private developer-control socket, is deferred
out of the control handler, and is explicitly rejected in release builds.

`scripts/probes/native-window-close.py` runs an idle shell and two closes with a
local browser page still waiting on a slow resource. It requires a dedicated
`.studio-close-fixture` marker and an isolated environment with fresh XDG profile,
cache/data, a short /tmp runtime path, and real Codex config hidden. The Codex
executable is /usr/bin/false; this does not reproduce the user's live conversation
workload. Bubblewrap also differs from the user's graphics/accessibility environment.

Baseline evidence: `/media/rui/codex-thread-studio-agent-teams/validation/native-close-before-03/report.json`.
All three closes returned exit status 0 in 38–54 ms, with no WebKit internal-error
message or TLS panic. Local installed WebKitGTK: 2.52.6. The first two fixture
attempts failed during setup (read-only /tmp, then an overlong Unix socket path);
they were not application-close failures. Root filesystem had about 62 GiB free.

This is a non-reproduction result, not proof that the intermittent error cannot
occur. Further diagnosis needs a matching close-time NetworkProcess/GPU/process
failure or a reproducible workload. Do not suppress this error or disable GPU
acceleration globally merely to make the terminal quiet.

The checked-in probe was rerun against the final debug binary in
`/media/rui/codex-thread-studio-agent-teams/validation/native-close-final/`.
Three more closes returned 0 in 52–64 ms with no matching error. The debug command
parser tests (3), UI suite (842), typecheck and locked offline build passed.
The user confirmed that the reported process was the Agent Teams feature debug
build, not main/release, then clarified that the error may predate closing. The
specific triggering workload remains unresolved.

## Runtime diagnostics

Linux Studio shell, browser tabs and browser toolbar now report resource failures
and WebProcess termination with epoch-millisecond timestamps, a view role, and a
`close_requested` flag. Main-window close-requested/destroyed events are timestamped
separately. The flag means a request was observed, not proof that destruction
completed. This does not intercept/cancel exit, interrupt sessions, or suppress
WebKit's own messages. Only failures and lifecycle boundaries log, not every load.

Resource logs include the error domain/network-error enum and HTTP origin only.
Userinfo, paths, query/fragment tokens and raw error messages are omitted. A unit
test covers URL secret redaction. The native fixture additionally creates a
failed resource while the window remains open, checks `close_requested=false`
and verifies the query-token fixture does not appear in the diagnostic.

Runtime diagnostic validation: `/media/rui/codex-thread-studio-agent-teams/validation/webview-diagnostics-native/report.json`.
Idle, pending-resource and deliberately failed-resource native cases all closed
normally (40–54 ms). The failed resource logged `close_requested=false` before
the close-requested and destroyed markers, and omitted the fixture query token.
No original WebKit internal error was reproduced. Diagnostic redaction unit test
passed; native engine/callback validation complements the synthetic browser UI suite.
