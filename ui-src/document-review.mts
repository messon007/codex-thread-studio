import type { FileRangeTarget } from './comment-types.mjs'
type RecordValue = Record<string, unknown>
const record = (value: unknown): RecordValue => value !== null && typeof value === 'object' ? value as RecordValue : {}
interface PreviewFormat { language: 'json' | 'yaml' | 'xml' | 'toml' | 'config'; label: string }
interface PreviewToken { kind: string; start: number; end: number; value: string; name?: string }
interface ArtifactFile { loading?: boolean; error?: unknown; content?: string; root?: string; documentRoot?: string; path?: string; relativePath?: string; hash?: string }
const MAX_QUOTE_LENGTH = 16_000

export const STATIC_HTML_FORBIDDEN_TAGS = Object.freeze([
  'script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'textarea', 'select',
  'option', 'link', 'meta', 'base', 'img', 'video', 'audio', 'source', 'canvas',
])

export const STATIC_HTML_FORBIDDEN_ATTRIBUTES = Object.freeze([
  'style', 'src', 'srcset', 'href', 'xlink:href', 'formaction', 'action', 'target', 'download', 'srcdoc',
])

const MAX_STRUCTURED_HIGHLIGHT_LENGTH = 1024 * 1024
const MAX_STRUCTURED_FORMAT_LENGTH = 256 * 1024
const MAX_STRUCTURED_FORMATTED_LENGTH = 1024 * 1024
const STRUCTURED_INDENT = '  '

const STRUCTURED_TEXT_EXTENSIONS: Readonly<Record<string, PreviewFormat>> = Object.freeze({
  json: Object.freeze({ language: 'json', label: 'JSON' }),
  json5: Object.freeze({ language: 'json', label: 'JSON5' }),
  jsonc: Object.freeze({ language: 'json', label: 'JSONC' }),
  jsonl: Object.freeze({ language: 'json', label: 'JSONL' }),
  ndjson: Object.freeze({ language: 'json', label: 'NDJSON' }),
  yaml: Object.freeze({ language: 'yaml', label: 'YAML' }),
  yml: Object.freeze({ language: 'yaml', label: 'YAML' }),
  xml: Object.freeze({ language: 'xml', label: 'XML' }),
  plist: Object.freeze({ language: 'xml', label: 'PLIST' }),
  rss: Object.freeze({ language: 'xml', label: 'RSS' }),
  atom: Object.freeze({ language: 'xml', label: 'ATOM' }),
  toml: Object.freeze({ language: 'toml', label: 'TOML' }),
  ini: Object.freeze({ language: 'config', label: 'INI' }),
  cfg: Object.freeze({ language: 'config', label: 'CFG' }),
  conf: Object.freeze({ language: 'config', label: 'CONF' }),
  properties: Object.freeze({ language: 'config', label: 'PROPERTIES' }),
})

const STRUCTURED_TEXT_NAMES: Readonly<Record<string, PreviewFormat>> = Object.freeze({
  '.editorconfig': Object.freeze({ language: 'config', label: 'EDITORCONFIG' }),
  '.gitconfig': Object.freeze({ language: 'config', label: 'GITCONFIG' }),
  '.npmrc': Object.freeze({ language: 'config', label: 'NPMRC' }),
  '.pnpmrc': Object.freeze({ language: 'config', label: 'PNPMRC' }),
  '.yarnrc': Object.freeze({ language: 'config', label: 'YARNRC' }),
  'cargo.lock': Object.freeze({ language: 'toml', label: 'TOML' }),
  'pdm.lock': Object.freeze({ language: 'toml', label: 'TOML' }),
  'pipfile': Object.freeze({ language: 'toml', label: 'TOML' }),
  'poetry.lock': Object.freeze({ language: 'toml', label: 'TOML' }),
})

const JSON_TOKEN_PATTERN = /\/\/[^\r\n]*|\/\*[\s\S]*?\*\/|"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|\b(?:true|false|null|Infinity|NaN|undefined)\b|[A-Za-z_$][\w$-]*(?=\s*:)|[{}\[\],:]/gu
const YAML_TOKEN_PATTERN = /^[\t ]*(?:-[\t ]*)?[A-Za-z0-9_.-]+(?=[\t ]*:)|^(?:---|\.\.\.)(?=[\t ]*(?:#|$))|"(?:\\[\s\S]|[^"\\])*"|'(?:''|[^'])*'|#[^\r\n]*|[&*!][\w.-]+|\b(?:true|false|null|yes|no|on|off|~)\b|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|[{}\[\],:|>]/gimu
const XML_TOKEN_PATTERN = /<!--[\s\S]*?(?:-->|$)|<!\[CDATA\[[\s\S]*?(?:\]\]>|$)|<\?[\s\S]*?(?:\?>|$)|<!DOCTYPE(?:\s+(?:[^>"']|"[^"]*"|'[^']*')*)*>|<\/?[A-Za-z_][\w:.-]*(?:\s+(?:[^"'<>]|"[^"]*"|'[^']*')*)?\/?>|&(?:#\d+|#x[\da-f]+|[\w.-]+);/giu
const TOML_TOKEN_PATTERN = /^[\t ]*\[\[?[^\]\r\n]+\]\]?[\t ]*(?=#|$)|^[\t ]*(?:"(?:\\[\s\S]|[^"\\])*"|'[^']*'|[A-Za-z0-9_-]+(?:[\t ]*\.[\t ]*[A-Za-z0-9_-]+)*)(?=[\t ]*=)|"""[\s\S]*?(?:"""|$)|'''[\s\S]*?(?:'''|$)|"(?:\\[\s\S]|[^"\\])*"|'[^']*'|#[^\r\n]*|\b(?:true|false)\b|\b\d{4}-\d{2}-\d{2}(?:[Tt ][\d:.+-]+[Zz]?)?\b|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|[={}\[\],.]/gimu
const CONFIG_TOKEN_PATTERN = /^[\t ]*[;#][^\r\n]*|^[\t ]*\[[^\]\r\n]+\][\t ]*$|^[\t ]*(?:export[\t ]+)?(?:[A-Za-z_][\w.-]*|[^=:#;\r\n][^=:\r\n]*?)(?=[\t ]*(?:=|:))|"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'|\b(?:true|false|yes|no|on|off|null|none)\b|-?(?:0|[1-9]\d*)(?:\.\d+)?|[=:]/gimu

export function artifactSearchAvailable(file: ArtifactFile | null | undefined, view: string) {
  return Boolean(
    file
    && !file.loading
    && !file.error
    && typeof file.content === 'string'
    && (view === 'preview' || view === 'source' || view === 'edit'),
  )
}

export function artifactInlineSearchAvailable(file: ArtifactFile | null | undefined, view: string) {
  return artifactSearchAvailable(file, view) && view !== 'edit'
}

export function resolveMarkdownImagePath(file: ArtifactFile | null | undefined, source: unknown) {
  const raw = String(source || '').trim()
  if (!raw || raw.startsWith('#') || raw.startsWith('//')) return null
  if (/^(?:data|blob):/iu.test(raw)) return { embedded: raw }
  if (/^[a-z][a-z\d+.-]*:/iu.test(raw)) return null

  let decoded
  try {
    decoded = decodeURIComponent(raw.split(/[?#]/u, 1)[0]!)
  } catch {
    return null
  }
  const reference = decoded.replaceAll('\\', '/')
  if (!reference || reference.includes('\0')) return null

  const root = String(file?.root || '').replaceAll('\\', '/').replace(/\/+$/u, '')
  const documentRoot = String(file?.documentRoot || file?.root || '').replaceAll('\\', '/').replace(/\/+$/u, '')
  const absoluteDocument = String(file?.path || '').replaceAll('\\', '/')
  const relativeDocument = String(file?.relativePath || '').replaceAll('\\', '/')
  const rootPrefix = documentRoot ? `${documentRoot}/` : ''
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
  if (!segments.length) return null
  const path = segments.join('/')
  return { path: documentRoot && documentRoot !== root ? `${documentRoot}/${path}` : path }
}

export function resolveMarkdownFileLink(value: unknown) {
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

export function normalizeAnnotationTarget(value: unknown = {}): FileRangeTarget | { kind: 'chatRange'; itemId: string | null; turnId: string | null } {
  const draft = record(value)
  const target = record(draft.target)
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

export function snapshotAnnotationSelection(value: unknown) {
  const selection = record(value)
  const quote = String(selection?.quote || '').trim().slice(0, MAX_QUOTE_LENGTH)
  if (!quote) return null
  return {
    quote,
    itemId: selection?.itemId ? String(selection.itemId).slice(0, 256) : null,
    turnId: selection?.turnId ? String(selection.turnId).slice(0, 256) : null,
    target: normalizeAnnotationTarget(selection),
  }
}

export function locateQuote(content: unknown, quote: unknown, fromOffset = 0) {
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

export function findTextMatchRanges(text: unknown, query: unknown) {
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

export function createFileRangeTarget(file: ArtifactFile | null | undefined, quote: unknown, hintOffset = 0) {
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

export function lineNumberAt(content: unknown, offset: number) {
  if (!Number.isFinite(offset) || offset < 0) return null
  return String(content || '').slice(0, offset).split('\n').length
}

export function lineRangeForOffsets(content: unknown, startOffset: unknown, endOffset: unknown) {
  const start = finiteOffset(startOffset)
  const end = finiteOffset(endOffset)
  if (start == null || end == null || end <= start) return { startLine: null, endLine: null }
  return {
    startLine: lineNumberAt(content, start),
    endLine: lineNumberAt(content, end - 1),
  }
}

export function formatLineAnchor(startLine: unknown, endLine: unknown) {
  const start = finiteLine(startLine)
  const end = finiteLine(endLine)
  if (start == null || end == null || end < start) return ''
  return start === end ? `line ${start}` : `lines ${start}-${end}`
}

export function lineRangeForTarget(value: unknown, content: string | null = null) {
  const target = record(value)
  const startLine = finiteLine(target?.startLine)
  const endLine = finiteLine(target?.endLine)
  if (startLine != null && endLine != null && endLine >= startLine) return { startLine, endLine }
  if (content == null) return { startLine: null, endLine: null }
  return lineRangeForOffsets(content, target?.startOffset, target?.endOffset)
}

export function fileAnnotationAnchor(value: unknown, content: string | null = null) {
  const target = record(value)
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

export function fileDisplayName(path: unknown) {
  const normalized = String(path || '').replaceAll('\\', '/')
  return normalized.split('/').filter(Boolean).pop() || normalized || 'Untitled'
}

export function isMarkdownFile(path: unknown) {
  return /\.(md|mdown|markdown|mkd)$/i.test(String(path || ''))
}

export function isHtmlFile(path: unknown) {
  return /\.(html?|xhtml)$/i.test(String(path || ''))
}

export function structuredTextPreviewKind(path: unknown): PreviewFormat | null {
  const normalized = String(path || '').replaceAll('\\', '/')
  const name = normalized.split('/').filter(Boolean).pop()?.toLowerCase() || ''
  if (!name) return null
  if (name === '.env' || name.startsWith('.env.')) return { language: 'config', label: 'ENV' }
  const named = STRUCTURED_TEXT_NAMES[name]
  if (named) return { ...named }
  const extension = name.includes('.') ? name.split('.').pop()! : ''
  const matched = STRUCTURED_TEXT_EXTENSIONS[extension]
  return matched ? { ...matched } : null
}

export function renderStructuredTextPreview(path: unknown, value: unknown) {
  const format = structuredTextPreviewKind(path)
  if (!format) return null
  const source = String(value ?? '')
  const formatted = source.length <= MAX_STRUCTURED_FORMAT_LENGTH
    ? formatStructuredPreviewSource(format, source)
    : null
  const text = formatted?.text ?? source
  const sourceMap = formatted?.sourceMap ?? null
  if (text.length > MAX_STRUCTURED_HIGHLIGHT_LENGTH) {
    return {
      ...format,
      formatted: Boolean(formatted),
      highlighted: false,
      html: escapePreviewHtml(text),
      sourceMap,
      text,
    }
  }
  const renderer = {
    config: renderConfigTokens,
    json: renderJsonTokens,
    toml: renderTomlTokens,
    xml: renderXmlTokens,
    yaml: renderYamlTokens,
  }[format.language]
  return {
    ...format,
    formatted: Boolean(formatted),
    highlighted: true,
    html: renderer(text),
    sourceMap,
    text,
  }
}

export function structuredPreviewSourceRange(sourceMap: unknown, startOffset: unknown, endOffset: unknown) {
  const start = finiteOffset(startOffset)
  const end = finiteOffset(endOffset)
  if (start == null || end == null || end <= start) return null
  if (!(sourceMap instanceof Int32Array)) return { startOffset: start, endOffset: end }
  const upper = Math.min(end, sourceMap.length)
  let sourceStart = null
  let sourceEnd = null
  for (let index = Math.min(start, upper); index < upper; index += 1) {
    const sourceIndex = sourceMap[index]!
    if (sourceIndex < 0) continue
    if (sourceStart == null) sourceStart = sourceIndex
    sourceEnd = sourceIndex + 1
  }
  return sourceStart == null || sourceEnd == null
    ? null
    : { startOffset: sourceStart, endOffset: sourceEnd }
}

function formatStructuredPreviewSource(format: PreviewFormat, source: string) {
  if (!source.trim()) return null
  if (format.language === 'json') return formatJsonPreviewSource(source)
  if (format.language === 'xml') return formatXmlPreviewSource(source)
  if (format.language === 'toml') return formatAssignmentPreviewSource(source, '=')
  if (format.language === 'config') return formatAssignmentPreviewSource(source, '=:')
  // YAML indentation is semantic. Without a full round-trip parser, preserving it is safer
  // than presenting a visually tidy document whose hierarchy may be incorrect.
  return null
}

function createMappedPreviewBuilder(source: string) {
  const parts: string[] = []
  const sourceOffsets: number[] = []
  let length = 0
  let overflow = false
  return {
    appendSource(start: number, end: number) {
      if (overflow || end <= start) return
      const value = source.slice(start, end)
      if (length + value.length > MAX_STRUCTURED_FORMATTED_LENGTH) {
        overflow = true
        return
      }
      parts.push(value)
      for (let index = start; index < end; index += 1) sourceOffsets.push(index)
      length += value.length
    },
    appendText(value: string) {
      if (overflow || !value) return
      if (length + value.length > MAX_STRUCTURED_FORMATTED_LENGTH) {
        overflow = true
        return
      }
      parts.push(value)
      for (let index = 0; index < value.length; index += 1) sourceOffsets.push(-1)
      length += value.length
    },
    finish() {
      if (overflow) return null
      const text = parts.join('')
      return {
        text,
        sourceMap: text === source ? null : Int32Array.from(sourceOffsets),
      }
    },
  }
}

function formatJsonPreviewSource(source: string) {
  const tokens = tokenizeJsonPreviewSource(source)
  if (!tokens?.length) return null
  const builder = createMappedPreviewBuilder(source)
  const stack: string[] = []
  let depth = 0
  let atLineStart = true
  let previous: PreviewToken | null = null

  const indent = () => {
    if (!atLineStart) return
    builder.appendText(STRUCTURED_INDENT.repeat(Math.min(depth, 64)))
    atLineStart = false
  }
  const newline = () => {
    if (!atLineStart) builder.appendText('\n')
    atLineStart = true
  }
  const space = () => {
    if (!atLineStart) builder.appendText(' ')
  }
  const appendToken = (token: PreviewToken) => {
    indent()
    builder.appendSource(token.start, token.end)
    atLineStart = source.slice(token.start, token.end).endsWith('\n')
  }

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index]!
    const next = tokens[index + 1]
    if (token.kind === 'open') {
      if (depth === 0 && previous && !atLineStart) newline()
      appendToken(token)
      stack.push(token.value)
      depth += 1
      if (!(next?.kind === 'close' && closesJsonToken(token.value, next.value))) newline()
    } else if (token.kind === 'close') {
      const opening = stack.pop()
      if (!opening || !closesJsonToken(opening, token.value)) return null
      depth = Math.max(0, depth - 1)
      if (!(previous?.kind === 'open' && closesJsonToken(previous.value, token.value))) newline()
      appendToken(token)
    } else if (token.kind === 'comma') {
      appendToken(token)
      newline()
    } else if (token.kind === 'colon') {
      appendToken(token)
      space()
    } else if (token.kind === 'line-comment') {
      if (!atLineStart) space()
      appendToken(token)
      if (next) newline()
    } else if (token.kind === 'block-comment') {
      if (!atLineStart) space()
      appendToken(token)
      if (next && next.kind !== 'comma' && next.kind !== 'colon' && next.kind !== 'close') space()
    } else {
      if (depth === 0 && previous && !atLineStart) newline()
      appendToken(token)
    }
    previous = token
  }
  if (stack.length) return null
  if (/\r?\n$/u.test(source) && !atLineStart) newline()
  return builder.finish()
}

function tokenizeJsonPreviewSource(source: string) {
  const tokens: PreviewToken[] = []
  for (let index = 0; index < source.length;) {
    const character = source[index]!
    if (/\s/u.test(character)) {
      index += 1
      continue
    }
    if (character === '"' || character === "'") {
      const end = quotedTokenEnd(source, index, character)
      if (end == null) return null
      tokens.push({ kind: 'value', start: index, end, value: source.slice(index, end) })
      index = end
      continue
    }
    if (character === '/' && source[index + 1] === '/') {
      let end = index + 2
      while (end < source.length && source[end] !== '\n' && source[end] !== '\r') end += 1
      tokens.push({ kind: 'line-comment', start: index, end, value: source.slice(index, end) })
      index = end
      continue
    }
    if (character === '/' && source[index + 1] === '*') {
      const closing = source.indexOf('*/', index + 2)
      if (closing < 0) return null
      const end = closing + 2
      tokens.push({ kind: 'block-comment', start: index, end, value: source.slice(index, end) })
      index = end
      continue
    }
    const structuralKind = ({
      '{': 'open', '[': 'open', '}': 'close', ']': 'close', ',': 'comma', ':': 'colon',
    } as Record<string, string>)[character]
    if (structuralKind) {
      tokens.push({ kind: structuralKind, start: index, end: index + 1, value: character })
      index += 1
      continue
    }
    let end = index + 1
    while (end < source.length) {
      const next = source[end]!
      if (/\s/u.test(next) || /[{}\[\],:'"]/u.test(next)) break
      if (next === '/' && (source[end + 1] === '/' || source[end + 1] === '*')) break
      end += 1
    }
    tokens.push({ kind: 'value', start: index, end, value: source.slice(index, end) })
    index = end
  }
  return tokens
}

function quotedTokenEnd(source: string, start: number, quote: string) {
  for (let index = start + 1; index < source.length; index += 1) {
    if (source[index] === '\\') {
      index += 1
      continue
    }
    if (source[index] === quote) return index + 1
  }
  return null
}

function closesJsonToken(opening: string, closing: string) {
  return (opening === '{' && closing === '}') || (opening === '[' && closing === ']')
}

function formatXmlPreviewSource(source: string) {
  const tokens = tokenizeXmlPreviewSource(source)
  if (!tokens?.length || !xmlPreviewStructureIsSafe(tokens)) return null
  const visible = tokens.filter((token) => token.kind !== 'text' || token.value.trim())
  const builder = createMappedPreviewBuilder(source)
  let depth = 0
  let atLineStart = true

  const indent = () => {
    if (!atLineStart) return
    builder.appendText(STRUCTURED_INDENT.repeat(Math.min(depth, 64)))
    atLineStart = false
  }
  const newline = () => {
    if (!atLineStart) builder.appendText('\n')
    atLineStart = true
  }
  const appendToken = (token: PreviewToken) => {
    indent()
    builder.appendSource(token.start, token.end)
    atLineStart = token.value.endsWith('\n')
  }

  for (let index = 0; index < visible.length; index += 1) {
    const token = visible[index]!
    const text = visible[index + 1]
    const closing = visible[index + 2]
    if (token.kind === 'start' && text && ['text', 'cdata'].includes(text.kind) && closing?.kind === 'end' && closing.name === token.name) {
      appendToken(token)
      builder.appendSource(text.start, text.end)
      builder.appendSource(closing.start, closing.end)
      atLineStart = closing.value.endsWith('\n')
      if (index + 3 < visible.length) newline()
      index += 2
      continue
    }
    if (token.kind === 'start') {
      appendToken(token)
      depth += 1
      if (index + 1 < visible.length) newline()
    } else if (token.kind === 'end') {
      depth = Math.max(0, depth - 1)
      newline()
      appendToken(token)
      if (index + 1 < visible.length) newline()
    } else if (token.kind === 'text') {
      appendToken(token)
      if (index + 1 < visible.length) newline()
    } else {
      appendToken(token)
      if (index + 1 < visible.length) newline()
    }
  }
  if (/\r?\n$/u.test(source) && !atLineStart) newline()
  return builder.finish()
}

function tokenizeXmlPreviewSource(source: string) {
  const tokens: PreviewToken[] = []
  for (let index = 0; index < source.length;) {
    if (source[index] !== '<') {
      let end = source.indexOf('<', index)
      if (end < 0) end = source.length
      tokens.push({ kind: 'text', start: index, end, value: source.slice(index, end), name: '' })
      index = end
      continue
    }
    const end = xmlMarkupEnd(source, index)
    if (end == null) return null
    const value = source.slice(index, end)
    let kind = value.startsWith('<![CDATA[') ? 'cdata' : 'markup'
    let name = ''
    const endMatch = value.match(/^<\/\s*([^\s>]+)/u)
    const startMatch = value.match(/^<\s*([^!?/\s>]+)/u)
    if (endMatch) {
      kind = 'end'
      name = endMatch[1]!
    } else if (startMatch) {
      name = startMatch[1]!
      kind = /\/\s*>$/u.test(value) ? 'self' : 'start'
    }
    tokens.push({ kind, start: index, end, value, name })
    index = end
  }
  return tokens
}

function xmlMarkupEnd(source: string, start: number) {
  const terminal = source.startsWith('<!--', start) ? '-->'
    : source.startsWith('<![CDATA[', start) ? ']]>'
      : source.startsWith('<?', start) ? '?>' : null
  if (terminal) {
    const closing = source.indexOf(terminal, start + terminal.length)
    return closing < 0 ? null : closing + terminal.length
  }
  let quote = ''
  let subsetDepth = 0
  for (let index = start + 1; index < source.length; index += 1) {
    const character = source[index]!
    if (quote) {
      if (character === quote) quote = ''
      continue
    }
    if (character === '"' || character === "'") {
      quote = character
      continue
    }
    if (character === '[') subsetDepth += 1
    else if (character === ']') subsetDepth = Math.max(0, subsetDepth - 1)
    else if (character === '>' && subsetDepth === 0) return index + 1
  }
  return null
}

function xmlPreviewStructureIsSafe(tokens: PreviewToken[]) {
  const stack: { name: string | undefined; hasChild: boolean; hasText: boolean }[] = []
  for (const token of tokens) {
    if (token.kind === 'start') {
      if (stack.length) stack.at(-1)!.hasChild = true
      stack.push({ name: token.name, hasChild: false, hasText: false })
    } else if (token.kind === 'self') {
      if (stack.length) stack.at(-1)!.hasChild = true
    } else if ((token.kind === 'text' && token.value.trim()) || token.kind === 'cdata') {
      if (!stack.length) return false
      stack.at(-1)!.hasText = true
    } else if (token.kind === 'markup' && stack.length) {
      stack.at(-1)!.hasChild = true
    } else if (token.kind === 'end') {
      const current = stack.pop()
      if (!current || current.name !== token.name || (current.hasChild && current.hasText)) return false
    }
  }
  return stack.length === 0
}

function formatAssignmentPreviewSource(source: string, delimiters: string) {
  const builder = createMappedPreviewBuilder(source)
  let cursor = 0
  let inTomlMultiline = ''
  while (cursor < source.length) {
    const newline = source.indexOf('\n', cursor)
    const end = newline < 0 ? source.length : newline
    const contentEnd = end > cursor && source[end - 1] === '\r' ? end - 1 : end
    const line = source.slice(cursor, contentEnd)
    const triple = delimiters === '=' ? tomlMultilineDelimiter(line, inTomlMultiline) : ''
    const delimiter = !inTomlMultiline && !/^\s*(?:[#;]|\[)/u.test(line)
      ? assignmentDelimiterIndex(line, delimiters)
      : -1
    if (delimiter >= 0) {
      let leftEnd = delimiter
      while (leftEnd > 0 && /[\t ]/u.test(line[leftEnd - 1]!)) leftEnd -= 1
      let rightStart = delimiter + 1
      while (rightStart < line.length && /[\t ]/u.test(line[rightStart]!)) rightStart += 1
      builder.appendSource(cursor, cursor + leftEnd)
      builder.appendText(' ')
      builder.appendSource(cursor + delimiter, cursor + delimiter + 1)
      builder.appendText(' ')
      builder.appendSource(cursor + rightStart, contentEnd)
    } else {
      builder.appendSource(cursor, contentEnd)
    }
    if (newline >= 0) builder.appendSource(contentEnd, newline + 1)
    if (inTomlMultiline && triple === inTomlMultiline) inTomlMultiline = ''
    else if (!inTomlMultiline && triple) inTomlMultiline = triple
    cursor = newline < 0 ? source.length : newline + 1
  }
  return builder.finish()
}

function assignmentDelimiterIndex(line: string, delimiters: string) {
  let quote = ''
  let escaped = false
  let colon = -1
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index]!
    if (escaped) {
      escaped = false
      continue
    }
    if (quote) {
      if (character === '\\' && quote === '"') escaped = true
      else if (character === quote) quote = ''
      continue
    }
    if (character === '"' || character === "'") {
      quote = character
      continue
    }
    if (character === '=' && delimiters.includes(character)) return index
    if (character === ':' && delimiters.includes(character) && colon < 0) colon = index
    if (character === '#' || character === ';') return -1
  }
  return colon
}

function tomlMultilineDelimiter(line: string, active: string) {
  const candidates = active ? [active] : ['"""', "'''"]
  for (const candidate of candidates) {
    const first = line.indexOf(candidate)
    if (first < 0) continue
    if (active || line.indexOf(candidate, first + candidate.length) < 0) return candidate
  }
  return ''
}

function renderJsonTokens(source: string) {
  return renderTokenMatches(source, JSON_TOKEN_PATTERN, (token, index) => {
    if (token.startsWith('//') || token.startsWith('/*')) return 'comment'
    if (token.startsWith('"') || token.startsWith("'")) {
      return /^\s*:/u.test(source.slice(index + token.length)) ? 'key' : 'string'
    }
    if (/^[{}\[\],:]$/u.test(token)) return 'punctuation'
    if (/^(?:true|false|null|Infinity|NaN|undefined)$/u.test(token)) return 'literal'
    if (/^-?\d/u.test(token)) return 'number'
    return 'key'
  })
}

function renderYamlTokens(source: string) {
  return renderTokenMatches(source, YAML_TOKEN_PATTERN, (token, index) => {
    const trimmed = token.trimStart()
    if (trimmed.startsWith('#')) {
      const hashIndex = index + token.indexOf('#')
      return hashIndex === 0 || /\s/u.test(source[hashIndex - 1]!) ? 'comment' : ''
    }
    if (/^(?:---|\.\.\.)$/u.test(trimmed)) return 'document'
    if (/^["']/u.test(trimmed)) return 'string'
    if (/^[&*!]/u.test(trimmed)) return 'symbol'
    if (/^(?:true|false|null|yes|no|on|off|~)$/iu.test(trimmed)) return 'literal'
    if (/^-?\d/u.test(trimmed)) return 'number'
    if (/^[{}\[\],:|>]$/u.test(trimmed)) return 'punctuation'
    return 'key'
  })
}

function renderXmlTokens(source: string) {
  return renderTokenMatches(source, XML_TOKEN_PATTERN, (token) => {
    if (token.startsWith('<!--')) return 'comment'
    if (token.startsWith('<![CDATA[')) return 'cdata'
    if (token.startsWith('<?') || /^<!DOCTYPE/iu.test(token)) return 'document'
    if (token.startsWith('&')) return 'entity'
    return 'tag'
  })
}

function renderTomlTokens(source: string) {
  return renderTokenMatches(source, TOML_TOKEN_PATTERN, (token, index) => {
    const trimmed = token.trimStart()
    if (trimmed.startsWith('#')) return 'comment'
    if (trimmed.startsWith('[')) return 'section'
    if (/^["']/u.test(trimmed)) {
      return /^\s*=/u.test(source.slice(index + token.length)) ? 'key' : 'string'
    }
    if (/^\s*=/u.test(source.slice(index + token.length))) return 'key'
    if (/^(?:true|false)$/iu.test(trimmed)) return 'literal'
    if (/^-?\d/u.test(trimmed)) return 'number'
    return 'punctuation'
  })
}

function renderConfigTokens(source: string) {
  return renderTokenMatches(source, CONFIG_TOKEN_PATTERN, (token, index) => {
    const trimmed = token.trimStart()
    if (/^[;#]/u.test(trimmed)) return 'comment'
    if (trimmed.startsWith('[')) return 'section'
    if (/^["']/u.test(trimmed)) return 'string'
    if (/^\s*[=:]/u.test(source.slice(index + token.length))) return 'key'
    if (/^(?:true|false|yes|no|on|off|null|none)$/iu.test(trimmed)) return 'literal'
    if (/^-?\d/u.test(trimmed)) return 'number'
    return 'punctuation'
  })
}

function renderTokenMatches(source: string, pattern: RegExp, tokenClass: (token: string, index: number) => string) {
  pattern.lastIndex = 0
  let cursor = 0
  let html = ''
  for (const match of source.matchAll(pattern)) {
    const index = match.index ?? 0
    if (index < cursor) continue
    html += escapePreviewHtml(source.slice(cursor, index))
    const className = tokenClass(match[0], index)
    const token = escapePreviewHtml(match[0])
    html += className ? `<span class="artifact-token-${className}">${token}</span>` : token
    cursor = index + match[0].length
  }
  return html + escapePreviewHtml(source.slice(cursor))
}

function escapePreviewHtml(value: unknown) {
  return String(value ?? '').replace(/[&<>'"]/gu, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;',
  } as Record<string, string>)[character]!)
}

function finiteOffset(value: unknown) {
  if (value == null || typeof value === 'boolean') return null
  if (typeof value === 'string' && !value.trim()) return null
  const number = Number(value)
  return Number.isInteger(number) && number >= 0 ? number : null
}

function finiteLine(value: unknown) {
  const line = finiteOffset(value)
  return line != null && line >= 1 ? line : null
}

