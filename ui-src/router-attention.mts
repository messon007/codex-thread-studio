export interface AttentionDispatch {
  status?: string
  requestedAt?: number
  unread?: boolean
  targetTurnId?: string
  decision?: { targetSessionKey?: string; forwardedPrompt?: string }
}

/** Only the controller session's latest routed turn can require attention. */
export function routerAttentionEntries(
  dispatches: Map<string, AttentionDispatch>, controllers: Map<string, string>, controller: string, latestTurnKey: string,
) {
  const entry = dispatches.get(latestTurnKey)
  return entry?.decision?.targetSessionKey
    && controllers.get(latestTurnKey) === controller
    && entry.unread
    && ['completed', 'failed'].includes(entry.status || '')
    ? [{ key: latestTurnKey, entry }]
    : []
}

export function responseIsVisible(rect: { top: number; bottom: number; height: number }, viewport: { top: number; bottom: number }) {
  const overlap = Math.min(rect.bottom, viewport.bottom) - Math.max(rect.top, viewport.top)
  return rect.height > 0 && overlap >= Math.min(48, rect.height)
}
