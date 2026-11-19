# Agent Teams capability probes

These are isolated G1/G2 experiments, not a Teams implementation or a passing test suite. Read `docs/agent-teams/verification-plan.md` and `verification-results.md` first. A completed process does **not** mean the product gates pass.

The fixture runs a real Codex App Server with synthetic local Responses SSE output and a read-only MCP echo tool. It never uses a live model or user credentials. Two threads are created before MCP registration. Raw logs record synthetic inputs and protocol events; do not commit the temporary WebSocket capability token or isolated Codex state.

Current harness prerequisites: Linux bubblewrap, Node 24, Python 3, Codex CLI 0.170.0. The dedicated root is intentionally restricted to `/media/rui/codex-thread-studio-agent-teams` to honor this development session's disk constraint. `ws@8.18.3` lives in `validation/probe-deps/node_modules`, separate from application dependencies. No application build is needed.

Preparation, on this host:

```bash
npm install --prefix /media/rui/codex-thread-studio-agent-teams/validation/probe-deps --cache /media/rui/codex-thread-studio-agent-teams/validation/npm-cache --ignore-scripts --no-audit --no-fund ws@8.18.3
```

Create a fresh, empty `validation/run-NN/codex-config` directory. Put the absolute run directory followed by a newline in `codex-config/.teams-probe-fixture`. The harness refuses to run unless this marker is visible inside the active Codex config root. Run only with isolated mounts; never place the marker in the user's real config directory.

Example command (replace `run-NN` consistently):

```bash
bwrap --unshare-net --unshare-pid --ro-bind / / \
  --bind /media/rui/codex-thread-studio-agent-teams/validation/run-NN /media/rui/codex-thread-studio-agent-teams/validation/run-NN \
  --bind /media/rui/codex-thread-studio-agent-teams/validation/run-NN/codex-config /home/rui/.codex \
  --tmpfs /tmp --proc /proc --dev /dev --die-with-parent \
  -- node /media/rui/codex-thread-studio-agent-teams/worktree/scripts/probes/agent-teams/agent-teams-gates.mjs \
  /media/rui/codex-thread-studio-agent-teams/validation/run-NN
```

This mount assumes the existing config root is `/home/rui/.codex`. If it differs, bind the empty fixture to that existing root; do not reassign HOME or CODEX_HOME. Do not work around missing isolation by pointing at a production daemon. Nested shell sandbox restrictions are logged; this fixture does not execute shell tools and cannot validate that security boundary.

Inspect `report.json` even when exit status is zero. `INCOMPLETE` means infrastructure or protocol prerequisites failed. The final identity comparison ignores only MCP progressToken, which is a progress correlation field; the fixture does not establish a trusted mapping from it to native item events. Direct calls use an authenticated test host client; this does not demonstrate that an ordinary sandboxed member has that authority.

The report and JSONL evidence are intentionally retained outside Git under `/media`. The checked-in observation excerpt records the decisive test-run IDs and source hashes; it is not a complete transcript. No tests here authorize production feature work or mainline merging.

For the revised G1b/G2b samples, append `thread-config` after the run-directory argument. This creates three synthetic pre-existing threads, tries a member-only resume config, checks bystander inventories and original history, then tests idle/busy single queue admission and exact deletion. When loaded-thread overrides are ignored, it tries unsubscribe followed by the same thread ID only after the loaded list confirms unloading. If unloading cannot be established, it stops as INCOMPLETE; it never archives, forks or deletes a thread to force the test through. `run-04` predates this last detach check and completed the queue samples; `run-05` records the bounded unload timeout. Neither is a full G1b/G2b pass.

## Integrated Studio acceptance

The early `agent-teams-gates.mjs` experiments above are retained as historical evidence. The final implementation uses `studio-integration.mjs`, which starts the actual feature-branch Studio binary and Codex App Server, installs the owned MCP registration into the isolated config, exercises the real native tools and queue, restarts Studio, and opens its independent Teams UI in Chromium.

Use a fresh `validation/integration-NN/codex-config` and the same marker/mount isolation as above. Run `studio-integration.mjs RUN_DIRECTORY STUDIO_BINARY` instead of the gates script. The integration fixture also uses Playwright 1.58.2 from `validation/browser-tools/node_modules` and the cached Chromium headless shell identified in the script; browser profile/cache state stays in the fixture. Those are development-only prerequisites, not application or public-CI dependencies. This host's full Chromium build crashes in the restricted fixture; its headless-shell build has been verified there.

Coverage includes existing-session registration, nonmember rejection, busy delivery without interruption, explicit submission/review, stale queue CAS, identical multi-page sends, supervision reservations, A→B delegation/question/answer/rework/acceptance, pause, host restart, browser task/result details, in-flight result reporting while disabling, and precise uninstall. Inspect the final `report.json`: every named check must be present and `success` true. No test uses a live model, original user config or real user sessions. See the sanitized [validation manifest](../../../docs/agent-teams/implementation-validation.json) for final counts and source hashes.

The right-workspace redesign adds group/task navigation, contextual deletion,
confirmation cancellation, draft preservation, shared-tool restoration and the
language/theme/font matrix. Current evidence is in
[ui-redesign-validation.json](../../../docs/agent-teams/ui-redesign-validation.json);
the original implementation manifest remains a historical baseline.
