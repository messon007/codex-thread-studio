// Revision-owned rendered coordinates. These are never original source bytes.
const MAX_TEXT = 262144, MAX_NODES = 32768
const registered = new WeakMap(), fingerprints = new WeakMap()
const ignored = 'button,input,textarea,select,script,style,noscript,.table-controls,.asset-controls,figure[data-diagram],.comment-mark-popup,[data-comment-overlay]'
export function commentScopeKey(scope) {
  if (scope.kind === 'projection') return `p:${scope.page}`
  if (scope.kind === 'cell') return `c:${scope.table}:${scope.row}:${scope.column}`
  if (scope.kind === 'queryCell') return `q:${scope.queryId}:${scope.row}:${scope.column}`
  throw Error('Unsupported rendered comment scope')
}
export function registerCommentScope(node, scope, owner = node) {
  if (scope && ['cell', 'queryCell'].includes(scope.kind)) registered.set(node, { scope: { ...scope }, owner })
}
export function commentTextMap(root) {
  const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let node, text = '', visited = 0
  const entries = []
  while ((node = walker.nextNode())) {
    if (++visited > MAX_NODES) throw Error('Comment scope exceeds its text-node budget')
    if (node.parentElement?.closest(ignored)) continue
    if (text.length + node.length > MAX_TEXT) throw Error('Comment scope exceeds its text budget')
    entries.push({ node, start: text.length, end: text.length + node.length })
    text += node.data
  }
  return { text, entries }
}
export async function commentTextHash(node, text) {
  const cached = fingerprints.get(node)
  if (cached?.text === text) return cached.hash
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))
  const hash = [...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, '0')).join('')
  fingerprints.set(node, { text, hash })
  return hash
}
const boundary = (text, offset) => Number.isSafeInteger(offset) && offset >= 0 && offset <= text.length && !(offset && offset < text.length && /[\uD800-\uDBFF]/.test(text[offset - 1]) && /[\uDC00-\uDFFF]/.test(text[offset]))
export function commentRangeAt(map, from, to) {
  if (!boundary(map.text, from) || !boundary(map.text, to) || to <= from) return null
  const first = map.entries.find(entry => entry.end > from), last = map.entries.find(entry => entry.end >= to && entry.end > entry.start)
  if (!first || !last) return null
  const range = first.node.ownerDocument.createRange()
  range.setStart(first.node, from - first.start); range.setEnd(last.node, to - last.start)
  return range
}
function clippedOffsets(map, range) {
  let from = null, to = null
  for (const entry of map.entries) {
    const node = entry.node
    if (range.comparePoint(node, node.length) < 0) continue
    if (range.comparePoint(node, 0) > 0) break
    const start = node === range.startContainer ? range.startOffset : 0
    const end = node === range.endContainer ? range.endOffset : node.length
    if (end <= start) continue
    from ??= entry.start + start; to = entry.start + end
  }
  return from == null ? null : { from, to }
}
export async function captureCommentRange(range, scopes, signal) {
  if (!range || range.collapsed) return undefined
  const captured = range.cloneRange(), segments = []
  for (const { node, scope } of scopes) {
    signal?.throwIfAborted()
    if (!node.isConnected || !captured.intersectsNode(node)) continue
    if (segments.length >= 32) return undefined // Never save a partial selector.
    const map = commentTextMap(node), offsets = clippedOffsets(map, captured)
    if (!offsets) continue
    const textHash = await commentTextHash(node, map.text)
    signal?.throwIfAborted()
    if (!node.isConnected || commentTextMap(node).text !== map.text) throw new DOMException('Comment selection changed', 'AbortError')
    segments.push({ scope, startUtf16: offsets.from, endUtf16: offsets.to, textHash })
  }
  return segments.length ? { version: 1, segments } : undefined
}
export function commentScopes(root, { viewport = root, includeDialogs = true } = {}) {
  const result = [], bounds = viewport.getBoundingClientRect()
  const visible = node => {
    const box = node.getBoundingClientRect()
    return box.width > 0 && box.height > 0 && box.bottom > bounds.top - 160 && box.top < bounds.bottom + 160 && box.right > bounds.left && box.left < bounds.right
      && !node.closest('[inert],[aria-hidden="true"]')
  }
  for (const node of root.querySelectorAll('article[data-page]')) {
    if (node.dataset.table != null || !visible(node) || (!node.dataset.ready && !node.hasAttribute('data-marknoria-prose'))) continue
    const page = Number(node.dataset.page)
    if (Number.isSafeInteger(page) && page >= 0) result.push({ node, scope: { kind: 'projection', page } })
  }
  const candidates = [...root.querySelectorAll('tbody td')]
  if (includeDialogs) candidates.push(...root.ownerDocument.querySelectorAll('.table-cell-full,dialog[open] tbody td'))
  for (const node of candidates) {
    const entry = registered.get(node)
    if (entry && (root.contains(entry.owner) || root === entry.owner) && visible(node)) result.push({ node, scope: entry.scope })
  }
  if (result.length > 512) throw Error('Comment view exceeds 512 rendered scopes')
  return result
}
export async function captureProjectionComment(root, range, location, signal) {
  if (!range || range.collapsed) return undefined
  if (['cell', 'queryCell'].includes(location.kind)) {
    const start = range.startContainer.nodeType === Node.ELEMENT_NODE ? range.startContainer : range.startContainer.parentElement
    const node = start?.closest('td,.table-cell-full')
    if (!node || !node.contains(range.endContainer)) return undefined
    const entry = registered.get(node)
    if (!entry || commentScopeKey(entry.scope) !== commentScopeKey(location)) return undefined
    return captureCommentRange(range, [{ node, scope: entry.scope }], signal)
  }
  // Capture all intersected pages, not just currently visible ones. Scope text
  // and endpoints are bounded, and an oversized selection gets no guessed mark.
  const scopes = [...root.querySelectorAll('article[data-page]')].filter(node => node.dataset.table == null && range.intersectsNode(node))
    .map(node => ({ node, scope: { kind: 'projection', page: Number(node.dataset.page) } }))
  if (scopes.length > 32) return undefined
  return captureCommentRange(range, scopes, signal)
}
