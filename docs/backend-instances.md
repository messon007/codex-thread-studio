# Local backend instances

Codex Thread Studio can load additional Codex-compatible backend instances from
a machine-local JSON file. This keeps private launchers, profiles, and model
catalogs out of the repository while preserving independent sessions and model
selection in the application.

## Configuration location

The application reads `backends.json` beside its `settings.json` file. Open
**Connections** in Studio to see the exact path for the current machine.

Set `CODEX_THREAD_STUDIO_BACKENDS_CONFIG` before starting Studio to use a
different absolute path. Studio reads this file only at startup.

No file is created automatically. When the file is absent, Studio exposes only
the built-in Codex and OpenCode backends. A malformed file is reported in
Connections and does not prevent the built-in backends from loading.

## File format

```json
{
  "version": 1,
  "backends": [
    {
      "id": "ept-codex",
      "name": "EPT Codex",
      "tag": "EP",
      "adapter": "codex-app-server",
      "command": "ept",
      "args": ["codex"]
    }
  ]
}
```

Studio appends `app-server --stdio` after `args`, so the example starts:

```text
ept codex app-server --stdio
```

Launcher options that must precede the `codex` subcommand remain separate
arguments:

```json
{
  "version": 1,
  "backends": [
    {
      "id": "ept-codex",
      "name": "EPT Codex",
      "tag": "EP",
      "adapter": "codex-app-server",
      "command": "ept",
      "args": [
        "--ept-profile", "work",
        "codex",
        "--project-code", "project-name"
      ]
    }
  ]
}
```

The command is executed directly rather than through a shell. Do not add shell
quoting or combine the command and arguments into one string.

## Fields

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | Yes | Stable lowercase namespace used for sessions and preferences. |
| `name` | Yes | User-facing backend name. |
| `tag` | Yes | One to four uppercase letters or digits shown in the session list. |
| `adapter` | Yes | Must currently be `codex-app-server`. |
| `command` | Yes | Executable name or absolute path. |
| `args` | No | Arguments inserted before `app-server --stdio`. |
| `enabled` | No | Defaults to `true`; set to `false` to ignore the entry. |

`codex` and `opencode` are reserved IDs. A configuration can contain at most 16
additional backends.

## Isolation guarantees

Each configured entry owns a separate App Server process, WebSocket connection,
session catalog, model catalog, reconnect generation, and selected model state.
Session references use `backend-id:thread-id`, so equal native thread IDs cannot
collide across instances. Router dispatch, favorites, comments, Session Maps,
and project-environment application retain that namespace.

Models are always obtained from the selected instance through its native
`model/list` method. Studio does not copy the built-in Codex model list into a
configured instance.

## Credentials and local-only data

The backend process inherits Studio's environment. Prefer the launcher's own
credential store or environment variables for secrets. Do not place API keys in
`args`, because the command is visible in Studio diagnostics and may be visible
to operating-system process inspection.

Keep `backends.json` outside the repository. It is a machine preference, not a
project file.

See [Add a DeepSeek Codex backend](deepseek-backend.md) for a worked example that runs an
independent provider alongside the built-in backend.

## Share a session with the Codex CLI

The built-in Codex backend now uses the same local app-server daemon as recent
Codex CLI clients. With Codex CLI 0.166.0, Studio starts the daemon idempotently
and attaches using `codex app-server proxy`. The official proxy carries
WebSocket frames over stdio; Studio performs the WebSocket handshake and keeps
its JSON-RPC connection independent from each CLI connection.

Studio and the CLI can stay open on the same session and submit messages. Both
receive the session's events. Codex coordinates the shared turn; CLI queue/steer
behavior still applies while a turn is running. Studio's own queued messages
retain their existing queue controls.

For another computer, SSH to the machine running that session and use the same
OS user and `CODEX_HOME`:

```sh
ssh -t user@session-host 'codex resume <session-id>'
```

Use the CLI's default daemon mode for sharing. A login to the same OpenAI account
on two separate hosts does not connect those hosts to the same local daemon.
The SSH session runs the CLI beside Studio's backend and needs no access to the
already-open Studio window. After updating Studio, restart it once so its old
private app-server process can exit.

Studio disconnecting or exiting only closes its proxy; it does not stop the
shared daemon or another CLI's work. The connector sends WebSocket heartbeats
and detects closed/stalled transports, so normal reconnect handling can resume
after the daemon restarts. Studio does not restart or stop an existing daemon.

### Choose a connection mode

For the built-in backend, set `CODEX_THREAD_STUDIO_CODEX_CONNECTION` before
starting Studio:

| Mode | Behavior |
| --- | --- |
| `auto` (default) | Use the shared daemon when the CLI supports daemon/proxy commands; use the legacy embedded server for older CLIs. |
| `shared` | Require daemon/proxy support. Report startup or connection failures instead of opening a private writer. |
| `embedded` | Start Studio's independent `app-server --stdio` process. |

A supported daemon failing to start is always reported. Only an older CLI's
unsupported-command response allows `auto` to select the embedded server.

Configured providers default to `embedded` to preserve profile isolation. To
share a configured provider, add `"connectionMode": "shared"` (or `"auto"`) to
its `backends.json` entry. Studio preserves the command/prefix arguments when
probing capabilities, starting the daemon and attaching the proxy. Share the
same provider, OS user and `CODEX_HOME` with its CLI clients.

### Legacy single-writer handoff

For embedded mode, when the session is idle choose **More session actions →
Release session write access**. Studio pauses its local queue and unsubscribes.
The embedded app server uses `thread_unload_delay_secs=0`, so an idle thread
without subscribers unloads and releases its writer lock. Another independent
CLI can then resume it. This compatibility action requires the Studio UI;
shared mode above supports the remote, unattended scenario.

The saved conversation, draft and queued messages remain intact. Use **Reload
session** to resume in Studio after the independent writer releases it. Running
turns, queue submissions and supervised sessions cannot be released by this
compatibility action.

### Verification

`python3 scripts/smoke-codex-shared.py` compiles the actual Studio connector
sources in a headless harness and uses a private temporary `CODEX_HOME`, a real
Codex CLI daemon and a local mock Responses provider. It verifies two attached
clients writing the same thread, a Studio turn, a real `codex queue --resume`
message, completion events on both connections and reconnection after a daemon
restart. It also runs the connector/runtime/configuration unit tests. No real
model service or account data is used. Verified locally with Codex CLI 0.166.0
on Linux; the headless harness requires Cargo dependencies to be cached.

`python3 scripts/smoke-codex-handoff.py` separately verifies the legacy embedded
handoff with two independent app servers and no inference requests.
