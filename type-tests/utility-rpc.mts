import type { UtilityResult } from '../ui-src/utility-task.mjs'
import { PendingRpcRequests } from '../ui-src/rpc-lifecycle.mjs'
import type { QueueExecutionState } from '../ui-src/queue-execution.mjs'

function consume(result: UtilityResult<{ text: string }>): string {
  if (result.status === 'completed') return result.text
  // @ts-expect-error Running/failed results have no completed payload.
  const text: string = result.text
  return text
}
void consume

// @ts-expect-error A completed utility result requires its payload.
const incomplete: UtilityResult<{ text: string }> = { status: 'completed' }
void incomplete

async function readRpc(requests: PendingRpcRequests) {
  const result = await requests.request(1, 'thread/read', () => {}, 1000, 'Timeout')
  // @ts-expect-error Untrusted RPC output must be narrowed before use.
  const thread: { id: string } = result
  return thread
}
void readRpc

function invalidQueue(state: QueueExecutionState) {
  // @ts-expect-error Queue execution ownership is a set, not an array.
  state.runningMessageQueues = []
  // @ts-expect-error Error messages cannot contain arbitrary structured values.
  state.messageQueueErrors.set('codex:thread', { message: 'failed' })
}
void invalidQueue
