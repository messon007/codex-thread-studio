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
