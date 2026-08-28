import { BACKEND_IDS, backendSearchAliases } from './backends.mjs'

const activeStatuses = new Set(['active', 'running', 'inProgress'])

export const SIDEBAR_TEXT_LIMIT = 200

export function compactSidebarText(value, limit = SIDEBAR_TEXT_LIMIT) {
  const maximum = Math.max(1, Number.isFinite(limit) ? Math.floor(limit) : SIDEBAR_TEXT_LIMIT)
  const characters = []
  let pendingSpace = false
  let truncated = false
  for (const character of String(value ?? '')) {
    if (/\s/u.test(character)) {
      if (characters.length) pendingSpace = true
      continue
    }
    if (pendingSpace) {
      if (characters.length >= maximum) {
        truncated = true
        break
      }
      characters.push(' ')
      pendingSpace = false
    }
    if (characters.length >= maximum) {
      truncated = true
      break
    }
    characters.push(character)
  }
  if (!truncated) return characters.join('')
  if (maximum === 1) return '…'
  return `${characters.slice(0, maximum - 1).join('').trimEnd()}…`
}

export function syncCatalogSelection(rows, backend, selectedId) {
  for (const row of rows || []) {
    const selected = row?.dataset?.backend === backend
      && row?.dataset?.threadId === String(selectedId || '')
    row?.classList?.toggle('active', selected)
  }
}

export function threadCatalogKey(backend, id) {
  return `${backend}:${id}`
}

export function isActiveCatalogThread(thread) {
  return activeStatuses.has(thread?.status?.type || thread?.status)
}

export function catalogEntries(catalogs = {}) {
  return BACKEND_IDS.flatMap((backend) =>
    (catalogs[backend] || []).map((thread) => ({ backend, thread })),
  )
}

export function catalogCountsWithAttention(catalogs = {}, attention = new Set(), hiddenDirectories = [], ignorePatterns = []) {
  const entries = catalogEntries(catalogs)
    .filter(({ thread }) => !isSessionDirectoryHidden(thread?.cwd, hiddenDirectories, ignorePatterns))
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
  attention = new Set(),
  hiddenDirectories = [],
  ignorePatterns = [],
} = {}) {
  const query = search.trim().toLowerCase()
  const entries = catalogEntries(catalogs).filter(({ backend, thread }) => {
    if (isSessionDirectoryHidden(thread?.cwd, hiddenDirectories, ignorePatterns)) return false
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
      backendSearchAliases(backend),
    ].filter(Boolean).join(' ').toLowerCase().includes(query)
  })

  if (filter !== 'attention') return entries

  return entries.sort((left, right) => {
    const leftUpdated = catalogTimestamp(left.thread.updatedAt || left.thread.updated_at || left.thread.createdAt)
    const rightUpdated = catalogTimestamp(right.thread.updatedAt || right.thread.updated_at || right.thread.createdAt)
    if (rightUpdated !== leftUpdated) return rightUpdated - leftUpdated
    return String(left.thread.name || left.thread.title || left.thread.id)
      .localeCompare(String(right.thread.name || right.thread.title || right.thread.id))
  })
}

export function normalizeHiddenSessionDirectories(values = []) {
  if (!Array.isArray(values)) return []
  return [...new Set(values.map(normalizeDirectoryPath).filter(Boolean))]
}

export function isSessionDirectoryHidden(directory, hiddenDirectories = [], ignorePatterns = []) {
  const path = normalizeDirectoryPath(directory)
  if (!path) return false
  let hidden = normalizeHiddenSessionDirectories(hiddenDirectories).some((hidden) => {
    const caseInsensitive = isWindowsDirectory(path) || isWindowsDirectory(hidden)
    const candidate = caseInsensitive ? path.toLowerCase() : path
    const prefix = caseInsensitive ? hidden.toLowerCase() : hidden
    return candidate === prefix || candidate.startsWith(`${prefix}/`)
  })
  for (const rule of normalizeSessionDirectoryIgnore(ignorePatterns)) {
    if (rule.regex.test(path)) hidden = !rule.negated
  }
  return hidden
}

export function normalizeSessionDirectoryIgnore(patterns = []) {
  if (!Array.isArray(patterns)) return []
  return patterns.map(parseIgnoreRule).filter(Boolean)
}

function parseIgnoreRule(value) {
  let pattern = String(value || '').replace(/\s+$/u, '')
  if (!pattern || pattern.startsWith('#')) return null
  let negated = false
  if (pattern.startsWith('!')) {
    negated = true
    pattern = pattern.slice(1)
  } else if (pattern.startsWith('\\!') || pattern.startsWith('\\#')) {
    pattern = pattern.slice(1)
  }
  if (!pattern) return null
  pattern = pattern.replaceAll('\\', '/')
  const directoryOnly = pattern.endsWith('/')
  pattern = pattern.replace(/\/+$/u, '')
  if (!pattern) return null
  const anchored = pattern.startsWith('/') || /^[a-z]:\//iu.test(pattern) || pattern.startsWith('//')
  const containsSlash = pattern.includes('/')
  const body = globPatternSource(pattern)
  const start = anchored ? '^' : containsSlash ? '(?:^|.*/)' : '(?:^|/)'
  const end = directoryOnly || containsSlash ? '(?:/.*)?$' : '(?:/|$)'
  try {
    return {
      negated,
      regex: new RegExp(`${start}${body}${end}`, isWindowsDirectory(pattern) ? 'iu' : 'u'),
    }
  } catch {
    return null
  }
}

function globPatternSource(pattern) {
  let source = ''
  for (let index = 0; index < pattern.length;) {
    const character = pattern[index]
    if (character === '*') {
      if (pattern[index + 1] === '*') {
        while (pattern[index + 1] === '*') index += 1
        if (pattern[index + 1] === '/') {
          source += '(?:.*/)?'
          index += 2
        } else {
          source += '.*'
          index += 1
        }
      } else {
        source += '[^/]*'
        index += 1
      }
    } else if (character === '?') {
      source += '[^/]'
      index += 1
    } else if (character === '[') {
      const closing = pattern.indexOf(']', index + 1)
      if (closing > index + 1) {
        let content = pattern.slice(index + 1, closing)
        if (content.startsWith('!')) content = `^${content.slice(1)}`
        source += `[${content.replaceAll('\\', '\\\\')}]`
        index = closing + 1
      } else {
        source += '\\['
        index += 1
      }
    } else {
      source += /[.+^${}()|\\]/u.test(character) ? `\\${character}` : character
      index += 1
    }
  }
  return source
}

function normalizeDirectoryPath(value) {
  let path = String(value || '').trim().replaceAll('\\', '/')
  if (!path) return ''
  const networkPrefix = path.startsWith('//') ? '//' : ''
  path = networkPrefix + path.slice(networkPrefix.length).replace(/\/{2,}/gu, '/')
  return path.length > 1 ? path.replace(/\/+$/gu, '') : path
}

function isWindowsDirectory(path) {
  return /^[a-z]:\//iu.test(path) || path.startsWith('//')
}

export function catalogTimestamp(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return normalizeEpoch(value)
  const numeric = Number(value)
  if (Number.isFinite(numeric)) return normalizeEpoch(numeric)
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

export function isCatalogCacheFresh(updatedAt, validatedAt, { coarse = false } = {}) {
  const updated = catalogTimestamp(updatedAt)
  const validated = Number(validatedAt)
  if (!updated) return true
  if (!Number.isFinite(validated)) return false
  // Codex state-db timestamps currently have one-second precision. Treat the
  // entire validation second as uncertain so an event missed by a reconnect in
  // that same second cannot be hidden behind an apparently fresh cache.
  if (coarse) return updated < Math.floor(validated / 1000) * 1000
  return updated <= validated
}

function normalizeEpoch(value) {
  return value > 0 && value < 100_000_000_000 ? value * 1000 : value
}
