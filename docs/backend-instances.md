# Local backend instances

Codex Thread Studio can load additional Codex-compatible backend instances from
a machine-local JSON file. This keeps private launchers, profiles, and model
catalogs out of the repository while preserving independent sessions and model
selection in the application.

## Configuration location

The application reads `backends.json` beside its `settings.json` file. Open
**Connections** in Studio to see the exact path for the current machine.

Set `CODEX_THREAD_STUDIO_BACKENDS_CONFIG` before starting Studio to use a
different absolute path. Studio loads the active backend registry from this file
only at startup.

No file is created automatically. When the file is absent, Studio exposes only
the built-in Codex and OpenCode backends. A malformed file is reported in
Connections and does not prevent the built-in backends from loading.

The native GNOME frontend also provides **Settings → Backends → Edit backend
configuration**. This opens a syntax-highlighted JSON editor for that same file.
Validate checks the startup schema without writing; Save explicitly writes the
original file. Reload discards edits only after confirmation. Concurrent external
changes reject a stale save and retain the editor contents. An invalid JSON file
can be opened and repaired there. No file is created until Save succeeds.

Saving does not change the running registry or start a provider. Restart Studio
to apply the saved configuration; a configured process starts when that backend
is selected. The editor has a 256 KiB UTF-8 file budget and keeps unsaved edits
only in memory. See [native backend configuration](gnome-native.md#native-backend-configuration)
for validation details.

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
