export interface CachedViewModel {
  threadId: string | null
  activeTurnId: string | null
  turns: { id: string }[]
}
export interface SessionCacheEntry<M extends CachedViewModel> {
  model: M
  validatedAt: number
  historyEpoch?: number | null
}
export type SessionModelCache<M extends CachedViewModel> = Map<string, SessionCacheEntry<M>>

export function cachedSession<M extends CachedViewModel>(cache: SessionModelCache<M>, backend: string, id: string | null) {
  return id ? cache.get(`${backend}:${id}`) || null : null
}

export function storeCachedSession<M extends CachedViewModel>(
  cache: SessionModelCache<M>, backend: string, id: string | null, model: M | null,
  historyEpoch: number | null, now: number,
): boolean {
  if (!id || !model || model.threadId !== id) return false
  const key = `${backend}:${id}`
  const existing = cache.get(key)
  cache.set(key, {
    ...(existing || {}), model, validatedAt: now,
    ...(backend === 'opencode' ? { historyEpoch } : {}),
  })
  return true
}

export function validateCachedModel<M extends CachedViewModel>(
  cache: SessionModelCache<M>, backend: string, model: M, openCodeHistoryEpoch: number, now: number,
) {
  for (const [key, cached] of cache) {
    if (key.startsWith(`${backend}:`) && cached.model === model) {
      cached.validatedAt = now
      // A live delta cannot certify events missed before reconnect.
      if (backend === 'opencode' && cached.historyEpoch !== openCodeHistoryEpoch) return
      if (backend === 'opencode') cached.historyEpoch = openCodeHistoryEpoch
      return
    }
  }
}

export function unvalidateCachedModel<M extends CachedViewModel>(cache: SessionModelCache<M>, backend: string, model: M) {
  for (const [key, cached] of cache) {
    if (key.startsWith(`${backend}:`) && cached.model === model) {
      cached.validatedAt = 0
      return
    }
  }
}

export function cachedModelThreadId<M extends CachedViewModel>(cache: SessionModelCache<M>, backend: string, model: M) {
  for (const [key, cached] of cache) {
    if (key.startsWith(`${backend}:`) && cached.model === model) return key.slice(backend.length + 1)
  }
  return null
}

export interface NotificationRoutingContext<M extends CachedViewModel> {
  backend: string
  selectedBackend: string
  selectedId: string | null
  selectedModel: M
  cache: SessionModelCache<M>
  hiddenThreads: ReadonlySet<string>
  hiddenTurns: ReadonlySet<string>
  sessionKey: (backend: string, id: unknown) => string
  turnKey: (backend: string, id: unknown) => string
}

export function routeCodexNotification<M extends CachedViewModel>(message: unknown, context: NotificationRoutingContext<M>): M | null {
  const { backend, selectedBackend, selectedId, selectedModel, cache, hiddenThreads, hiddenTurns } = context
  const params = record(record(message).params)
  const turn = record(params.turn)
  const explicitId = params.threadId || record(params.thread).id || turn.threadId
  if (explicitId && hiddenThreads.has(context.sessionKey(backend, explicitId))) return null
  if (explicitId) {
    if (selectedBackend === backend && selectedId === explicitId) return selectedModel
    return cache.get(`${backend}:${explicitId}`)?.model || null
  }
  const turnId = params.turnId || turn.id
  if (turnId && hiddenTurns.has(context.turnKey(backend, turnId))) return null
  if (turnId) {
    for (const [key, cached] of cache) {
      if (!key.startsWith(`${backend}:`)) continue
      if (cached.model.activeTurnId === turnId || cached.model.turns.some((candidate) => candidate.id === turnId)) return cached.model
    }
  }
  return selectedBackend === backend ? selectedModel : null
}

function record(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {}
}
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object'
}
