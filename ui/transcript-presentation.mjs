const DEFAULT_VISIBLE_TURNS = 30
const OUTPUT_PREVIEW_LINES = 5

export class TranscriptPresentationCache {
  constructor({ visibleTurns = DEFAULT_VISIBLE_TURNS } = {}) {
    this.visibleTurns = Math.max(1, visibleTurns)
    this.threads = new Map()
  }

  get(threadKey, model) {
    const key = String(threadKey || '')
    let entry = this.threads.get(key)
    if (!entry) {
      entry = {
        turns: new Map(),
        orderedIds: [],
        historyWindow: this.visibleTurns,
        visibleStart: Math.max(0, (model?.turns?.length || 0) - this.visibleTurns),
        scrollTop: null,
      }
      this.threads.set(key, entry)
    }
    syncEntry(entry, model)
    return entry
  }

  invalidateTurn(threadKey, turnId) {
    this.threads.get(String(threadKey || ''))?.turns.delete(String(turnId || ''))
  }

  invalidateThread(threadKey) {
    this.threads.delete(String(threadKey || ''))
  }

  setScrollTop(threadKey, scrollTop) {
    const entry = this.threads.get(String(threadKey || ''))
    if (entry) entry.scrollTop = Number.isFinite(scrollTop) ? scrollTop : null
  }

  updateTurn(threadKey, model, turnId) {
    const key = String(threadKey || '')
    const id = String(turnId || '')
    const entry = this.threads.get(key)
    const turns = Array.isArray(model?.turns) ? model.turns : []
    if (!entry || entry.orderedIds.length !== turns.length) return this.get(key, model)
    const turn = turns.find((candidate) => String(candidate?.id || '') === id)
    if (!turn) return this.get(key, model)
    syncTurn(entry, turn)
    return entry
  }

  showTurn(threadKey, model, turnId) {
    const entry = this.get(threadKey, model)
    const index = entry.orderedIds.indexOf(String(turnId || ''))
    if (index >= 0 && index < entry.visibleStart) {
      entry.visibleStart = Math.max(0, index - 2)
      entry.historyWindow = Math.max(entry.historyWindow, entry.orderedIds.length - entry.visibleStart)
    }
    return entry
  }

  showEarlier(threadKey, model, count = 20) {
    const entry = this.get(threadKey, model)
    entry.historyWindow += Math.max(1, count)
    entry.visibleStart = Math.max(0, entry.orderedIds.length - entry.historyWindow)
    return entry
  }
}

export function presentTurn(turn) {
  const items = Array.isArray(turn?.items) ? turn.items : []
  const lastAssistantIndex = findLastAssistantIndex(items)
  const blocks = []
  let activityItems = []

  const flushActivity = () => {
    if (!activityItems.length) return
    blocks.push(buildActivityBlock(turn, activityItems))
    activityItems = []
  }

  items.forEach((item, index) => {
    if (item?.type === 'userMessage') {
      flushActivity()
      blocks.push({ type: 'user', itemId: item.id, item })
      return
    }
    if (item?.type === 'agentMessage' || item?.type === 'plan') {
      if (index === lastAssistantIndex) {
        flushActivity()
        blocks.push({ type: 'assistant', itemId: item.id, item, variant: item.type === 'plan' ? 'plan' : 'message' })
      } else {
        activityItems.push({ kind: 'progress', itemId: item.id, item })
      }
      return
    }
    activityItems.push(activityEntry(item))
  })
  flushActivity()

  if (turn?.status === 'failed' || turn?.error?.message) {
    blocks.push({ type: 'error', message: turn?.error?.message || 'Turn failed' })
  }

  return {
    id: String(turn?.id || ''),
    status: turn?.status || 'unknown',
    blocks,
    source: turn,
  }
}

export function shouldShowTurnPlaceholder(presentation) {
  if (presentation?.status !== 'inProgress') return false
  return !(presentation.blocks || []).some((block) => block.type === 'assistant' || block.type === 'activity')
}

export function activityOutputPreview(value, lineLimit = OUTPUT_PREVIEW_LINES) {
  const limit = Math.max(1, lineLimit)
  const headCount = Math.ceil(limit / 2)
  const tailCount = Math.floor(limit / 2)
  const first = []
  const tail = []
  const text = String(value || '')
  let lineStart = 0
  let lineCount = 0
  for (let index = 0; index <= text.length; index += 1) {
    if (index !== text.length && text[index] !== '\n') continue
    if (index === text.length && lineStart === text.length) break
    let line = text.slice(lineStart, index)
    if (line.endsWith('\r')) line = line.slice(0, -1)
    lineCount += 1
    if (first.length < limit) first.push(line)
    tail.push(line)
    if (tail.length > tailCount) tail.shift()
    lineStart = index + 1
  }
  if (lineCount <= limit) return { lines: first, omitted: 0 }
  return {
    lines: [...first.slice(0, headCount), ...tail],
    omitted: Math.max(0, lineCount - headCount - tailCount),
    splitAt: headCount,
  }
}

export function summarizeActivity(entries) {
  const summary = {
    reads: 0,
    searches: 0,
    lists: 0,
    commands: 0,
    tools: 0,
    webSearches: 0,
    changedFiles: 0,
    plans: 0,
    reasoning: 0,
    failures: 0,
  }
  for (const entry of entries) {
    if (entry.status === 'failed' || entry.item?.error) summary.failures += 1
    if (entry.kind === 'command') summary[entry.commandKind] += 1
    else if (entry.kind === 'tool') summary.tools += 1
    else if (entry.kind === 'search') summary.webSearches += 1
    else if (entry.kind === 'change') summary.changedFiles += entry.item?.changes?.length || 0
    else if (entry.kind === 'plan') summary.plans += 1
    else if (entry.kind === 'reasoning') summary.reasoning += 1
  }
  return summary
}

export function reasoningStage(item) {
  const parts = Array.isArray(item?.summary) && item.summary.length
    ? item.summary
    : Array.isArray(item?.content) ? item.content : [item?.summary || item?.content || '']
  const text = parts.map(String).map((part) => part.trim()).filter(Boolean).join('\n\n')
  const bold = text.match(/^\*\*([^*]+)\*\*/u)?.[1]
  const first = bold || text.split('\n').map((line) => line.trim()).find(Boolean) || ''
  return truncateText(first.replace(/^[-*•]\s*/u, ''), 140)
}

export function commandKind(command) {
  const text = Array.isArray(command) ? command.join(' ') : String(command || '')
  const normalized = text.replace(/^\s*(?:bash|sh|zsh)\s+(?:-[a-z]*c\s+)?/iu, '').trim()
  if (/(?:^|[;&|]\s*)(?:rg|grep|git\s+grep)\b/iu.test(normalized) && !/\brg\s+--files\b/iu.test(normalized)) return 'searches'
  if (/(?:^|[;&|]\s*)(?:ls|find|fd|tree)\b/iu.test(normalized) || /\brg\s+--files\b/iu.test(normalized)) return 'lists'
  if (/(?:^|[;&|]\s*)(?:cat|sed|head|tail|less|bat)\b/iu.test(normalized)) return 'reads'
  return 'commands'
}

function syncEntry(entry, model) {
  const turns = Array.isArray(model?.turns) ? model.turns : []
  const nextIds = turns.map((turn) => String(turn?.id || ''))
  const activeIds = new Set(nextIds)
  for (const id of entry.turns.keys()) if (!activeIds.has(id)) entry.turns.delete(id)
  for (const turn of turns) syncTurn(entry, turn)
  entry.orderedIds = nextIds
  entry.visibleStart = Math.max(0, nextIds.length - entry.historyWindow)
}

function syncTurn(entry, turn) {
  const id = String(turn?.id || '')
  const signature = turnSignature(turn)
  const cached = entry.turns.get(id)
  if (!cached || cached.signature !== signature) {
    entry.turns.set(id, { signature, presentation: presentTurn(turn) })
  }
}

function turnSignature(turn) {
  const items = Array.isArray(turn?.items) ? turn.items : []
  return [
    turn?.status || '',
    turn?.error?.message || '',
    ...items.map(itemSignature),
  ].join('|')
}

function itemSignature(item) {
  const changes = Array.isArray(item?.changes) ? item.changes : []
  return [
    item?.id || '', item?.type || '', item?.status || '',
    textFingerprint(item?.text), textFingerprint(arrayText(item?.summary)), valueFingerprint(item?.content),
    textFingerprint(item?.aggregatedOutput), changesFingerprint(changes), valueFingerprint(item?.result), valueFingerprint(item?.error),
  ].join(':')
}

function buildActivityBlock(turn, entries) {
  const active = turn?.status === 'inProgress' || entries.some((entry) => entry.status === 'inProgress')
  const latestStage = [...entries].reverse().map((entry) => {
    if (entry.kind === 'reasoning') return reasoningStage(entry.item)
    if (entry.kind === 'progress') return truncateText(entry.item?.text || '', 180)
    return ''
  }).find(Boolean) || ''
  return {
    type: 'activity',
    id: `activity-${turn?.id || 'turn'}`,
    active,
    entries,
    displayEntries: collapseActivityEntries(entries),
    summary: summarizeActivity(entries),
    latestStage,
    sourceItemIds: entries.map((entry) => entry.itemId).filter(Boolean),
  }
}

function collapseActivityEntries(entries) {
  const collapsed = []
  for (const entry of entries) {
    const previous = collapsed.at(-1)
    if (entry.kind === 'reasoning' && previous?.kind === 'reasoning') {
      collapsed[collapsed.length - 1] = entry
    } else {
      collapsed.push(entry)
    }
  }
  return collapsed
}

function activityEntry(item) {
  const common = { itemId: item?.id, item, status: item?.status || 'completed' }
  if (item?.type === 'reasoning') return { ...common, kind: 'reasoning' }
  if (item?.type === 'commandExecution') return { ...common, kind: 'command', commandKind: commandKind(item.command) }
  if (item?.type === 'fileChange') return { ...common, kind: 'change' }
  if (item?.type === 'planUpdate') return { ...common, kind: 'plan' }
  if (item?.type === 'webSearch') return { ...common, kind: 'search' }
  if (item?.type === 'mcpToolCall' || item?.type === 'collabToolCall') return { ...common, kind: 'tool' }
  if (item?.type === 'contextCompaction' || item?.type === 'stepFinish') return { ...common, kind: 'system' }
  return { ...common, kind: 'unknown' }
}

function findLastAssistantIndex(items) {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const type = items[index]?.type
    if (type === 'contextCompaction' || type === 'stepFinish') continue
    return type === 'agentMessage' || type === 'plan' ? index : -1
  }
  return -1
}

function arrayText(value) {
  if (Array.isArray(value)) return value.map(String).join('\n')
  return String(value || '')
}

function changesFingerprint(changes) {
  return changes.map((change) => [change?.kind, change?.path, textFingerprint(change?.diff)].join(',')).join(';')
}

function valueFingerprint(value, depth = 0) {
  if (value == null) return ''
  if (typeof value === 'string') return textFingerprint(value)
  if (typeof value !== 'object') return String(value)
  if (depth >= 2) return Array.isArray(value) ? `array:${value.length}` : `object:${Object.keys(value).length}`
  if (Array.isArray(value)) {
    const sampled = value.length > 6 ? [...value.slice(0, 3), ...value.slice(-3)] : value
    return `a${value.length}[${sampled.map((part) => valueFingerprint(part, depth + 1)).join(',')}]`
  }
  const keys = Object.keys(value).sort()
  const sampled = keys.length > 8 ? [...keys.slice(0, 4), ...keys.slice(-4)] : keys
  return `o${keys.length}{${sampled.map((key) => `${key}=${valueFingerprint(value[key], depth + 1)}`).join(',')}}`
}

function textFingerprint(value) {
  const text = String(value || '')
  if (!text) return '0'
  const sample = text.length <= 256 ? text : `${text.slice(0, 128)}${text.slice(-128)}`
  let hash = 2166136261
  for (let index = 0; index < sample.length; index += 1) {
    hash ^= sample.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return `${text.length}.${(hash >>> 0).toString(36)}`
}

function truncateText(value, max) {
  const text = String(value || '').replace(/\s+/gu, ' ').trim()
  return text.length <= max ? text : `${text.slice(0, Math.max(0, max - 1)).trimEnd()}…`
}
