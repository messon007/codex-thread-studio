const activeStatuses = new Set(['active', 'running', 'inProgress'])

export function threadCatalogKey(backend, id) {
  return `${backend}:${id}`
}

export function isActiveCatalogThread(thread) {
  return activeStatuses.has(thread?.status?.type || thread?.status)
}

export function catalogEntries(catalogs = {}) {
  return ['codex', 'opencode'].flatMap((backend) =>
    (catalogs[backend] || []).map((thread) => ({ backend, thread })),
  )
}

export function catalogCountsWithAttention(catalogs = {}, attention = new Set()) {
  const entries = catalogEntries(catalogs)
  return {
    all: entries.length,
    active: entries.filter(({ thread }) => isActiveCatalogThread(thread)).length,
    attention: entries.filter(({ backend, thread }) => attention.has(threadCatalogKey(backend, thread.id))).length,
  }
}

export function groupCatalogEntries(entries = []) {
  const groups = new Map()
  for (const entry of entries) {
    const cwd = entry.thread.cwd || ''
    if (!groups.has(cwd)) groups.set(cwd, [])
    groups.get(cwd).push(entry)
  }
  return [...groups].map(([cwd, groupEntries]) => ({
    cwd,
    name: String(cwd).split(/[\\/]/u).filter(Boolean).at(-1) || '',
    entries: groupEntries,
  }))
}

export function filterCatalogEntries(catalogs, {
  filter = 'all',
  search = '',
  activity = {},
  attention = new Set(),
} = {}) {
  const query = search.trim().toLowerCase()
  const entries = catalogEntries(catalogs).filter(({ backend, thread }) => {
    if (filter === 'active' && !isActiveCatalogThread(thread)) return false
    if (filter === 'attention' && !attention.has(threadCatalogKey(backend, thread.id))) return false
    if (!query) return true
    return [
      thread.name,
      thread.title,
      thread.preview,
      thread.cwd,
      thread.id,
      backend,
      backend === 'codex' ? 'cx codex' : 'oc opencode',
    ].filter(Boolean).join(' ').toLowerCase().includes(query)
  })

  return entries.sort((left, right) => {
    const leftActivity = Number(activity[threadCatalogKey(left.backend, left.thread.id)] || 0)
    const rightActivity = Number(activity[threadCatalogKey(right.backend, right.thread.id)] || 0)
    const leftUpdated = catalogTimestamp(left.thread.updatedAt || left.thread.updated_at || left.thread.createdAt)
    const rightUpdated = catalogTimestamp(right.thread.updatedAt || right.thread.updated_at || right.thread.createdAt)
    if (filter === 'attention' && rightActivity !== leftActivity) return rightActivity - leftActivity
    if (isActiveCatalogThread(left.thread) !== isActiveCatalogThread(right.thread)) {
      return isActiveCatalogThread(left.thread) ? -1 : 1
    }
    if (rightUpdated !== leftUpdated) return rightUpdated - leftUpdated
    if (rightActivity !== leftActivity) return rightActivity - leftActivity
    return String(left.thread.name || left.thread.title || left.thread.id)
      .localeCompare(String(right.thread.name || right.thread.title || right.thread.id))
  })
}

export function catalogTimestamp(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  const numeric = Number(value)
  if (Number.isFinite(numeric)) return numeric
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}
