import type { BackendKind, SessionCatalogEntry } from './backend-types.mjs'

const CODEX_CATALOG_FIELDS = Object.freeze([
  'cwd',
  'name',
  'updatedAt',
  'recencyAt',
  'section',
  'sectionEnteredAt',
])

export function catalogListParams(kind: BackendKind, params: Record<string, unknown> = {}) {
  return kind === 'codex' ? { ...params, useStateDbOnly: true } : { ...params }
}

export function shouldRecoverCodexCatalog(catalog: readonly (SessionCatalogEntry | null)[] | null, preferredId: unknown) {
  const threads = Array.isArray(catalog) ? catalog : []
  if (!threads.length) return true
  const preferred = typeof preferredId === 'string' ? preferredId : ''
  return Boolean(preferred) && !threads.some((thread) => String(thread?.id || '') === preferred)
}

export function reconcileStartedThreadCatalog(catalog: readonly SessionCatalogEntry[] | null, startedThreads: readonly (SessionCatalogEntry | null)[] = []) {
  const listed: SessionCatalogEntry[] = Array.isArray(catalog) ? [...catalog] : []
  const listedIds = new Set(listed.map((thread) => String(thread?.id || '')).filter(Boolean))
  const retainedById = new Map<string, SessionCatalogEntry>()
  const confirmedIds: string[] = []

  for (const thread of Array.isArray(startedThreads) ? startedThreads : []) {
    const id = String(thread?.id || '')
    if (!id) continue
    if (listedIds.has(id)) {
      confirmedIds.push(id)
      continue
    }
    retainedById.set(id, { ...thread, turns: undefined })
  }

  const retained = [...retainedById.values()].reverse()
  return {
    threads: [...retained, ...listed],
    retainedIds: retained.map((thread) => thread.id),
    confirmedIds: [...new Set(confirmedIds)],
  }
}

export function mergeCatalogMetadata(kind: BackendKind, current: SessionCatalogEntry | null, incoming: SessionCatalogEntry | null) {
  const merged = { ...(current || {}), ...(incoming || {}) }
  if (kind !== 'codex' || !current) return merged
  for (const field of CODEX_CATALOG_FIELDS) {
    if (Object.hasOwn(current, field)) merged[field] = current[field]
  }
  return merged
}

export function turnStartParams(kind: BackendKind, thread: SessionCatalogEntry | null, params: Record<string, unknown> = {}) {
  if (kind !== 'codex' || !thread?.cwd) return { ...params }
  return { ...params, cwd: thread.cwd }
}
