import type { LifecycleEvent, LifecycleStreamMessage } from './notification-types.mjs'

const CODEX_LIFECYCLE_METHODS = new Set([
  'thread/status/changed',
  'turn/started',
  'turn/completed',
])

function record(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {}
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object'
}

export function codexLifecycleEvent(message: unknown): LifecycleEvent | null {
  const envelope = record(message)
  const method = String(envelope.method || '')
  if (!CODEX_LIFECYCLE_METHODS.has(method)) return null
  const params = record(envelope.params)
  const turn = record(params.turn)
  const status = method === 'thread/status/changed'
    ? record(params.status).type || params.status
    : turn.status || params.status
  return {
    method,
    threadId: String(params.threadId || record(params.thread).id || turn.threadId || ''),
    turnId: String(params.turnId || turn.id || ''),
    notificationStatus: typeof status === 'string' ? status : '',
  }
}

export function codexLifecycleStreamMessage(payload: unknown): LifecycleStreamMessage | null {
  const envelope = record(payload)
  const method = String(envelope.method || '')
  const params = record(envelope.params)
  if (method === 'studio/codexLifecycle/ready') {
    return {
      type: 'ready',
      backends: Array.isArray(params.backends)
        ? params.backends.filter((backend) => typeof backend === 'string' && backend).map(String)
        : [],
    }
  }
  if (method !== 'studio/codexLifecycle/event') return null
  const backend = String(params.backend || '')
  const message = record(params.message)
  const eventMethod = String(message.method || '')
  if (!backend || !params.message || (
    !CODEX_LIFECYCLE_METHODS.has(eventMethod)
    && eventMethod !== 'studio/appServer/status'
    && eventMethod !== 'studio/appServer/lagged'
  )) return null
  return { type: 'event', backend, message }
}

export function rememberBoundedLifecycleEvent<T>(collection: Map<string, T>, key: string, value: T, limit = 1_024) {
  collection.delete(key)
  collection.set(key, value)
  while (collection.size > limit) {
    const first = collection.keys().next()
    if (first.done) break
    collection.delete(first.value)
  }
}

/** Caller owns these maps so reconnect reset points retain their existing semantics. */
export function claimLifecycleNotification(
  backend: string, message: unknown,
  turns: Map<string, boolean>, statuses: Map<string, number>, now: () => number,
): LifecycleEvent | null {
  const event = codexLifecycleEvent(message)
  if (!event) return null
  if ((event.method === 'turn/started' || event.method === 'turn/completed') && event.turnId) {
    const key = `${backend}:${event.method}:${event.turnId}`
    if (turns.has(key)) return null
    rememberBoundedLifecycleEvent(turns, key, true)
    return event
  }
  if (event.method === 'thread/status/changed') {
    const status = record(record(message).params).status
    const key = `${backend}:${event.threadId}:${JSON.stringify(status ?? '')}`
    const timestamp = now()
    const previous = statuses.get(key)
    if (previous != null && timestamp - previous < 250) return null
    rememberBoundedLifecycleEvent(statuses, key, timestamp, 256)
  }
  return event
}
