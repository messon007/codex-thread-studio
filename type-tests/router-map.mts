import { parseRouterDecision, routerCandidates } from '../ui-src/thread-router.mjs'
import { normalizeSessionMap, parseSessionMapUpdate, SessionMapWorkerPool, safeAssistantOperations } from '../ui-src/session-map.mjs'
import type { AssistantMapOperation } from '../ui-src/session-map-types.mjs'

const decision = parseRouterDecision('{}', ['codex:one'])
const action: 'dispatch' | 'clarify' = decision.action
void action
// @ts-expect-error Catalog candidates must contain session metadata, not strings.
routerCandidates({}, { codex: ['one'] })
const map = normalizeSessionMap({})
if (map) {
  const revision: number = map.revision
  void revision
  // @ts-expect-error Normalized Map states are a closed union.
  map.items.push({ id: 'x', state: 'finished' })
}
const update = parseSessionMapUpdate('')
if (update.found) {
  const revision: number = update.update.baseRevision
  void revision
} else {
  const missing: null = update.update
  void missing
}
const operations: AssistantMapOperation[] = safeAssistantOperations({})
// @ts-expect-error AI operations cannot mark an item done.
operations.push({ op: 'setState', itemId: 'x', state: 'done' })
// @ts-expect-error AI operations cannot delete an item.
operations.push({ op: 'deleteItem', itemId: 'x' })
const pool = new SessionMapWorkerPool()
const result: Promise<number> = pool.enqueue('codex:one', () => 1)
void result
// @ts-expect-error A job's result type must be preserved through the queue.
const wrong: Promise<string> = pool.enqueue('codex:one', () => 1)
void wrong
