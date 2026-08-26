const MAX_QUOTE_LENGTH = 16_000

export const STATIC_HTML_FORBIDDEN_TAGS = Object.freeze([
  'script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'textarea', 'select',
  'option', 'link', 'meta', 'base', 'img', 'video', 'audio', 'source', 'canvas',
])

export const STATIC_HTML_FORBIDDEN_ATTRIBUTES = Object.freeze([
  'style', 'src', 'srcset', 'href', 'xlink:href', 'formaction', 'action', 'target', 'download', 'srcdoc',
])

export function artifactSearchAvailable(file, view) {
  return Boolean(
    file
    && !file.loading
    && !file.error
    && typeof file.content === 'string'
    && (view === 'preview' || view === 'source' || view === 'edit'),
  )
}

export function artifactInlineSearchAvailable(file, view) {
  return artifactSearchAvailable(file, view) && view !== 'edit'
}

export function resolveMarkdownImagePath(file, source) {
  const raw = String(source || '').trim()
  if (!raw || raw.startsWith('#') || raw.startsWith('//')) return null
  if (/^(?:data|blob):/iu.test(raw)) return { embedded: raw }
  if (/^[a-z][a-z\d+.-]*:/iu.test(raw)) return null

  let decoded
  try {
    decoded = decodeURIComponent(raw.split(/[?#]/u, 1)[0])
  } catch {
    return null
  }
  const reference = decoded.replaceAll('\\', '/')
  if (!reference || reference.includes('\0')) return null

  const root = String(file?.root || '').replaceAll('\\', '/').replace(/\/+$/u, '')
  const absoluteDocument = String(file?.path || '').replaceAll('\\', '/')
  const relativeDocument = String(file?.relativePath || '').replaceAll('\\', '/')
  const rootPrefix = root ? `${root}/` : ''
  let candidate

  if (rootPrefix && absoluteDocument.startsWith(rootPrefix)) {
    const documentPath = absoluteDocument.slice(rootPrefix.length)
    candidate = reference.startsWith('/')
      ? reference.slice(1)
      : `${documentPath.slice(0, Math.max(0, documentPath.lastIndexOf('/') + 1))}${reference}`
  } else if (relativeDocument) {
    candidate = reference.startsWith('/')
      ? reference.slice(1)
      : `${relativeDocument.slice(0, Math.max(0, relativeDocument.lastIndexOf('/') + 1))}${reference}`
  } else {
    return null
  }

  const segments = []
  for (const segment of candidate.split('/')) {
    if (!segment || segment === '.') continue
    if (segment === '..') {
      if (!segments.length) return null
      segments.pop()
    } else {
      segments.push(segment)
    }
  }
  return segments.length ? { path: segments.join('/') } : null
}

export function resolveMarkdownFileLink(value) {
  let raw = String(value || '').trim()
  if (!raw || raw.startsWith('#') || /^https?:\/\//iu.test(raw)) return null

  const fragmentLocation = raw.match(/#L(\d+)(?:C(\d+))?$/iu)
  if (fragmentLocation) raw = raw.slice(0, fragmentLocation.index)
  else raw = raw.replace(/[?#].*$/u, '')

  try {
    raw = decodeURIComponent(raw)
  } catch {
    return null
  }
  raw = raw.replace(/^\.\//u, '')
  if (!raw || raw.includes('\0')) return null

  const location = raw.match(/^(.*?)(?::(\d+))(?::(\d+))?$/u)
  const path = location?.[1] || raw
  const unsupportedScheme = /^[a-z][a-z\d+.-]*:/iu.test(path) && !/^[a-z]:[\\/]/iu.test(path)
  if (!path || unsupportedScheme) return null
  return {
    path,
    line: Number(location?.[2] || fragmentLocation?.[1] || 0) || undefined,
    column: Number(location?.[3] || fragmentLocation?.[2] || 0) || undefined,
  }
}

export function normalizeAnnotationTarget(draft = {}) {
  const target = draft.target
  if (target?.kind === 'fileRange' && target.filePath) {
    const startOffset = finiteOffset(target.startOffset)
    const endOffset = finiteOffset(target.endOffset)
    const validRange = startOffset != null && endOffset != null && endOffset > startOffset
    const startLine = finiteLine(target.startLine)
    const endLine = finiteLine(target.endLine)
    const validLines = startLine != null && endLine != null && endLine >= startLine
    return {
      kind: 'fileRange',
      filePath: String(target.filePath).slice(0, 4096),
      root: String(target.root || '').slice(0, 4096),
      baseHash: String(target.baseHash || '').slice(0, 128),
      startOffset: validRange ? startOffset : null,
      endOffset: validRange ? endOffset : null,
      startLine: validLines ? startLine : null,
      endLine: validLines ? endLine : null,
      prefix: validRange ? String(target.prefix || '').slice(0, 256) : '',
      suffix: validRange ? String(target.suffix || '').slice(0, 256) : '',
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

export function findTextMatchRanges(text, query) {
  const source = String(text || '')
  const needle = String(query || '')
  if (!source || !needle) return []
  const loweredSource = source.toLowerCase()
  const loweredNeedle = needle.toLowerCase()
  const matches = []
  for (let index = 0; index < source.length;) {
    const start = loweredSource.indexOf(loweredNeedle, index)
    if (start < 0) break
    const end = start + needle.length
    matches.push({ start, end })
    index = end
  }
  return matches
}

export function createFileRangeTarget(file, quote, hintOffset = 0) {
  const selected = String(quote || '').slice(0, MAX_QUOTE_LENGTH)
  const located = locateQuote(file?.content, selected, hintOffset)
  const start = located.startOffset
  const end = located.endOffset
  const source = String(file?.content || '')
  if (start == null || end == null || end <= start) {
    return {
      kind: 'fileRange',
      filePath: String(file?.path || ''),
      root: String(file?.root || ''),
      baseHash: String(file?.hash || ''),
      startOffset: null,
      endOffset: null,
      startLine: null,
      endLine: null,
      prefix: '',
      suffix: '',
      ambiguous: located.ambiguous,
    }
  }
  return {
    kind: 'fileRange',
    filePath: String(file?.path || ''),
    root: String(file?.root || ''),
    baseHash: String(file?.hash || ''),
    startOffset: start,
    endOffset: end,
    ...lineRangeForOffsets(source, start, end),
    prefix: start == null ? '' : source.slice(Math.max(0, start - 96), start),
    suffix: end == null ? '' : source.slice(end, end + 96),
    ambiguous: located.ambiguous,
  }
}

export function lineNumberAt(content, offset) {
  if (!Number.isFinite(offset) || offset < 0) return null
  return String(content || '').slice(0, offset).split('\n').length
}

export function lineRangeForOffsets(content, startOffset, endOffset) {
  const start = finiteOffset(startOffset)
  const end = finiteOffset(endOffset)
  if (start == null || end == null || end <= start) return { startLine: null, endLine: null }
  return {
    startLine: lineNumberAt(content, start),
    endLine: lineNumberAt(content, end - 1),
  }
}

export function formatLineAnchor(startLine, endLine) {
  const start = finiteLine(startLine)
  const end = finiteLine(endLine)
  if (start == null || end == null || end < start) return ''
  return start === end ? `line ${start}` : `lines ${start}-${end}`
}

export function lineRangeForTarget(target, content = null) {
  const startLine = finiteLine(target?.startLine)
  const endLine = finiteLine(target?.endLine)
  if (startLine != null && endLine != null && endLine >= startLine) return { startLine, endLine }
  if (content == null) return { startLine: null, endLine: null }
  return lineRangeForOffsets(content, target?.startOffset, target?.endOffset)
}

export function fileAnnotationAnchor(target, content = null) {
  if (target?.kind !== 'fileRange' || !target.filePath) return ''
  const lines = lineRangeForTarget(target, content)
  const lineAnchor = formatLineAnchor(lines.startLine, lines.endLine)
  const startOffset = finiteOffset(target.startOffset)
  const endOffset = finiteOffset(target.endOffset)
  const offsetAnchor = startOffset != null && endOffset != null && endOffset > startOffset
    ? `offset ${target.startOffset}-${target.endOffset}`
    : ''
  return [target.filePath, lineAnchor || offsetAnchor, target.baseHash && `base ${target.baseHash}`]
    .filter(Boolean)
    .join(' / ')
}

export function fileDisplayName(path) {
  const normalized = String(path || '').replaceAll('\\', '/')
  return normalized.split('/').filter(Boolean).pop() || normalized || 'Untitled'
}

export function isMarkdownFile(path) {
  return /\.(md|mdown|markdown|mkd)$/i.test(String(path || ''))
}

export function isHtmlFile(path) {
  return /\.(html?|xhtml)$/i.test(String(path || ''))
}

function finiteOffset(value) {
  if (value == null || typeof value === 'boolean') return null
  if (typeof value === 'string' && !value.trim()) return null
  const number = Number(value)
  return Number.isInteger(number) && number >= 0 ? number : null
}

function finiteLine(value) {
  const line = finiteOffset(value)
  return line != null && line >= 1 ? line : null
}
