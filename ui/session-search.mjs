const MAX_SEARCHABLE_TEXT = 200_000
const SNIPPET_CONTEXT = 82

export function isUnsupportedOccurrenceSearchError(error) {
  if (Number(error?.code) === -32601) return true
  const message = String(error?.message || error || '')
  return /method (?:not found|unknown)|unsupported(?: method)?|does not support|not supported/iu.test(message)
}

export function localSessionOccurrences(model, searchTerm, { includeMessages = true, includeActivity = true } = {}) {
  const query = String(searchTerm || '').trim()
  if (!query) return []
  const turns = Array.isArray(model?.turns) ? model.turns : []
  const results = []

  turns.forEach((turn, turnIndex) => {
    const items = Array.isArray(turn?.items) ? turn.items : []
    const finalAssistantIndex = findFinalAssistantIndex(items)
    items.forEach((item, itemIndex) => {
      const type = occurrenceType(item, itemIndex, finalAssistantIndex)
      if (type === 'activity' ? !includeActivity : !includeMessages) return
      const text = searchableItemText(item)
      const match = matchedSnippet(text, query)
      if (!match) return
      results.push({
        turnId: String(turn?.id || ''),
        itemId: String(item?.id || ''),
        type,
        snippet: match.snippet,
        snippetMatchRange: match.range,
        turnIndex,
        itemIndex,
        source: 'local',
      })
    })
  })

  return results
}

export function mergeSessionOccurrences(remote = [], local = []) {
  const localByItem = new Map(local.map((entry) => [`${entry.turnId}:${entry.itemId}`, entry]))
  const merged = remote.map((entry, remoteIndex) => {
    const localEntry = localByItem.get(`${entry.turnId}:${entry.itemId}`)
    return {
      ...entry,
      type: localEntry?.type || entry.type || 'assistant',
      turnIndex: localEntry?.turnIndex ?? Number.MAX_SAFE_INTEGER,
      itemIndex: localEntry?.itemIndex ?? remoteIndex,
      source: 'remote',
    }
  })
  const remoteItems = new Set(remote.map((entry) => `${entry.turnId}:${entry.itemId}`))
  for (const entry of local) {
    if (entry.type !== 'activity' && remoteItems.has(`${entry.turnId}:${entry.itemId}`)) continue
    merged.push(entry)
  }
  return merged.sort((left, right) => left.turnIndex - right.turnIndex || left.itemIndex - right.itemIndex)
}

export function normalizeRemoteSessionOccurrences(remote = [], model = {}) {
  const itemMetadata = new Map()
  for (const [turnIndex, turn] of (Array.isArray(model?.turns) ? model.turns : []).entries()) {
    const items = Array.isArray(turn?.items) ? turn.items : []
    const finalAssistantIndex = findFinalAssistantIndex(items)
    for (const [itemIndex, item] of items.entries()) {
      itemMetadata.set(`${turn?.id || ''}:${item?.id || ''}`, {
        type: occurrenceType(item, itemIndex, finalAssistantIndex),
        turnIndex,
        itemIndex,
      })
    }
  }
  return (Array.isArray(remote) ? remote : []).map((entry, remoteIndex) => {
    const metadata = itemMetadata.get(`${entry?.turnId || ''}:${entry?.itemId || ''}`)
    return {
      ...entry,
      type: metadata?.type || entry?.type || 'assistant',
      turnIndex: metadata?.turnIndex ?? Number.MAX_SAFE_INTEGER,
      itemIndex: metadata?.itemIndex ?? remoteIndex,
      source: 'remote',
    }
  }).sort((left, right) => left.turnIndex - right.turnIndex || left.itemIndex - right.itemIndex)
}

export function filterSessionOccurrences(entries = [], type = 'all') {
  return type === 'all' ? [...entries] : entries.filter((entry) => entry.type === type)
}

export function matchedSnippet(value, searchTerm) {
  const text = String(value || '').slice(0, MAX_SEARCHABLE_TEXT)
  const query = String(searchTerm || '').trim()
  if (!text || !query) return null
  const index = text.toLocaleLowerCase().indexOf(query.toLocaleLowerCase())
  if (index < 0) return null
  const start = Math.max(0, index - SNIPPET_CONTEXT)
  const end = Math.min(text.length, index + query.length + SNIPPET_CONTEXT)
  const prefix = start > 0 ? '…' : ''
  const suffix = end < text.length ? '…' : ''
  const snippet = `${prefix}${text.slice(start, end)}${suffix}`
  const matchStart = prefix.length + index - start
  return {
    snippet,
    range: { start: matchStart, end: matchStart + query.length },
  }
}

function occurrenceType(item, itemIndex, finalAssistantIndex) {
  if (item?.type === 'userMessage') return 'user'
  if ((item?.type === 'agentMessage' || item?.type === 'plan') && itemIndex === finalAssistantIndex) return 'assistant'
  return 'activity'
}

function findFinalAssistantIndex(items) {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    if (items[index]?.type === 'agentMessage' || items[index]?.type === 'plan') return index
  }
  return -1
}

function searchableItemText(item) {
  if (!item || typeof item !== 'object') return ''
  if (item.type === 'userMessage') return userContentText(item.content)
  const values = [
    item.text,
    arrayText(item.summary),
    arrayText(item.content),
    item.explanation,
    commandText(item.command),
    item.aggregatedOutput,
    item.query,
    item.reason,
    item.tool,
    item.server,
    planText(item.plan),
    changesText(item.changes),
    boundedValueText(item.result),
    boundedValueText(item.error),
  ]
  return values.filter(Boolean).join('\n').slice(0, MAX_SEARCHABLE_TEXT)
}

function userContentText(content) {
  if (!Array.isArray(content)) return String(content || '')
  return content.map((entry) => entry?.text || entry?.path || entry?.url || '').filter(Boolean).join('\n')
}

function arrayText(value) {
  return Array.isArray(value) ? value.map((entry) => typeof entry === 'string' ? entry : entry?.text || '').filter(Boolean).join('\n') : String(value || '')
}

function commandText(value) {
  return Array.isArray(value) ? value.join(' ') : String(value || '')
}

function planText(value) {
  return Array.isArray(value) ? value.map((entry) => entry?.step || entry?.text || '').filter(Boolean).join('\n') : ''
}

function changesText(value) {
  return Array.isArray(value) ? value.map((entry) => [entry?.kind, entry?.path].filter(Boolean).join(' ')).filter(Boolean).join('\n') : ''
}

function boundedValueText(value) {
  if (value == null) return ''
  if (typeof value === 'string') return value.slice(0, MAX_SEARCHABLE_TEXT)
  try { return JSON.stringify(value).slice(0, MAX_SEARCHABLE_TEXT) } catch { return String(value).slice(0, MAX_SEARCHABLE_TEXT) }
}
