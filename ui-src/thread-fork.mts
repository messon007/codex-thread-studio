export function threadForkParams(threadId: unknown, lastTurnId: unknown = null) {
  const params: { threadId: string; lastTurnId?: string } = { threadId: String(threadId || '') }
  if (lastTurnId) params.lastTurnId = String(lastTurnId)
  return params
}

export function openCodeForkBody(params: { lastTurnId?: unknown } = {}) {
  return params.lastTurnId ? { messageID: String(params.lastTurnId) } : {}
}

export function isTurnForkable(turn: { id?: unknown; studioOptimistic?: boolean; status?: string } | null | undefined) {
  return Boolean(
    turn?.id
    && !turn.studioOptimistic
    && turn.status
    && turn.status !== 'inProgress'
    && turn.status !== 'unknown',
  )
}

