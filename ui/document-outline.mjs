import { marked } from './vendor/marked.esm.js'

const MAX_OUTLINE_ITEMS = 2_000
const MAX_LABEL_LENGTH = 512

export function extractMarkdownOutline(source) {
  const value = String(source || '')
  const tokens = marked.lexer(value)
  const items = []
  const occurrences = new Map()
  let cursor = 0
  for (const token of tokens) {
    const raw = String(token.raw || '')
    const offset = raw ? value.indexOf(raw, cursor) : cursor
    if (offset >= 0) cursor = offset + raw.length
    if (token?.type !== 'heading') continue
    const label = boundedLabel(inlineTokenText(token.tokens) || token.text)
    const baseId = outlineSlug(label) || `section-${items.length + 1}`
    const occurrence = (occurrences.get(baseId) || 0) + 1
    occurrences.set(baseId, occurrence)
    items.push({
      id: occurrence === 1 ? baseId : `${baseId}-${occurrence}`,
      label: label || 'Untitled section',
      depth: clampDepth(Number(token.depth) - 1),
      target: {
        kind: 'text-heading',
        offset: Math.max(0, offset),
        line: lineNumberAt(value, Math.max(0, offset)),
      },
    })
    if (items.length >= MAX_OUTLINE_ITEMS) break
  }
  return withHierarchy(items)
}

export function extractHtmlOutline(source) {
  const value = String(source || '')
  const items = []
  const occurrences = new Map()
  const blockedRanges = [...value.matchAll(/<!--[\s\S]*?-->|<(script|style|template|noscript)\b[^>]*>[\s\S]*?<\/\1\s*>/giu)]
    .map((match) => [match.index, match.index + match[0].length])
  const headingPattern = /<h([1-6])\b([^>]*)>([\s\S]*?)<\/h\1\s*>/giu
  let match
  while ((match = headingPattern.exec(value)) && items.length < MAX_OUTLINE_ITEMS) {
    if (blockedRanges.some(([start, end]) => match.index >= start && match.index < end)) continue
    const label = boundedLabel(decodeHtml(stripMarkup(match[3]))) || 'Untitled section'
    const idMatch = /\bid\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/iu.exec(match[2])
    const requestedId = boundedLabel(idMatch?.[1] || idMatch?.[2] || idMatch?.[3] || '')
    const baseId = outlineSlug(requestedId || label) || `section-${items.length + 1}`
    const occurrence = (occurrences.get(baseId) || 0) + 1
    occurrences.set(baseId, occurrence)
    items.push({
      id: occurrence === 1 ? baseId : `${baseId}-${occurrence}`,
      label,
      depth: clampDepth(Number(match[1]) - 1),
      target: { kind: 'text-heading', offset: match.index, line: lineNumberAt(value, match.index) },
    })
  }
  return withHierarchy(items)
}

export function normalizeDocumentOutline(items = []) {
  return withHierarchy((Array.isArray(items) ? items : []).slice(0, MAX_OUTLINE_ITEMS).map((item, index) => ({
    id: boundedLabel(item?.id) || `section-${index + 1}`,
    label: boundedLabel(item?.label) || 'Untitled section',
    depth: clampDepth(item?.depth),
    target: item?.target ?? null,
  })))
}

export function filterDocumentOutline(items, query) {
  const list = Array.isArray(items) ? items : []
  const needle = String(query || '').trim().toLocaleLowerCase()
  if (!needle) return list.map((item) => ({ ...item, contextOnly: false }))
  const matchingIds = new Set(list.filter((item) => item.label.toLocaleLowerCase().includes(needle)).map((item) => item.id))
  const visibleIds = new Set(matchingIds)
  for (const item of list) {
    if (!matchingIds.has(item.id)) continue
    let parentId = item.parentId
    while (parentId) {
      visibleIds.add(parentId)
      parentId = list.find((candidate) => candidate.id === parentId)?.parentId || ''
    }
  }
  return list.filter((item) => visibleIds.has(item.id)).map((item) => ({ ...item, contextOnly: !matchingIds.has(item.id) }))
}

export function outlineItemForLocation(items, location) {
  const list = Array.isArray(items) ? items : []
  if (!list.length || location == null) return null
  if (typeof location === 'string') {
    const clean = stripFragment(location)
    return list.find((item) => stripFragment(item.target?.href) === clean) || null
  }
  const page = Number(location?.page)
  if (Number.isFinite(page)) {
    return [...list].reverse().find((item) => Number(item.target?.page) <= page) || null
  }
  return null
}

export async function extractPdfOutline(document) {
  const source = await document.getOutline().catch(() => null)
  if (!Array.isArray(source) || !source.length) return []
  const result = []
  async function append(items, depth) {
    for (const item of items || []) {
      if (result.length >= MAX_OUTLINE_ITEMS) return
      let destination = item?.dest
      if (typeof destination === 'string') destination = await document.getDestination(destination).catch(() => null)
      let page = null
      const reference = Array.isArray(destination) ? destination[0] : null
      if (Number.isInteger(reference)) page = reference + 1
      else if (reference) page = await document.getPageIndex(reference).then((index) => index + 1).catch(() => null)
      if (page != null) {
        result.push({
          id: `pdf-${result.length + 1}`,
          label: boundedLabel(item?.title) || 'Untitled section',
          depth: clampDepth(depth),
          target: { kind: 'pdf', page, destination },
        })
      }
      await append(item?.items, depth + 1)
    }
  }
  await append(source, 0)
  return result
}

function withHierarchy(items) {
  const ancestors = []
  return items.map((item) => {
    const depth = clampDepth(item.depth)
    while (ancestors.length && ancestors.at(-1).depth >= depth) ancestors.pop()
    const parentId = ancestors.at(-1)?.id || ''
    ancestors.push({ depth, id: item.id })
    return { ...item, depth, parentId }
  })
}

function inlineTokenText(tokens) {
  return (Array.isArray(tokens) ? tokens : []).map((token) => {
    if (Array.isArray(token?.tokens)) return inlineTokenText(token.tokens)
    if (token?.type === 'image') return token.text || ''
    return token?.text || token?.raw || ''
  }).join('')
}

function outlineSlug(value) {
  return String(value || '').normalize('NFKC').toLocaleLowerCase()
    .replace(/<[^>]*>/gu, '')
    .replace(/[^\p{Letter}\p{Number}]+/gu, '-')
    .replace(/^-+|-+$/gu, '')
    .slice(0, 96)
}

function stripMarkup(value) {
  return String(value || '').replace(/<[^>]*>/gu, ' ').replace(/\s+/gu, ' ').trim()
}

function decodeHtml(value) {
  const entities = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' }
  return String(value || '').replace(/&(#x[\da-f]+|#\d+|[a-z]+);/giu, (match, entity) => {
    if (entity[0] === '#') {
      const hexadecimal = entity[1]?.toLowerCase() === 'x'
      const number = Number.parseInt(entity.slice(hexadecimal ? 2 : 1), hexadecimal ? 16 : 10)
      return Number.isFinite(number) ? String.fromCodePoint(number) : match
    }
    return entities[entity.toLowerCase()] ?? match
  })
}

function lineNumberAt(value, offset) {
  return String(value || '').slice(0, Math.max(0, offset)).split('\n').length
}

function boundedLabel(value) {
  return String(value || '').replace(/\s+/gu, ' ').trim().slice(0, MAX_LABEL_LENGTH)
}

function clampDepth(value) {
  return Math.min(12, Math.max(0, Number(value) || 0))
}

function stripFragment(value) {
  return String(value || '').split('#')[0].replace(/^\.\//u, '')
}
