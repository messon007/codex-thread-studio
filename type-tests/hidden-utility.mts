import type { HiddenUtilityState, UtilityRpcResult } from '../ui-src/hidden-utility-session.mjs'
import type { UtilityThread } from '../ui-src/utility-task.mjs'
import { createCodexViewModel } from '../ui-src/codex-native.mjs'

// Nullable errors in real Codex models must be accepted by utility parsers.
const thread: UtilityThread = createCodexViewModel()
void thread

function check(state: HiddenUtilityState) {
  // @ts-expect-error Structured task registration requires a real view model.
  state.structuredUtilityTasks.set('codex:task', { backend: 'codex', threadId: 'task', turnId: '' })
  // @ts-expect-error Thread IDs in decoded utility responses are strings.
  const result: UtilityRpcResult = { thread: { id: 123 } }
  void result
}
void check
