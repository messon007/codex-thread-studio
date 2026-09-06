export interface EnvironmentApplyOptions {
  backend: string
  threadId: string | null
  generation: string | number
  root: string
  includeThread: boolean
  profileRevision: string
  force: boolean
  excludeTurns: boolean
  initialTurnsPage: unknown
}

export function createEnvironmentApplication<T>(effects: {
  applied: Set<string>
  requests: Map<string, Promise<T>>
  supports: (backend: string) => boolean
  apply: (params: {
    root: string; threadId: string; backend: string; includeThread: boolean
    excludeTurns: boolean; initialTurnsPage?: unknown
  }) => Promise<T>
  compatibilityError: (error: unknown) => boolean
  disableTail: (backend: string, threadId: string) => void
}) {
  return async function applyEnvironment(options: EnvironmentApplyOptions): Promise<T | null> {
    const { backend, threadId, generation, root, profileRevision, force, includeThread, excludeTurns, initialTurnsPage } = options
    if (!effects.supports(backend) || !threadId) return null
    const key = `${backend}\u0000${threadId}\u0000${generation}\u0000${root}\u0000${profileRevision}`
    if (!force && effects.applied.has(key)) return null
    const active = effects.requests.get(key)
    if (!force && active) return active
    const request = (async () => {
      const apply = (incremental: boolean) => effects.apply({
        root, threadId, backend, includeThread, excludeTurns: incremental,
        ...(incremental && initialTurnsPage ? { initialTurnsPage } : {}),
      })
      let result: T
      try {
        result = await apply(excludeTurns)
      } catch (error) {
        if (!excludeTurns || !effects.compatibilityError(error)) throw error
        effects.disableTail(backend, threadId)
        result = await apply(false)
      }
      effects.applied.add(key)
      return result
    })().finally(() => effects.requests.delete(key))
    effects.requests.set(key, request)
    return request
  }
}
