export interface AttentionDispatch {
  status?: string
  requestedAt?: number
  unread?: boolean
  targetTurnId?: string
  decision?: { targetSessionKey?: string; forwardedPrompt?: string }
}

/** A new request supersedes the previous reminder, even while still running. */
export function routerAttentionEntries(
  dispatches: Map<string, AttentionDispatch>, controllers: Map<string, string>, controller: string,
) {
  const latest = new Map<string, { key: string; entry: AttentionDispatch }>()
  for (const [key, entry] of dispatches) {
    const target = entry.decision?.targetSessionKey
    if (!target || controllers.get(key) !== controller) continue
    const previous = latest.get(target)
    if (!previous || (entry.requestedAt || 0) >= (previous.entry.requestedAt || 0)) latest.set(target, { key, entry })
  }
  return [...latest.values()].filter(({ entry }) => entry.unread && ['completed', 'failed'].includes(entry.status || ''))
    .sort((a, b) => (a.entry.requestedAt || 0) - (b.entry.requestedAt || 0))
}

export function responseIsVisible(rect: { top: number; bottom: number; height: number }, viewport: { top: number; bottom: number }) {
  const overlap = Math.min(rect.bottom, viewport.bottom) - Math.max(rect.top, viewport.top)
  return rect.height > 0 && overlap >= Math.min(48, rect.height)
}
