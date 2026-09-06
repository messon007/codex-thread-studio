import type { SessionCatalogEntry } from './backend-types.mjs'
import { reconcileStartedThreadCatalog } from './session-catalog.mjs'

interface StartedEntry {
  backend: string
  operation: string
  thread: SessionCatalogEntry
  catalogMissLogged?: boolean
}

export function createStartedSessionCatalog(effects: {
  generations: Map<string, number>
  key: (backend: string, id: string) => string
  catalog: (backend: string) => SessionCatalogEntry[]
  merge: (backend: string, thread: SessionCatalogEntry) => void
  report: (phase: string, details: Record<string, unknown>) => void
  refresh: (backend: string) => Promise<unknown>
  refreshError: (backend: string, error: unknown) => void
  missingId: () => string
}) {
  const pending = new Map<string, StartedEntry>()
  const timers = new Map<string, ReturnType<typeof setTimeout>>()
  function entries(backend: string) {
    return [...pending.entries()].filter(([key]) => key.startsWith(`${backend}:`)).map(([, entry]) => entry)
  }
  function remember(backend: string, thread: SessionCatalogEntry | null, operation: string): void {
    if (!thread?.id) throw new Error(effects.missingId())
    pending.set(effects.key(backend, thread.id), { backend, operation, thread: { ...thread, turns: undefined } })
    // A response started before creation must not replace this new catalog entry.
    effects.generations.set(backend, (effects.generations.get(backend) || 0) + 1)
    effects.merge(backend, thread)
    effects.report('started', { backend, threadId: thread.id, operation, awaitingCatalog: true })
  }
  function forget(backend: string, threadId: string | undefined, reason: string): boolean {
    if (!threadId) return false
    const key = effects.key(backend, threadId)
    const entry = pending.get(key)
    if (!entry) return false
    pending.delete(key)
    clearTimeout(timers.get(key))
    timers.delete(key)
    effects.report(reason, { backend, threadId, operation: entry.operation, awaitingCatalog: false })
    return true
  }
  function reconcile(backend: string, threads: SessionCatalogEntry[] | null): SessionCatalogEntry[] {
    const started = entries(backend)
    if (!started.length) return Array.isArray(threads) ? threads : []
    const currentCatalog = effects.catalog(backend)
    const startedThreads = started.map(entry => {
      const current = currentCatalog.find(thread => thread.id === entry.thread.id)
      if (current) entry.thread = { ...entry.thread, ...current, turns: undefined }
      return entry.thread
    })
    const result = reconcileStartedThreadCatalog(threads, startedThreads)
    for (const threadId of result.retainedIds) {
      if (!threadId) continue
      const entry = pending.get(effects.key(backend, threadId))
      if (!entry || entry.catalogMissLogged) continue
      entry.catalogMissLogged = true
      effects.report('catalog-retained', { backend, threadId, operation: entry.operation, awaitingCatalog: true })
    }
    for (const threadId of result.confirmedIds) forget(backend, threadId, 'catalog-confirmed')
    return result.threads
  }
  function schedule(backend: string, threadId: string, delay = 800): void {
    const key = effects.key(backend, threadId)
    if (!pending.has(key)) return
    clearTimeout(timers.get(key))
    timers.set(key, setTimeout(() => {
      timers.delete(key)
      if (!pending.has(key)) return
      effects.refresh(backend).catch(error => effects.refreshError(backend, error))
    }, delay))
  }
  function clearBackend(backend: string, reason: string): void {
    for (const entry of entries(backend)) forget(backend, entry.thread.id, reason)
  }
  return { remember, forget, reconcile, schedule, clearBackend }
}
