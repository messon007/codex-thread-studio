import type { QueuedMessage } from './session-types.mjs'

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

export const MAX_MESSAGE_QUEUE_DEPTH = 3

export function normalizeQueueDepth(value: unknown) {
  const depth = Math.trunc(Number(value))
  return depth >= 1 && depth <= MAX_MESSAGE_QUEUE_DEPTH ? depth : 1
}

export function normalizeStoredMessageQueues(value: unknown, now = () => Date.now()) {
  const normalized: Record<string, QueuedMessage[]> = {}
  if (!value || typeof value !== 'object' || Array.isArray(value)) return normalized
  for (const [key, entries] of Object.entries(value).slice(0, 2048)) {
    if (typeof key !== 'string' || !key.includes(':') || key.length > 320 || !Array.isArray(entries)) continue
    const messages = entries.slice(0, MAX_MESSAGE_QUEUE_DEPTH).flatMap((entry) => {
      if (!isRecord(entry)) return []
      const id = String(entry.id || '').trim().slice(0, 128)
      const text = String(entry.text || '').slice(0, 64 * 1024)
      const input = Array.isArray(entry.input)
        ? entry.input.slice(0, 32).filter((item): item is Record<string, unknown> => isRecord(item) && item.type !== 'text')
        : []
      if (!id || (!text && !input.length)) return []
      return [{ id, text, input, createdAt: Number(entry.createdAt) || now() }]
    })
    if (messages.length) normalized[key] = messages
  }
  return normalized
}

export function completedQueueShouldAdvance(status: unknown, paused: boolean) {
  return !paused && String(status || '').toLowerCase() === 'completed'
}
