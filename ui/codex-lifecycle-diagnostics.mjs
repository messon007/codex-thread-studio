const CODEX_LIFECYCLE_METHODS = new Set([
  'thread/status/changed',
  'turn/started',
  'turn/completed',
])

export function codexLifecycleEvent(message) {
  const method = String(message?.method || '')
  if (!CODEX_LIFECYCLE_METHODS.has(method)) return null
  const params = message?.params || {}
  const status = method === 'thread/status/changed'
    ? params.status?.type || params.status
    : params.turn?.status || params.status
  return {
    method,
    threadId: String(params.threadId || params.thread?.id || params.turn?.threadId || ''),
    turnId: String(params.turnId || params.turn?.id || ''),
    notificationStatus: typeof status === 'string' ? status : '',
  }
}

export function codexLifecycleStreamMessage(payload) {
  const method = String(payload?.method || '')
  if (method === 'studio/codexLifecycle/ready') {
    return {
      type: 'ready',
      backends: Array.isArray(payload.params?.backends)
        ? payload.params.backends
          .filter((backend) => typeof backend === 'string' && backend)
          .map(String)
        : [],
    }
  }
  if (method !== 'studio/codexLifecycle/event') return null
  const backend = String(payload.params?.backend || '')
  const message = payload.params?.message
  const eventMethod = String(message?.method || '')
  if (!backend || !message || (
    !CODEX_LIFECYCLE_METHODS.has(eventMethod)
    && eventMethod !== 'studio/appServer/status'
    && eventMethod !== 'studio/appServer/lagged'
  )) return null
  return { type: 'event', backend, message }
}
