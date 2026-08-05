# Native Thread Router specification

## Purpose and boundary

Thread Router lets a user keep one fixed Codex session open as an inbox while work executes in other existing Codex Threads. It is a Codex Thread Studio feature built directly on Codex App Server. It does not read Agent Deck configuration, invoke Agent Deck commands, attach to tmux, or require a bridge daemon.

Version 1 routes one text request to exactly one existing Codex Thread. It does not split one request across several targets, queue work behind a busy target, route to OpenCode, or keep work alive after the Studio/App Server process exits.

## Configuration

The user opens **More → Router settings** and configures:

- an optional responsibility description for every target Thread;
- whether each Thread is an ordinary target or a fallback target.

When the Codex catalog first becomes available, Studio creates one read-only, non-interactive system Router Thread in its own `~/.local/share/codex-thread-studio/router` workspace and continuously reuses it. The user never chooses a Router implementation Thread. Only this Thread exposes **More → Router settings**. Thread title, working directory, and any locally captured opening message are included in the routing catalog automatically. The Router itself is always excluded from targets. Deleting or archiving a configured target removes it from routing; deleting or archiving the system Router causes Studio to create a replacement when the catalog is next loaded.

The configuration is validated by the Rust gateway and stored in `~/.config/codex-thread-studio/settings.json`. It contains no model credentials and no copied response history.

## Dispatch sequence

```text
User → Router Thread composer
  → thread/resume (refresh routing instructions and target catalog)
  → turn/start (structured output schema)
  → { dispatch | clarify }
      ├─ clarify → render a question in the Router transcript
      └─ dispatch
          → lazily thread/read target into browser cache
          → reject if target already has an active Turn
          → turn/start on target without changing selection
          → stream target notifications into target cache
          → update Router dispatch card and Attention ordering
```

The designated Router Thread is durable and reused for every routing request. No temporary Thread is created. The controller is instructed not to call tools or solve the request; it returns only a schema-constrained decision containing `action`, `targetThreadId`, `forwardedPrompt`, `reason`, and `message`.

The target is validated against the exact catalog supplied for that routing Turn. A model cannot invent an ID or route back to the Router. Explicit target references win, followed by specific responsibility matches. If none matches, the model compares the request against all fallback responsibilities and selects the best fit; there may be any number of fallback targets. Ambiguous requests produce a clarification instead of a guessed dispatch.

## Presentation

Native JSON remains in the provider-owned Router history, but Studio recognizes valid routing decisions and renders them as compact dispatch cards. While routing, the card shows target selection; after selection it exposes the target title, reason, execution state, and an **Open session** action. On restart, completed routing decisions can be reconstructed from native history. Runtime-only target completion state is not persisted separately.

The target Thread appears active in the ordinary sidebar and receives its full structured output—messages, commands, file changes, approvals, and stop reason—through the existing App Server event path. Router output is never substituted for target output.

## Failure behavior

- No available targets: do not start a Router Turn; direct the user to Router settings.
- Invalid or out-of-catalog decision: show a failed routing card; do not dispatch.
- Busy target: show a failed routing card; do not steer or interrupt the target.
- Target removed between decision and dispatch: show a failed routing card.
- App Server disconnect: existing reconnect/reconciliation behavior applies; no client-side automatic replay occurs, preventing duplicate work.

## Acceptance criteria

1. Loading the Codex catalog creates exactly one system-managed Router Thread when absent; subsequent loads, saves, and routing requests reuse it.
2. Restarting Studio preserves the Router ID, target responsibilities, and target roles.
3. Sending text from the Router starts one schema-constrained Router Turn and creates no temporary Thread.
4. A valid decision starts exactly one Turn in the selected existing target without selecting that Thread.
5. Target streaming updates and completion are visible after opening the target, and the target participates in the normal Attention ordering.
6. Invalid IDs, ambiguity, missing targets, and busy targets never start an unintended Turn.
7. Router history displays readable status cards instead of raw decision JSON after completion.
8. OpenCode sessions are never offered as Router or target in version 1.
9. The implementation contains no Agent Deck, tmux, or conductor dependency.
