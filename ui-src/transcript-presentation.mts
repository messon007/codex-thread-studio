import type { PresentationItem, PresentationTurn, PresentationModel, PresentationCacheEntry, ScrollState, ActivityEntry, ActivitySummary, ActivityBlock, MessageBlock, PresentationBlock, TurnPresentation } from './transcript-types.mjs'

import { transcriptModelRevision } from './model-revision.mjs'

const DEFAULT_VISIBLE_TURNS = 30
const OUTPUT_PREVIEW_LINES = 5

export class TranscriptPresentationCache {
  visibleTurns: number
  threads: Map<string, PresentationCacheEntry>

  constructor({ visibleTurns = DEFAULT_VISIBLE_TURNS } = {}) {
    this.visibleTurns = Math.max(1, visibleTurns)
    this.threads = new Map()
  }

  get(threadKey: string, model: PresentationModel | null) {
    return this._prepareEntry(threadKey, model)
  }

  _prepareEntry(threadKey: string, model: PresentationModel | null, { present = true } = {}) {
    const key = String(threadKey || '')
    let entry = this.threads.get(key)
    if (!entry) {
      const turnCount = model?.turns?.length || 0
      entry = {
        turns: new Map(),
        sourceModel: null,
        sourceRevision: null,
        sourceTurns: [],
        orderedIds: [],
        historyWindow: this.visibleTurns,
        visibleStart: Math.max(0, turnCount - this.visibleTurns),
        visibleEnd: turnCount,
        windowMode: 'latest',
        scrollTop: null,
        scrollState: null,
      }
      this.threads.set(key, entry)
    }
    const revision = transcriptModelRevision(model)
    if (revision != null && entry.sourceModel === model && entry.sourceRevision === revision) return entry
    syncEntry(entry, model, this.visibleTurns, { present })
    entry.sourceModel = model
    entry.sourceRevision = revision
    return entry
  }

  peek(threadKey: string) {
    return this.threads.get(String(threadKey || '')) || null
  }

  peekCurrent(threadKey: string, model: PresentationModel | null) {
    const entry = this.peek(threadKey)
    const revision = transcriptModelRevision(model)
    return revision != null && entry?.sourceModel === model && entry.sourceRevision === revision
      ? entry
      : null
  }

  invalidateTurn(threadKey: string, turnId: string) {
    this.threads.get(String(threadKey || ''))?.turns.delete(String(turnId || ''))
  }

  invalidateThread(threadKey: string) {
    this.threads.delete(String(threadKey || ''))
  }

  setScrollTop(threadKey: string, scrollTop: number) {
    const entry = this.threads.get(String(threadKey || ''))
    if (entry) entry.scrollTop = Number.isFinite(scrollTop) ? scrollTop : null
  }

  setScrollState(threadKey: string, scrollState: ScrollState | null) {
    const entry = this.threads.get(String(threadKey || ''))
    if (!entry) return
    entry.scrollState = scrollState ? { ...scrollState } : null
    entry.scrollTop = typeof scrollState?.scrollTop === 'number' && Number.isFinite(scrollState.scrollTop) ? scrollState.scrollTop : null
  }

  scrollState(threadKey: string) {
    const state = this.threads.get(String(threadKey || ''))?.scrollState
    return state ? { ...state } : null
  }

  updateTurn(threadKey: string, model: PresentationModel | null, turnId: string) {
    const key = String(threadKey || '')
    const id = String(turnId || '')
    const entry = this.threads.get(key)
    const turns = Array.isArray(model?.turns) ? model.turns : []
    if (!entry || entry.orderedIds.length !== turns.length) return this.get(key, model)
    const turn = turns.find((candidate) => String(candidate?.id || '') === id)
    if (!turn) return this.get(key, model)
    entry.sourceTurns = turns
    syncTurn(entry, turn)
    entry.sourceModel = model
    entry.sourceRevision = transcriptModelRevision(model)
    return entry
  }

  showTurn(threadKey: string, model: PresentationModel | null, turnId: string) {
    const entry = this.revealTurn(this.get(threadKey, model), turnId)
    pinWindow(entry, this.visibleTurns)
    return entry
  }

  restoreTurn(threadKey: string, model: PresentationModel | null, turnId: string) {
    const entry = this.revealTurn(this._prepareEntry(threadKey, model, { present: false }), turnId, { force: true, centered: true })
    pinWindow(entry, this.visibleTurns)
    return entry
  }

  revealTurn(entry: PresentationCacheEntry, turnId: string, { force = false, centered = force }: { force?: boolean; centered?: boolean } = {}) {
    if (!entry) return entry
    const index = entry.orderedIds.indexOf(String(turnId || ''))
    if (index >= 0 && (force || index < entry.visibleStart || index >= entry.visibleEnd)) {
      const turnCount = entry.orderedIds.length
      const contextBefore = centered
        ? Math.floor(this.visibleTurns / 2)
        : Math.min(2, Math.max(0, this.visibleTurns - 1))
      let visibleStart = Math.max(0, index - contextBefore)
      const visibleEnd = Math.min(turnCount, visibleStart + this.visibleTurns)
      visibleStart = Math.max(0, visibleEnd - this.visibleTurns)
      entry.visibleStart = visibleStart
      entry.visibleEnd = visibleEnd
      entry.windowMode = 'fixed'
      entry.historyWindow = this.visibleTurns
      syncVisibleTurns(entry)
    } else if (index < 0) {
      // restoreTurn prepares metadata without presenting the previous window.
      // A missing target still needs a usable bounded fallback presentation.
      syncVisibleTurns(entry)
    }
    return entry
  }

  showEarlier(threadKey: string, model: PresentationModel | null, count = 20) {
    const entry = this.get(threadKey, model)
    const previousStart = entry.visibleStart
    entry.visibleStart = Math.max(0, entry.visibleStart - Math.max(1, count))
    pinWindow(entry, this.visibleTurns)
    if (entry.visibleStart !== previousStart) syncVisibleTurns(entry)
    return entry
  }

  showLater(threadKey: string, model: PresentationModel | null, count = 20) {
    const entry = this.get(threadKey, model)
    const previousEnd = entry.visibleEnd
    entry.visibleEnd = Math.min(entry.orderedIds.length, entry.visibleEnd + Math.max(1, count))
    pinWindow(entry, this.visibleTurns)
    if (entry.visibleEnd !== previousEnd) syncVisibleTurns(entry)
    return entry
  }

  followLatest(threadKey: string, model: PresentationModel | null) {
    const entry = this.get(threadKey, model)
    const visibleEnd = entry.orderedIds.length
    const visibleStart = Math.max(0, visibleEnd - this.visibleTurns)
    const windowChanged = entry.visibleStart !== visibleStart || entry.visibleEnd !== visibleEnd
    entry.windowMode = 'latest'
    entry.historyWindow = this.visibleTurns
    entry.visibleStart = visibleStart
    entry.visibleEnd = visibleEnd
    if (windowChanged) syncVisibleTurns(entry)
    return entry
  }

  pinCurrent(threadKey: string) {
    const entry = this.peek(threadKey)
    pinWindow(entry, this.visibleTurns)
    return entry
  }
}

function pinWindow(entry: PresentationCacheEntry | null, visibleTurns: number) {
  if (!entry) return
  entry.windowMode = 'fixed'
  entry.historyWindow = visibleTurns
}

export function presentTurn(turn: PresentationTurn): TurnPresentation {
  const items = Array.isArray(turn?.items) ? turn.items : []
  const finalAssistant = findFinalAssistant(items, turn?.status)
  const blocks: PresentationBlock[] = []
  let activityItems: ActivityEntry[] = []
  let activityIndex = 0
  let deferredAssistantBlock: MessageBlock | null = null

  const flushActivity = () => {
    if (!activityItems.length) return
    blocks.push(buildActivityBlock(turn, activityItems, activityIndex))
    activityIndex += 1
    activityItems = []
  }

  items.forEach((item, index) => {
    if (item?.type === 'userMessage') {
      flushActivity()
      blocks.push({ type: 'user', itemId: item.id, item })
      return
    }
    if (item?.type === 'agentMessage' || item?.type === 'plan') {
      if (index === finalAssistant.index) {
        const assistantBlock: MessageBlock = { type: 'assistant', itemId: item.id, item, variant: item.type === 'plan' ? 'plan' : 'message' }
        if (finalAssistant.deferUntilAfterActivity) deferredAssistantBlock = assistantBlock
        else {
          flushActivity()
          blocks.push(assistantBlock)
        }
      } else {
        activityItems.push({ kind: 'progress', itemId: item.id, item })
      }
      return
    }
    activityItems.push(activityEntry(item))
  })
  flushActivity()
  if (deferredAssistantBlock) blocks.push(deferredAssistantBlock)

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

export function presentationActivityBlocks(presentation: TurnPresentation | null) {
  return (presentation?.blocks || []).filter((block) => block.type === 'activity')
}

export function presentationActivityEntries(presentation: TurnPresentation | null) {
  return presentationActivityBlocks(presentation).flatMap((block) => block.entries || [])
}

export function shouldShowTurnPlaceholder(presentation: TurnPresentation | null) {
  if (presentation?.status !== 'inProgress') return false
  return !(presentation.blocks || []).some((block) => block.type === 'assistant' || block.type === 'activity')
}

export function activityOutputPreview(value: unknown, lineLimit = OUTPUT_PREVIEW_LINES) {
  const limit = Math.max(1, lineLimit)
  const headCount = Math.ceil(limit / 2)
  const tailCount = Math.floor(limit / 2)
  const first = []
  const tail = []
  const text = String(value || '')
  let lineStart = 0
  let lineCount = 0
  while (lineStart < text.length) {
    const newline = text.indexOf('\n', lineStart)
    const index = newline < 0 ? text.length : newline
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

export function summarizeActivity(entries: ActivityEntry[]) {
  const summary: ActivitySummary = {
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

export function reasoningStage(item: PresentationItem | null) {
  const parts = Array.isArray(item?.summary) && item.summary.length
    ? item.summary
    : Array.isArray(item?.content) ? item.content : [item?.summary || item?.content || '']
  const text = firstBoundedStagePart(parts)
  const bold = text.match(/^\*\*([^*]+)\*\*/u)?.[1]
  const lineEnd = text.search(/[\r\n]/u)
  const first = bold || (lineEnd >= 0 ? text.slice(0, lineEnd) : text)
  return boundedTextSummary(first.replace(/^[-*•]\s*/u, ''), 140)
}

export function commandKind(command: unknown) {
  const text = Array.isArray(command) ? command.join(' ') : String(command || '')
  const normalized = text.replace(/^\s*(?:bash|sh|zsh)\s+(?:-[a-z]*c\s+)?/iu, '').trim()
  if (/(?:^|[;&|]\s*)(?:rg|grep|git\s+grep)\b/iu.test(normalized) && !/\brg\s+--files\b/iu.test(normalized)) return 'searches'
  if (/(?:^|[;&|]\s*)(?:ls|find|fd|tree)\b/iu.test(normalized) || /\brg\s+--files\b/iu.test(normalized)) return 'lists'
  if (/(?:^|[;&|]\s*)(?:cat|sed|head|tail|less|bat)\b/iu.test(normalized)) return 'reads'
  return 'commands'
}

function syncEntry(entry: PresentationCacheEntry, model: PresentationModel | null, visibleTurns: number, { present = true } = {}) {
  const turns = Array.isArray(model?.turns) ? model.turns : []
  const nextIds = turns.map((turn) => String(turn?.id || ''))
  const previousIds = entry.orderedIds
  if (entry.windowMode === 'fixed') {
    syncFixedWindow(entry, previousIds, nextIds, visibleTurns)
  } else {
    entry.visibleEnd = nextIds.length
    entry.visibleStart = Math.max(0, nextIds.length - entry.historyWindow)
  }
  entry.orderedIds = nextIds
  entry.sourceTurns = turns
  if (present) syncVisibleTurns(entry)
}

function syncVisibleTurns(entry: PresentationCacheEntry) {
  const visibleIds = new Set(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd))
  for (const id of entry.turns.keys()) {
    if (!visibleIds.has(id)) entry.turns.delete(id)
  }
  for (let index = entry.visibleStart; index < entry.visibleEnd; index += 1) {
    const turn = entry.sourceTurns[index]
    if (turn && String(turn?.id || '') === entry.orderedIds[index]) syncTurn(entry, turn)
  }
}

function syncFixedWindow(entry: PresentationCacheEntry, previousIds: string[], nextIds: string[], visibleTurns: number) {
  const previousStart = Math.max(0, Math.min(previousIds.length, entry.visibleStart))
  const previousEnd = Math.max(previousStart, Math.min(previousIds.length, entry.visibleEnd))
  const previousSpan = previousEnd - previousStart
  const nextIndexes = new Map<string, number>()
  nextIds.forEach((id, index) => {
    if (!nextIndexes.has(id)) nextIndexes.set(id, index)
  })

  let stable: { nextIndex: number; offset: number } | null = null
  for (let index = previousStart; index < previousEnd; index += 1) {
    const nextIndex = nextIndexes.get(previousIds[index]!)
    if (nextIndex == null) continue
    stable = { nextIndex, offset: index - previousStart }
    break
  }

  if (!stable || previousSpan === 0) {
    entry.windowMode = 'latest'
    entry.historyWindow = visibleTurns
    entry.visibleEnd = nextIds.length
    entry.visibleStart = Math.max(0, nextIds.length - visibleTurns)
    return
  }

  const span = Math.min(previousSpan, nextIds.length)
  let visibleStart = stable.nextIndex - Math.min(stable.offset, Math.max(0, span - 1))
  visibleStart = Math.max(0, Math.min(visibleStart, nextIds.length - span))
  entry.visibleStart = visibleStart
  entry.visibleEnd = visibleStart + span
}

function syncTurn(entry: PresentationCacheEntry, turn: PresentationTurn) {
  const id = String(turn?.id || '')
  const signature = turnSignature(turn)
  const cached = entry.turns.get(id)
  if (!cached || cached.signature !== signature) {
    entry.turns.set(id, { signature, presentation: presentTurn(turn) })
  }
}

function turnSignature(turn: PresentationTurn) {
  const items = Array.isArray(turn?.items) ? turn.items : []
  return [
    turn?.status || '',
    turn?.error?.message || '',
    ...items.map(itemSignature),
  ].join('|')
}

function itemSignature(item: PresentationItem) {
  const changes = Array.isArray(item?.changes) ? item.changes : []
  return [
    item?.id || '', item?.type || '', item?.status || '',
    textFingerprint(item?.text), arrayTextFingerprint(item?.summary), valueFingerprint(item?.content),
    textFingerprint(item?.aggregatedOutput), changesFingerprint(changes), valueFingerprint(item?.result), valueFingerprint(item?.error),
  ].join(':')
}

function buildActivityBlock(turn: PresentationTurn, entries: ActivityEntry[], activityIndex = 0): ActivityBlock {
  const active = turn?.status === 'inProgress' || entries.some((entry) => entry.status === 'inProgress')
  let latestStage = ''
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry = entries[index]!
    if (entry.kind === 'reasoning') latestStage = reasoningStage(entry.item)
    else if (entry.kind === 'progress') latestStage = boundedTextSummary(entry.item?.text, 180)
    if (latestStage) break
  }
  return {
    type: 'activity',
    id: `activity-${turn?.id || 'turn'}-${activityIndex}`,
    active,
    entries,
    displayEntries: collapseActivityEntries(entries),
    summary: summarizeActivity(entries),
    latestStage,
    sourceItemIds: entries.map((entry) => entry.itemId).filter(Boolean),
  }
}

function collapseActivityEntries(entries: ActivityEntry[]) {
  const collapsed: ActivityEntry[] = []
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

function activityEntry(item: PresentationItem): ActivityEntry {
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

function findFinalAssistant(items: PresentationItem[], turnStatus: string | undefined) {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const type = items[index]?.type
    if (type === 'contextCompaction' || type === 'stepFinish') continue
    if (type === 'agentMessage' || type === 'plan') {
      return { index, deferUntilAfterActivity: false }
    }
    break
  }

  // Some provider adapters reuse an earlier assistant item id for the final
  // answer. App-server history then keeps that item's old position while
  // replacing its content, leaving completed commands after the final text.
  // Recover only completed, phase-less/final assistant messages; explicit
  // commentary remains progress so active and incomplete turns are unchanged.
  if (turnStatus === 'completed') {
    for (let index = items.length - 1; index >= 0; index -= 1) {
      const item = items[index]
      if (item?.type === 'userMessage') break
      if (item?.type !== 'agentMessage') continue
      if (item.phase === 'commentary') return { index: -1, deferUntilAfterActivity: false }
      return { index, deferUntilAfterActivity: true }
    }
  }

  return { index: -1, deferUntilAfterActivity: false }
}

function changesFingerprint(changes: NonNullable<PresentationItem['changes']>) {
  return changes.map((change) => [change?.kind, change?.path, textFingerprint(change?.diff)].join(',')).join(';')
}

function arrayTextFingerprint(value: unknown) {
  if (!Array.isArray(value)) return textFingerprint(value)
  let totalLength = Math.max(0, value.length - 1)
  let prefix = ''
  let tail = ''
  for (let index = 0; index < value.length; index += 1) {
    const text = index in value ? String(value[index]) : ''
    totalLength += text.length
    if (prefix.length < 256) {
      if (index > 0) prefix += '\n'
      prefix += text.slice(0, Math.max(0, 256 - prefix.length))
    }
    if (index > 0) tail = appendTailSample(tail, '\n')
    tail = appendTailSample(tail, text)
  }
  if (!totalLength) return '0'
  const sample = totalLength <= 256 ? prefix : `${prefix.slice(0, 128)}${tail.slice(-128)}`
  return fingerprintTextSample(totalLength, sample)
}

function appendTailSample(tail: string, text: string) {
  if (text.length >= 128) return text.slice(-128)
  return `${tail}${text}`.slice(-128)
}

function valueFingerprint(value: unknown, depth = 0): string {
  if (value == null) return ''
  if (typeof value === 'string') return textFingerprint(value)
  if (typeof value !== 'object') return String(value)
  if (depth >= 2) return Array.isArray(value) ? `array:${value.length}` : `object:${Object.keys(value).length}`
  if (Array.isArray(value)) {
    const sampled = value.length > 6 ? [...value.slice(0, 3), ...value.slice(-3)] : value
    return `a${value.length}[${sampled.map((part) => valueFingerprint(part, depth + 1)).join(',')}]`
  }
  if (!isRecord(value)) return String(value)
  const keys = Object.keys(value).sort()
  const sampled = keys.length > 8 ? [...keys.slice(0, 4), ...keys.slice(-4)] : keys
  return `o${keys.length}{${sampled.map((key) => `${key}=${valueFingerprint(value[key], depth + 1)}`).join(',')}}`
}

function textFingerprint(value: unknown) {
  const text = String(value || '')
  if (!text) return '0'
  const sample = text.length <= 256 ? text : `${text.slice(0, 128)}${text.slice(-128)}`
  return fingerprintTextSample(text.length, sample)
}

function fingerprintTextSample(length: number, sample: string) {
  let hash = 2166136261
  for (let index = 0; index < sample.length; index += 1) {
    hash ^= sample.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return `${length}.${(hash >>> 0).toString(36)}`
}

const STAGE_SCAN_CHARS = 2_048
const STAGE_SCAN_PARTS = 32

function boundedTextSummary(value: unknown, maxLength: number, scanLimit = STAGE_SCAN_CHARS) {
  const source = String(value || '')
  const maximum = Math.max(1, Math.floor(Number(maxLength) || 1))
  const limit = Math.max(maximum, Math.floor(Number(scanLimit) || STAGE_SCAN_CHARS))
  const sampleLength = Math.min(source.length, limit)
  let output = ''
  let pendingSpace = false
  let overflow = source.length > sampleLength

  for (let index = 0; index < sampleLength; index += 1) {
    const character = source[index]!
    if (/\s/u.test(character)) {
      if (output) pendingSpace = true
      continue
    }
    if (pendingSpace && output) {
      if (output.length < maximum) output += ' '
      else overflow = true
      pendingSpace = false
    }
    if (output.length < maximum) output += character
    else {
      overflow = true
      break
    }
  }

  if (!output) return ''
  if (!overflow) return output
  if (maximum === 1) return '…'
  return `${output.slice(0, maximum - 1).trimEnd()}…`
}

function firstBoundedStagePart(parts: unknown[]) {
  let remaining = STAGE_SCAN_CHARS
  const count = Math.min(parts.length, STAGE_SCAN_PARTS)
  for (let index = 0; index < count && remaining > 0; index += 1) {
    const source = String(parts[index] || '')
    const sample = source.slice(0, remaining)
    remaining -= sample.length
    const trimmed = sample.trimStart()
    if (trimmed) return trimmed
  }
  return ''
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object'
}
