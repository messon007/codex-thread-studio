import { threadCatalogKey } from './thread-catalog.mjs'

export function addLoadedThread(loadedThreads, backend, id) {
  if (!id) return false
  const key = threadCatalogKey(backend, id)
  if (loadedThreads.has(key)) return false
  loadedThreads.add(key)
  return true
}

export function updateLoadedCatalogTimestamp(catalogs, loadedThreads, backend, id, timestamp) {
  if (!loadedThreads.has(threadCatalogKey(backend, id))) return false
  const thread = (catalogs[backend] || []).find((candidate) => candidate.id === id)
  if (!thread) return false
  thread.updatedAt = timestamp
  return true
}
