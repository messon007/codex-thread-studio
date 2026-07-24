import { isActiveCatalogThread, threadCatalogKey } from './thread-catalog.mjs'

export function setThreadAttention(attentionThreads, backend, id, attention) {
  if (!id) return false
  const key = threadCatalogKey(backend, id)
  const hadAttention = attentionThreads.has(key)
  if (attention) attentionThreads.add(key)
  else attentionThreads.delete(key)
  return hadAttention !== attention
}

export function catalogAttentionState(previous, current) {
  if (!previous || !current) return null
  if (isActiveCatalogThread(previous) && !isActiveCatalogThread(current)) return true
  if (isActiveCatalogThread(current)) return false
  return null
}

export function codexAttentionState(method = '') {
  if (method === 'turn/started') return false
  if (method === 'turn/completed' || method.includes('/requestApproval')) return true
  return null
}

export function openCodeAttentionState(payload = {}) {
  const type = payload.type || ''
  const properties = payload.properties || {}
  const status = properties.status?.type || properties.status
  if (type === 'message.updated' && properties.info?.role === 'user') return false
  if (type === 'session.status' && ['busy', 'active', 'retry'].includes(status)) return false
  if (type === 'session.idle' || type === 'session.error' || type === 'permission.asked') return true
  return null
}
