# Add a DeepSeek Codex backend

Studio loads extra Codex-compatible backends from `backends.json` beside its `settings.json`
([local backend instances](backend-instances.md)). This guide adds a second Codex instance whose
provider, credentials, model catalog, and session history belong to DeepSeek, while the built-in
**Codex** entry keeps whatever its own Codex home already is. Either side can be the OpenAI one —
Studio does not care which; whichever home an instance points at defines that instance.

**Preferred setup:** use shape B below—a wrapper with a dedicated `CODEX_HOME` and an API key loaded
through `env_key`. This keeps DeepSeek configuration, credentials, sessions, history, and the model
catalog separate from the built-in Codex backend, while keeping the key out of `backends.json` and
the process arguments. A shared Codex home is supported by shape A, but is intended only when that
shared state is acceptable.

Studio passes only `command` and `args` to the process, and the schema rejects unknown fields, so
there is no `env` field to set: put `CODEX_HOME` in the arguments or wrap the CLI in a short script.

## 1. Create an isolated Codex home

Skip this step if you pick shape A in step 2: it reuses the ordinary `~/.codex` and only needs the
DeepSeek provider block inside that existing config.

For the preferred isolated setup, create a fresh home instead of copying the ordinary OpenAI Codex
configuration:

```bash
mkdir -p ~/.codex-deepseek
chmod 700 ~/.codex-deepseek
```

Install the current `models.json` from the manual setup section of the
[official DeepSeek Codex guide](https://api-docs.deepseek.com/quick_start/agent_integrations/codex/)
as `~/.codex-deepseek/models.json`, then set its permissions to `600`. The official setup script can
also generate this catalog in a temporary directory before it is installed into the isolated home.
Do not substitute Codex's `models_cache.json`; it is a different file managed by Codex itself.

If another DeepSeek Codex home already works on the machine, copying its `config.toml` and
`models.json` is also valid. Review absolute paths and replace any credential stored directly in the
copied TOML with the `env_key` form below.

Create `~/.codex-deepseek/config.toml` with the following recommended configuration. The provider,
authentication, model, and catalog fields establish the DeepSeek connection; the reasoning and web
search fields define compatible runtime behaviour.

```toml
model = "deepseek-flash"
model_provider = "deepseek"
preferred_auth_method = "apikey"
forced_login_method = "api"
model_reasoning_effort = "high"
web_search = "disabled"
model_catalog_json = "~/.codex-deepseek/models.json"

[model_providers.deepseek]
name = "deepseek"
base_url = "https://api.deepseek.com/"
wire_api = "responses"
env_key = "DEEPSEEK_API_KEY"
```

- `model` must exist in the catalog named by `model_catalog_json`. A private alias such as
  `deepseek-flash` is not a public DeepSeek model name, so the picker stays empty without that file —
  copy the working catalog into the new home or regenerate it for this instance.
- `model_provider` selects the provider block, while `preferred_auth_method` and
  `forced_login_method` keep the instance on API-key auth instead of a ChatGPT login.
- Current Codex releases support only `wire_api = "responses"`. A provider that exposes only an
  OpenAI-compatible chat-completions endpoint needs a Responses-compatible adapter or proxy before
  Codex App Server can use it.
- Credentials live either in `experimental_bearer_token` (then `chmod 600` the file) or in
  `env_key = "DEEPSEEK_API_KEY"`, which the preferred launcher in step 2 supplies. Codex 0.155.1
  supports both; `env_key` is preferred because it keeps the key out of `config.toml`.
- `model_reasoning_effort` and `web_search` carry the provider-compatible runtime behaviour.
  `approvals_reviewer` is optional and can be added separately when that policy is wanted.

Because the home is separate, this instance has its own `auth.json`, `sessions/`, history, and
trusted-project list. Nothing here changes the built-in backend.

## 2. Choose how the instance starts

Studio runs `<command> <args> app-server --stdio`, so `args` must stop before the subcommand: never
add `app-server` yourself. Three shapes work, and they differ in how much the two backends share.
Shape B is preferred for a normal Studio installation.

**A. Override the model on a shared Codex home.** Keep the DeepSeek provider block in the ordinary
`~/.codex/config.toml`, then let the instance select it:

```text
command: codex
args:    ["-c", "model_provider=deepseek", "-c", "model=deepseek-flash"]
```

`codex` is the command and the DeepSeek selection is the argument, which reads the way you would
expect. The app server accepts `-c key=value` overrides, and the credential stays in the config file.
The cost is shared state: both backends read the same `auth.json`, `sessions/`, and model catalog, so
their thread lists are identical and only the model and provider differ.

A profile can replace the overrides when its configuration lives in
`$CODEX_HOME/deepseek.config.toml`:

```text
command: codex
args:    ["--profile", "deepseek"]
```

Studio places these arguments before `app-server`, producing
`codex --profile deepseek app-server --stdio`. Putting `--profile` after `app-server` is what Codex
rejects. Like the `-c` form, a profile shares that Codex home's authentication and session state, so
use shape B when the DeepSeek backend should be independent.

**B. Give the instance its own Codex home through a wrapper (preferred).** This is the shape that
isolates `auth.json`, `sessions/`, history, and the model catalog from the built-in backend:

```bash
cat > ~/.codex-deepseek/launch-codex <<'EOF'
#!/bin/sh
export CODEX_HOME="$HOME/.codex-deepseek"

if [ -r "$CODEX_HOME/api-key" ]; then
  DEEPSEEK_API_KEY=$(tr -d '\r\n' < "$CODEX_HOME/api-key")
  export DEEPSEEK_API_KEY
fi

exec codex "$@"
EOF
chmod +x ~/.codex-deepseek/launch-codex
```

```text
command: /Users/you/.codex-deepseek/launch-codex
```

The wrapper exports the optional `env_key` credential, so the key never appears in `args` or shell
history. Create the protected file with an editor:

```bash
install -m 600 /dev/null ~/.codex-deepseek/api-key
${EDITOR:-vi} ~/.codex-deepseek/api-key
```

Studio does not read `api-key` itself. It starts `launch-codex`; the wrapper reads the file and
exports `DEEPSEEK_API_KEY`; then Codex resolves the provider's `env_key = "DEEPSEEK_API_KEY"` from
that process environment. The file can therefore remain absent until the API key is available.

**C. Its own Codex home without a script.** `backends.json` has no `env` field, so the variable has
to travel as an argument. `/usr/bin/env` is the trampoline: it sets `CODEX_HOME` and then executes
the real command, `codex`.

```text
command: /usr/bin/env
args:    ["CODEX_HOME=/home/you/.codex-deepseek", "codex"]
```

Never put an API key in `args`: Studio shows the command line in its diagnostics, and the operating
system exposes it in the process list.

On Windows, use a `.cmd` wrapper with `set CODEX_HOME=...` instead of the shell script.

## 3. Register the backend

The file lives beside `settings.json`:

- Linux and macOS: `$XDG_CONFIG_HOME/codex-thread-studio/backends.json`, or
  `~/.config/codex-thread-studio/backends.json` when `XDG_CONFIG_HOME` is unset.
- Windows: `%XDG_CONFIG_HOME%\codex-thread-studio\backends.json`; define `XDG_CONFIG_HOME` or `HOME`
  before launching Studio, otherwise preferences fall back to the temporary directory.

Studio prints the exact path in **Connections**, and `CODEX_THREAD_STUDIO_BACKENDS_CONFIG` overrides
it with an absolute path of your choosing.

The entry below uses shape B; replace `command` and `args` with the shape you chose in step 2. A
`command` that points at a script must be an absolute path.

```json
{
  "version": 1,
  "backends": [
    {
      "id": "ds-codex",
      "name": "DeepSeek Codex",
      "tag": "DS",
      "adapter": "codex-app-server",
      "command": "/Users/you/.codex-deepseek/launch-codex"
    }
  ]
}
```

## 4. Restart and verify

`backends.json` is read only at startup, so restart Studio, then:

```bash
python3 -m json.tool ~/.config/codex-thread-studio/backends.json > /dev/null && echo "backends.json valid"
~/.codex-deepseek/launch-codex --version
~/.codex-deepseek/launch-codex app-server --stdio < /dev/null   # starts, exits at EOF
```

In Studio, **Connections** lists the new backend and reports any validation error. Selecting it gives
an independent session list, and its model list comes from that instance's own `model/list`, so pick
the DeepSeek model in the composer. Threads are namespaced `ds-codex:<thread-id>`, so both backends
can hold identical native thread IDs without colliding.

## Limits and failure modes

- `id` is lowercase `[a-z0-9_-]` starting with a letter, at most 64 characters; `codex` and `opencode`
  are reserved, and a configuration can hold at most 16 extra backends.
- `tag` is one to four uppercase letters or digits; `adapter` must be `codex-app-server`.
- Unknown JSON fields are rejected, so a typo such as `"env"` fails validation instead of being
  ignored.
- A malformed file is reported in **Connections** and leaves the built-in backends working, so it is
  safe to iterate.
- Studio never copies the built-in model list into a configured instance; a provider that returns no
  models shows an empty picker until its own catalog is configured.
- Keep `backends.json`, `config.toml`, and any key material outside the repository.
