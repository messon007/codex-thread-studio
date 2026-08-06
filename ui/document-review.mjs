const MAX_QUOTE_LENGTH = 16_000

export function normalizeAnnotationTarget(draft = {}) {
  const target = draft.target
  if (target?.kind === 'fileRange' && target.filePath) {
    return {
      kind: 'fileRange',
      filePath: String(target.filePath).slice(0, 4096),
      root: String(target.root || '').slice(0, 4096),
      baseHash: String(target.baseHash || '').slice(0, 128),
      startOffset: finiteOffset(target.startOffset),
      endOffset: finiteOffset(target.endOffset),
      prefix: String(target.prefix || '').slice(0, 256),
      suffix: String(target.suffix || '').slice(0, 256),
    }
  }
  return {
    kind: 'chatRange',
    itemId: draft.itemId ? String(draft.itemId).slice(0, 256) : null,
    turnId: draft.turnId ? String(draft.turnId).slice(0, 256) : null,
  }
}

export function snapshotAnnotationSelection(selection) {
  const quote = String(selection?.quote || '').trim().slice(0, MAX_QUOTE_LENGTH)
  if (!quote) return null
  return {
    quote,
    itemId: selection?.itemId ? String(selection.itemId).slice(0, 256) : null,
    turnId: selection?.turnId ? String(selection.turnId).slice(0, 256) : null,
    target: normalizeAnnotationTarget(selection),
  }
}

export function locateQuote(content, quote, fromOffset = 0) {
  const source = String(content || '')
  const selected = String(quote || '').slice(0, MAX_QUOTE_LENGTH)
  if (!selected) return { startOffset: null, endOffset: null, ambiguous: false }
  const first = source.indexOf(selected, Math.max(0, finiteOffset(fromOffset) || 0))
  if (first < 0) return { startOffset: null, endOffset: null, ambiguous: false }
  return {
    startOffset: first,
    endOffset: first + selected.length,
    ambiguous: source.indexOf(selected, first + selected.length) >= 0,
  }
}

export function createFileRangeTarget(file, quote, hintOffset = 0) {
  const selected = String(quote || '').slice(0, MAX_QUOTE_LENGTH)
  const located = locateQuote(file?.content, selected, hintOffset)
  const start = located.startOffset
  const end = located.endOffset
  const source = String(file?.content || '')
  return {
    kind: 'fileRange',
    filePath: String(file?.path || ''),
    root: String(file?.root || ''),
    baseHash: String(file?.hash || ''),
    startOffset: start,
    endOffset: end,
    prefix: start == null ? '' : source.slice(Math.max(0, start - 96), start),
    suffix: end == null ? '' : source.slice(end, end + 96),
    ambiguous: located.ambiguous,
  }
}

export function lineNumberAt(content, offset) {
  if (!Number.isFinite(offset) || offset < 0) return null
  return String(content || '').slice(0, offset).split('\n').length
}

export function fileDisplayName(path) {
  const normalized = String(path || '').replaceAll('\\', '/')
  return normalized.split('/').filter(Boolean).pop() || normalized || 'Untitled'
}

export function isMarkdownFile(path) {
  return /\.(md|mdown|markdown|mkd)$/i.test(String(path || ''))
}

function finiteOffset(value) {
  const number = Number(value)
  return Number.isInteger(number) && number >= 0 ? number : null
}
