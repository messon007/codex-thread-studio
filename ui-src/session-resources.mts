import type { ResourceItem, ResourceTurn, ResourceModel, ResourceContext, ResourceSource, ResourceCandidate, RankedCandidate, ResourceExtractor, SessionResource, IndexedResource, ResourceOccurrence } from './resource-types.mjs'
const MAX_ITEM_TEXT = 1024 * 1024
const MAX_ITEM_OCCURRENCES = 100
const URL_PATTERN = /https?:\/\/[^\s<>"'`]+/giu
const MARKDOWN_TARGET_PATTERN = /!?\[[^\]\n]*\]\(([^\s)]+)(?:\s+["'][^"']*["'])?\)/gu
const INLINE_CODE_PATTERN = /`([^`\n]+)`/gu
const FILE_PATTERN = /(?:[a-z]:[\\/]|\.{0,2}[\\/])?(?:[\p{L}\p{N}_.@+-]+[\\/])+[\p{L}\p{N}_.@+-]+(?:\.[\p{L}\p{N}_.+-]+)?(?::\d+(?::\d+)?)?/giu
const FILE_EXTENSION_PATTERN = /\.(?:adoc|avif|bash|bmp|c|cc|cfg|cjs|cmake|conf|cpp|cs|css|csv|dart|doc|docx|epub|erl|ex|exs|fish|fs|fsx|gif|go|h|hpp|hrl|htm|html|ico|ini|java|jpeg|jpg|js|json|jsonl|jsx|kt|kts|less|lock|lua|m|md|mdx|mjs|mm|mov|mp3|mp4|odf|ods|odt|pdf|php|plist|png|ppt|pptx|properties|proto|ps1|py|rb|rs|rst|rtf|sass|scala|scss|sh|sql|svg|swift|toml|ts|tsv|tsx|txt|vb|vue|wasm|wav|webm|webp|xml|xls|xlsx|yaml|yml|zig|zsh)$/iu
const resourceObjectIds = new WeakMap<object, number>()
const resourceNarrativeRevisions = new WeakMap<object, { text: string; revision: number }>()
let nextResourceObjectId = 1
let nextResourceNarrativeRevision = 1

export const RESOURCE_KINDS = Object.freeze(['web', 'file', 'code', 'directory', 'issue', 'commit', 'artifact'])

export class ResourceExtractorRegistry {
  #extractors: (ResourceExtractor & { priority: number })[] = []

  register(extractor: ResourceExtractor) {
    const id = String(extractor?.id || '').trim()
    if (!id || typeof extractor.extract !== 'function') throw new Error('Resource extractor requires an id and extract()')
    if (this.#extractors.some((candidate) => candidate.id === id)) throw new Error(`Resource extractor already registered: ${id}`)
    this.#extractors.push(Object.freeze({ priority: 0, ...extractor, id }))
    this.#extractors.sort((left, right) => right.priority - left.priority || left.id.localeCompare(right.id))
    return this
  }

  extract(source: ResourceSource, context: ResourceContext = {}) {
    const candidates: RankedCandidate[] = []
    for (const extractor of this.#extractors) {
      if (extractor.supports && !extractor.supports(source, context)) continue
      let result: ResourceCandidate[] = []
      try { result = extractor.extract(source, context) || [] } catch { continue }
      for (const candidate of result) {
        if (candidates.length >= MAX_ITEM_OCCURRENCES) break
        candidates.push({ ...candidate, providerId: candidate.providerId || extractor.id, priority: extractor.priority })
      }
      if (candidates.length >= MAX_ITEM_OCCURRENCES) break
    }
    return arbitrateCandidates(candidates).slice(0, MAX_ITEM_OCCURRENCES)
  }
}

export class SessionResourceIndex {
  resourcesById = new Map<string, IndexedResource>()
  occurrencesById = new Map<string, ResourceOccurrence>()
  occurrenceIdsByResource = new Map<string, string[]>()
  occurrenceIdsBySource = new Map<string, string[]>()
  orderedResourceIds: string[] = []
  truncated = false
  constructor() {
    this.resourcesById = new Map()
    this.occurrencesById = new Map()
    this.occurrenceIdsByResource = new Map()
    this.occurrenceIdsBySource = new Map()
    this.orderedResourceIds = []
    this.truncated = false
  }

  rebuild(model: ResourceModel | null | undefined, context: ResourceContext = {}, registry = createDefaultResourceRegistry()) {
    this.resourcesById.clear()
    this.occurrencesById.clear()
    this.occurrenceIdsByResource.clear()
    this.occurrenceIdsBySource.clear()
    this.orderedResourceIds = []
    this.truncated = false

    let sequence = 0
    const latestTurn: ResourceTurn | null | undefined = Array.isArray(model?.turns) ? model.turns.at(-1) : null
    for (const turn of latestTurn ? [latestTurn] : []) {
      for (const item of turn?.items || []) {
        const source = resourceSourceForItem(turn, item, context)
        for (const candidate of registry.extract(source, context)) {
          const normalized = normalizeResourceCandidate(candidate, source, context)
          if (!normalized) continue
          sequence += 1
          const resource = { ...normalized, firstSeenOrder: sequence, lastSeenOrder: sequence }
          const existing = this.resourcesById.get(resource.id)
          if (existing) {
            existing.lastSeenOrder = sequence
            existing.lastSeenAt = resource.lastSeenAt
            if (resource.priority > existing.priority) {
              existing.providerId = resource.providerId
              existing.priority = resource.priority
              existing.display = resource.display
              existing.raw = resource.raw
            }
          } else {
            this.resourcesById.set(resource.id, resource)
          }

          const occurrence = createOccurrence(resource.id, candidate, source, sequence)
          if (this.occurrencesById.has(occurrence.id)) continue
          this.occurrencesById.set(occurrence.id, occurrence)
          appendMapList(this.occurrenceIdsByResource, resource.id, occurrence.id)
          appendMapList(this.occurrenceIdsBySource, sourceKey(source), occurrence.id)
        }
      }
    }
    this.orderedResourceIds = [...this.resourcesById.values()]
      .sort((left, right) => right.lastSeenOrder - left.lastSeenOrder || left.display.localeCompare(right.display))
      .map((resource) => resource.id)
    return this
  }

  resources({ query = '', kind = 'all' } = {}) {
    const needle = String(query || '').trim().toLocaleLowerCase()
    return this.orderedResourceIds.flatMap((id) => {
      const resource = this.resourcesById.get(id)
      if (!resource) return []
      if (kind !== 'all' && resourceGroup(resource.kind) !== kind) return []
      const haystack = [resource.display, resource.raw, resource.canonical, resource.providerId, resource.target?.url, resource.target?.path]
        .filter(Boolean).join('\n').toLocaleLowerCase()
      return !needle || haystack.includes(needle) ? [resource] : []
    })
  }

  occurrences(resourceId: unknown) {
    return (this.occurrenceIdsByResource.get(String(resourceId || '')) || [])
      .map((id) => this.occurrencesById.get(id)).filter(Boolean)
  }

  counts() {
    const groups = { all: this.resourcesById.size, web: 0, file: 0, code: 0 }
    for (const resource of this.resourcesById.values()) groups[resourceGroup(resource.kind)] += 1
    return { ...groups, occurrences: this.occurrencesById.size }
  }
}

export function createDefaultResourceRegistry() {
  return new ResourceExtractorRegistry()
    .register({
      id: 'markdown-link',
      priority: 400,
      supports: isNarrativeSource,
      extract: (source) => extractMarkdownTargets(source.text),
    })
    .register({
      id: 'inline-code-path',
      priority: 320,
      supports: isNarrativeSource,
      extract: (source) => extractInlineCodePaths(source.text),
    })
    .register({
      id: 'plain-http-url',
      priority: 300,
      supports: isNarrativeSource,
      extract: (source) => extractUrlCandidates(source.text),
    })
    .register({
      id: 'workspace-path',
      priority: 200,
      supports: isNarrativeSource,
      extract: (source) => extractPathCandidates(source.text),
    })
}

export function buildSessionResourceIndex(model: ResourceModel | null | undefined, context: ResourceContext = {}, registry = createDefaultResourceRegistry()) {
  return new SessionResourceIndex().rebuild(model, context, registry)
}

export function sessionResourceRevision(model: ResourceModel | null | undefined) {
  const latestTurn: ResourceTurn | null | undefined = Array.isArray(model?.turns) ? model.turns.at(-1) : null
  if (!latestTurn) return 'empty'
  const items = (latestTurn.items || []).filter((item) => ['userMessage', 'agentMessage', 'plan'].includes(item?.type || ''))
  return [
    String(latestTurn.id || ''),
    objectRevision(latestTurn),
    ...items.map((item) => [
      String(item.id || ''),
      String(item.type || ''),
      objectRevision(item),
      narrativeItemFingerprint(item),
    ].join(':')),
  ].join('|')
}

export function normalizeResourceCandidate(candidate: ResourceCandidate, source: ResourceSource = {}, context: ResourceContext = {}): SessionResource | null {
  const raw = cleanCandidate(candidate?.raw)
  if (!raw) return null
  if (/^https?:\/\//iu.test(raw)) return normalizeWebResource({ ...candidate, raw }, source)
  return normalizeFileResource({ ...candidate, raw }, source, context)
}

export function extractUrlCandidates(value: unknown) {
  const text = boundedText(value)
  const excludedRanges = markdownCodeBlockRanges(text)
  const output = []
  for (const match of text.matchAll(URL_PATTERN)) {
    if (rangeContains(excludedRanges, match.index)) continue
    const raw = trimUrlPunctuation(match[0])
    if (!raw) continue
    output.push({ raw, kind: 'web', field: 'text', start: match.index, end: match.index + raw.length, confidence: 0.98 })
  }
  return output
}

export function extractPathCandidates(value: unknown) {
  const text = boundedText(value)
  const excludedRanges = markdownCodeBlockRanges(text)
  const output = []
  for (const match of text.matchAll(FILE_PATTERN)) {
    if (rangeContains(excludedRanges, match.index) || looksLikeMarkupTag(text, match.index, match[0].length)) continue
    const raw = cleanCandidate(match[0])
    if (!looksLikeFilePath(raw, { requireStrongEvidence: true }) || /^https?:[\\/]/iu.test(raw)) continue
    output.push({ raw, kind: pathKind(raw), field: 'text', start: match.index, end: match.index + match[0].length, confidence: 0.78 })
  }
  return output
}

export function resourceGroup(kind: string) {
  if (['web', 'issue', 'commit'].includes(kind)) return 'web'
  if (kind === 'code') return 'code'
  return 'file'
}

export function resourceIcon(kind: string) {
  if (['web', 'issue', 'commit'].includes(kind)) return 'web'
  if (kind === 'code') return 'code'
  if (kind === 'directory') return 'directory'
  return 'file'
}

function resourceSourceForItem(turn: ResourceTurn, item: ResourceItem, context: ResourceContext): ResourceSource {
  return {
    backend: String(context.backend || 'codex'),
    threadId: String(context.threadId || ''),
    root: String(context.root || ''),
    turnId: String(turn?.id || ''),
    itemId: String(item?.id || ''),
    itemType: String(item?.type || 'unknown'),
    item,
    text: itemText(item),
  }
}

function itemText(item: ResourceItem = {}) {
  if (item.type === 'userMessage') return userContentText(item.content)
  if (item.type === 'agentMessage' || item.type === 'plan') return String(item.text || '')
  return ''
}

function narrativeItemFingerprint(item: ResourceItem = {}) {
  const text = itemText(item)
  const previous = resourceNarrativeRevisions.get(item)
  if (previous?.text === text) return `${text.length}:${previous.revision}`
  const revision = nextResourceNarrativeRevision++
  resourceNarrativeRevisions.set(item, { text, revision })
  return `${text.length}:${revision}`
}

function objectRevision(value: unknown) {
  if (!value || typeof value !== 'object') return 0
  if (!resourceObjectIds.has(value)) resourceObjectIds.set(value, nextResourceObjectId++)
  return resourceObjectIds.get(value)
}

function userContentText(content: unknown) {
  if (typeof content === 'string') return content
  if (!Array.isArray(content)) return valueText(content)
  return content.map((part) => typeof part === 'string' ? part : part?.text || part?.content || '').filter(Boolean).join('\n')
}

function valueText(value: unknown) {
  if (typeof value === 'string') return value
  try { return JSON.stringify(value) } catch { return '' }
}

function extractMarkdownTargets(value: unknown) {
  const text = boundedText(value)
  const excludedRanges = markdownCodeBlockRanges(text)
  const output = []
  for (const match of text.matchAll(MARKDOWN_TARGET_PATTERN)) {
    if (rangeContains(excludedRanges, match.index)) continue
    const raw = cleanCandidate(match[1]!)
    if (!raw) continue
    const offset = match.index + match[0].indexOf(match[1]!)
    output.push({ raw, kind: /^https?:\/\//iu.test(raw) ? 'web' : pathKind(raw), field: 'text', start: offset, end: offset + match[1]!.length, confidence: 1 })
  }
  return output
}

function extractInlineCodePaths(value: unknown) {
  const text = boundedText(value)
  const excludedRanges = markdownCodeBlockRanges(text)
  const output = []
  for (const match of text.matchAll(INLINE_CODE_PATTERN)) {
    if (rangeContains(excludedRanges, match.index)) continue
    const raw = cleanCandidate(match[1]!)
    if (!looksLikeFilePath(raw, { requireStrongEvidence: true })) continue
    const offset = match.index + 1
    output.push({ raw, kind: pathKind(raw), field: 'text', start: offset, end: offset + match[1]!.length, confidence: 0.92 })
  }
  return output
}

function normalizeWebResource(candidate: ResourceCandidate, source: ResourceSource): SessionResource | null {
  let url
  try { url = new URL(candidate.raw) } catch { return null }
  if (!['http:', 'https:'].includes(url.protocol)) return null
  const blocked = Boolean(url.username || url.password)
  url.protocol = url.protocol.toLowerCase()
  url.hostname = url.hostname.toLowerCase()
  if ((url.protocol === 'http:' && url.port === '80') || (url.protocol === 'https:' && url.port === '443')) url.port = ''
  const navigationUrl = url.href
  const identityUrl = new URL(navigationUrl)
  identityUrl.hash = ''
  const canonical = identityUrl.href
  const id = stableId(`web:${canonical}`)
  return {
    schemaVersion: 1,
    id,
    kind: candidate.kind === 'issue' || candidate.kind === 'commit' ? candidate.kind : 'web',
    raw: candidate.raw,
    canonical,
    display: url.hostname || candidate.raw,
    providerId: candidate.providerId || 'plain-http-url',
    priority: Number(candidate.priority || 0),
    confidence: Number(candidate.confidence || 0.9),
    state: blocked ? 'blocked' : 'resolved',
    reason: blocked ? 'The URL contains a username or password' : '',
    target: { url: navigationUrl },
    firstSeenAt: nowFromSource(source),
    lastSeenAt: nowFromSource(source),
  }
}

function normalizeFileResource(candidate: ResourceCandidate, source: ResourceSource, context: ResourceContext): SessionResource | null {
  const parsed = splitFileLocation(candidate.raw)
  if (!parsed.path || !looksLikeFilePath(parsed.path)) return null
  const root = normalizeSlashes(source.root || context.root || '')
  let path = normalizeSlashes(parsed.path).replace(/^\.\//u, '')
  const absolute = path.startsWith('/') || /^[a-z]:\//iu.test(path)
  let sharedAbsolute = false
  let blocked = false
  if (absolute) {
    const caseInsensitive = /^[a-z]:\//iu.test(path) || /^[a-z]:\//iu.test(root)
    const comparablePath = caseInsensitive ? path.toLowerCase() : path
    const comparableRoot = caseInsensitive ? root.toLowerCase() : root
    if (comparableRoot && (comparablePath === comparableRoot || comparablePath.startsWith(`${comparableRoot.replace(/\/$/u, '')}/`))) {
      path = path.slice(root.replace(/\/$/u, '').length).replace(/^\//u, '')
    } else if (candidate.kind !== 'directory' && (context.sharedDocumentDirectories || [])
      .some((directory) => pathInsideDirectory(path, directory))) {
      sharedAbsolute = true
    } else blocked = true
  }
  const segments = path.split('/').filter((part) => part && part !== '.')
  if (segments.some((part) => part === '..')) blocked = true
  path = `${sharedAbsolute && path.startsWith('/') ? '/' : ''}${segments.join('/')}`
  if (!path) return null
  const kind = parsed.line ? 'code' : candidate.kind === 'directory' ? 'directory' : 'file'
  const canonicalRoot = normalizeRoot(root)
  const canonical = `${source.backend || context.backend || 'codex'}:${canonicalRoot}:${path}${parsed.line ? `:${parsed.line}:${parsed.column || 1}` : ''}`
  return {
    schemaVersion: 1,
    id: stableId(`${kind}:${canonical}`),
    kind,
    raw: candidate.raw,
    canonical,
    display: path.split('/').at(-1) || path,
    providerId: candidate.providerId || 'workspace-path',
    priority: Number(candidate.priority || 0),
    confidence: Number(candidate.confidence || 0.75),
    state: blocked ? 'blocked' : 'unresolved',
    reason: blocked ? 'The path is outside the current session root' : '',
    target: {
      backend: String(source.backend || context.backend || 'codex'),
      workspaceRoot: root,
      path,
      ...(parsed.line ? { line: parsed.line, column: parsed.column || 1 } : {}),
    },
    firstSeenAt: nowFromSource(source),
    lastSeenAt: nowFromSource(source),
  }
}

function pathInsideDirectory(path: string, directory: string) {
  const normalizedPath = normalizeSlashes(path).replace(/\/+$/u, '')
  const normalizedDirectory = normalizeSlashes(directory).replace(/\/+$/u, '')
  if (!normalizedPath || !normalizedDirectory) return false
  const caseInsensitive = /^[a-z]:\//iu.test(normalizedPath) || /^[a-z]:\//iu.test(normalizedDirectory)
  const comparablePath = caseInsensitive ? normalizedPath.toLowerCase() : normalizedPath
  const comparableDirectory = caseInsensitive ? normalizedDirectory.toLowerCase() : normalizedDirectory
  return comparablePath === comparableDirectory || comparablePath.startsWith(`${comparableDirectory}/`)
}

function createOccurrence(resourceId: string, candidate: ResourceCandidate, source: ResourceSource, sequence: number): ResourceOccurrence {
  const start = nonNegativeInteger(candidate.start)
  const end = nonNegativeInteger(candidate.end)
  const field = String(candidate.field || 'text')
  const identity = `${sourceKey(source)}:${field}:${start ?? ''}:${end ?? ''}:${resourceId}`
  return {
    id: stableId(`occurrence:${identity}`),
    resourceId,
    threadKey: `${source.backend}:${source.threadId}`,
    turnId: source.turnId,
    itemId: source.itemId,
    itemType: source.itemType,
    field,
    ...(start != null ? { start } : {}),
    ...(end != null ? { end } : {}),
    excerpt: occurrenceExcerpt(source.text, start, end),
    observedOrder: sequence,
    observedAt: nowFromSource(source),
  }
}

function arbitrateCandidates(candidates: RankedCandidate[]) {
  const ordered = [...candidates].sort((left, right) => right.priority - left.priority || Number(left.start || 0) - Number(right.start || 0))
  const kept: RankedCandidate[] = []
  for (const candidate of ordered) {
    const start = nonNegativeInteger(candidate.start)
    const end = nonNegativeInteger(candidate.end)
    const overlap = start != null && end != null && kept.some((existing) => {
      if (String(existing.field || 'text') !== String(candidate.field || 'text')) return false
      const existingStart = nonNegativeInteger(existing.start)
      const existingEnd = nonNegativeInteger(existing.end)
      return existingStart != null && existingEnd != null && start < existingEnd && end > existingStart
    })
    if (!overlap) kept.push(candidate)
  }
  return kept.sort((left, right) => Number(left.start || 0) - Number(right.start || 0) || right.priority - left.priority)
}

function splitFileLocation(value: unknown) {
  const raw = cleanCandidate(value)
  const match = raw.match(/^(.*?)(?::(\d+))(?::(\d+))?$/u)
  if (!match || (/^[a-z]:$/iu.test(match[1]!) && !match[2])) return { path: raw }
  return { path: match[1]!, line: positiveInteger(match[2]), column: positiveInteger(match[3]) }
}

function looksLikeFilePath(value: unknown, { requireStrongEvidence = false } = {}) {
  const text = String(value || '').trim()
  if (!text || /^https?:\/\//iu.test(text)) return false
  const path = splitFileLocation(text).path
  if (!/[\\/]/u.test(path) || /^(?:\/path\/to|path\/to\/|example\/)/iu.test(path)) return false
  if (!requireStrongEvidence) return true
  const explicitPath = /^(?:[a-z]:[\\/]|~?[\\/]|\.{1,2}[\\/])/iu.test(path)
  return explicitPath || FILE_EXTENSION_PATTERN.test(path)
}

function isNarrativeSource(source: ResourceSource) {
  return ['agentMessage', 'plan', 'userMessage'].includes(source.itemType || '')
}

function markdownCodeBlockRanges(value: unknown) {
  const text = String(value || '')
  const ranges = []
  let open: { start: number; marker: string; length: number } | null = null
  let offset = 0
  for (const line of text.split(/(?<=\n)/u)) {
    const content = line.replace(/[\r\n]+$/u, '')
    if (!open) {
      const match = content.match(/^[ \t]{0,3}(`{3,}|~{3,})/u)
      if (match) open = { start: offset, marker: match[1]![0]!, length: match[1]!.length }
    } else {
      const closing = content.match(/^[ \t]{0,3}(`{3,}|~{3,})[ \t]*$/u)
      if (closing && closing[1]![0] === open.marker && closing[1]!.length >= open.length) {
        ranges.push({ start: open.start, end: offset + line.length })
        open = null
      }
    }
    offset += line.length
  }
  if (open) ranges.push({ start: open.start, end: text.length })
  return ranges
}

function rangeContains(ranges: { start: number; end: number }[], offset: number) {
  return ranges.some((range) => offset >= range.start && offset < range.end)
}

function looksLikeMarkupTag(text: string, start: number, length: number) {
  const before = text.slice(Math.max(0, start - 1), start)
  const after = text.slice(start + length, start + length + 1)
  return before === '<' && after === '>'
}

function pathKind(value: unknown) {
  if (/[\\/]$/u.test(String(value || ''))) return 'directory'
  return /:\d+(?::\d+)?[\])},.;!?]*$/u.test(String(value || '')) ? 'code' : 'file'
}

function cleanCandidate(value: unknown) {
  return String(value || '').trim().replace(/^[<({\["']+/u, '').replace(/[>"']+$/u, '').replace(/[.,;!?]+$/u, '')
}

function trimUrlPunctuation(value: unknown) {
  let text = cleanCandidate(value)
  const pairs: [string, string][] = [['(', ')'], ['[', ']'], ['{', '}']]
  for (const [left, right] of pairs) {
    while (text.endsWith(right) && count(text, right) > count(text, left)) text = text.slice(0, -1)
  }
  return text
}

function occurrenceExcerpt(text: unknown, start: number | null, end: number | null) {
  const value = String(text || '').replace(/\s+/gu, ' ').trim()
  if (!value) return ''
  if (start == null || end == null) return value.slice(0, 160)
  const from = Math.max(0, start - 48)
  const to = Math.min(value.length, end + 80)
  return `${from ? '…' : ''}${value.slice(from, to)}${to < value.length ? '…' : ''}`
}

function boundedText(value: unknown) {
  return String(value || '').slice(0, MAX_ITEM_TEXT)
}

function normalizeSlashes(value: unknown) {
  return String(value || '').replaceAll('\\', '/').replace(/\/{2,}/gu, '/').replace(/\/$/u, '')
}

function normalizeRoot(value: unknown) {
  const root = normalizeSlashes(value)
  return /^[a-z]:\//iu.test(root) ? root.toLowerCase() : root
}

function sourceKey(source: ResourceSource) {
  return `${source.backend || ''}:${source.threadId || ''}:${source.turnId || ''}:${source.itemId || ''}`
}

function nowFromSource(source: ResourceSource) {
  return String(source.item?.completedAt || source.item?.createdAt || source.item?.updatedAt || '')
}

function appendMapList(map: Map<string, string[]>, key: string, value: string) {
  const list = map.get(key) || []
  list.push(value)
  map.set(key, list)
}

function count(value: unknown, character: string) {
  return [...String(value || '')].filter((candidate) => candidate === character).length
}

function positiveInteger(value: unknown) {
  const number = Number(value)
  return Number.isInteger(number) && number > 0 ? number : null
}

function nonNegativeInteger(value: unknown) {
  const number = Number(value)
  return Number.isInteger(number) && number >= 0 ? number : null
}

function stableId(value: unknown) {
  let hash = 0x811c9dc5
  for (const character of String(value || '')) {
    hash ^= character.codePointAt(0)!
    hash = Math.imul(hash, 0x01000193)
  }
  return `resource-${(hash >>> 0).toString(16).padStart(8, '0')}`
}
