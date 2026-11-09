// Selection occurrences are deliberately local and case-sensitive. Unicode
// letters, combining marks, numbers and '_' form words; all-Han selections
// may match a bounded phrase within adjacent Han text (native double-click).
// No document fetch, text replacement, or search/comment decoration ownership.
export const WORD_HIGHLIGHT_LIMITS = Object.freeze({ bytes: 128 * 1024, nodes: 2000, marks: 500, word: 128, frameMs: 4 })
const token = /^[\p{L}\p{M}\p{N}_]+$/u
const wordCharacter = /[\p{L}\p{M}\p{N}_]/u
const han = /^\p{Script=Han}+$/u
const hanCharacter = /\p{Script=Han}/u
const blockTags = /^(ADDRESS|ARTICLE|ASIDE|BLOCKQUOTE|BR|DD|DIV|DL|DT|FIGCAPTION|FIGURE|FOOTER|H[1-6]|HEADER|HR|LI|MAIN|NAV|OL|P|PRE|SECTION|TABLE|TBODY|TD|TH|THEAD|TR|UL)$/u
const excludedTags = /^(BUTTON|INPUT|SELECT|TEXTAREA|OPTION|SCRIPT|STYLE|SVG|CANVAS|VIDEO|AUDIO)$/u
const encoder = new TextEncoder()
const idKey = Symbol.for('marknoria.selectionWordId')
export function isHighlightWord(word, before = '', after = '') {
  if (!word || word.length > 256 || [...word].length > WORD_HIGHLIGHT_LIMITS.word || !token.test(word)) return false
  const isBoundary = char => !char || !wordCharacter.test(char) || (han.test(word) && hanCharacter.test(char))
  return isBoundary(before) && isBoundary(after)
}
const firstCharacter = text => Array.from(text)[0] || ''
const lastCharacter = text => Array.from(text).at(-1) || ''
function blockOf(node, root, visit = () => true) {
  let element = node.parentElement
  // Ancestor inspection is bounded too; deeply nested input is declined.
  for (let depth = 0; element && depth < 64; depth++, element = element.parentElement) {
    if (!visit()) return null
    if (element === root || blockTags.test(element.tagName) || element.classList.contains('cm-line')) return element
  }
  return null
}
function adjacentCharacter(node, direction, root, visit) {
  const block = blockOf(node, root, visit)
  let cursor = node
  for (let count = 0; cursor && cursor !== block && count < 128; count++) {
    if (!visit()) return ''
    const sibling = direction < 0 ? cursor.previousSibling : cursor.nextSibling
    if (!sibling) { cursor = cursor.parentNode; continue }
    cursor = sibling
    for (; cursor.nodeType === 1 && (direction < 0 ? cursor.lastChild : cursor.firstChild) && count < 128; count++) {
      if (!visit() || blockTags.test(cursor.tagName) || excludedTags.test(cursor.tagName)) return ''
      cursor = direction < 0 ? cursor.lastChild : cursor.firstChild
    }
    if (cursor.nodeType === 3 && cursor.length) return direction < 0 ? lastCharacter(cursor.substringData(Math.max(0, cursor.length - 2), 2)) : firstCharacter(cursor.substringData(0, 2))
  }
  return ''
}
function textBoundary(container, offset, atEnd, visit) {
  if (container.nodeType === 3) return { node: container, offset }
  // DOM Range.selectNodeContents and keyboard selections can use element
  // boundaries. Resolve only the bounded edge path, never stringify the range.
  let node = container.childNodes[atEnd ? offset - 1 : offset]
  for (let steps = 0; node && steps < 128; steps++) {
    if (!visit()) return null
    if (node.nodeType === 3) return { node, offset: atEnd ? node.length : 0 }
    const child = atEnd ? node.lastChild : node.firstChild
    if (child) node = child
    else {
      while (node && node !== container && !(atEnd ? node.previousSibling : node.nextSibling) && steps++ < 128) {
        if (!visit()) return null
        node = node.parentNode
      }
      node = node && node !== container ? (atEnd ? node.previousSibling : node.nextSibling) : null
    }
  }
  return null
}
function nativeWord(root, visit) {
  const selection = root.ownerDocument.getSelection()
  if (!selection || selection.isCollapsed || selection.rangeCount !== 1) return null
  const range = selection.getRangeAt(0)
  const first = textBoundary(range.startContainer, range.startOffset, false, visit)
  const last = textBoundary(range.endContainer, range.endOffset, true, visit)
  if (!first || !last) return null
  const start = first.node, end = last.node
  if (!root.contains(start) || !root.contains(end)) return null
  if (blockOf(start, root, visit) !== blockOf(end, root, visit)) return null
  let node = start, word = '', visited = 0
  while (node && visited++ < 128) {
    if (!visit()) return null
    if (node.nodeType === 3) {
      const from = node === start ? first.offset : 0
      const to = node === end ? last.offset : node.length
      if (to - from + word.length > 256) return null
      word += node.substringData(from, to - from)
      if (node === end) break
    }
    if (node.firstChild) node = node.firstChild
    else {
      while (node && node !== root && !node.nextSibling && visited++ < 128) { if (!visit()) return null; node = node.parentNode }
      node = node && node !== root ? node.nextSibling : null
    }
  }
  if (node !== end) return null
  const before = first.offset ? lastCharacter(start.substringData(Math.max(0, first.offset - 2), Math.min(2, first.offset))) : adjacentCharacter(start, -1, root, visit)
  const after = last.offset < end.length ? firstCharacter(end.substringData(last.offset, 2)) : adjacentCharacter(end, 1, root, visit)
  return isHighlightWord(word, before, after) ? { word, node: start, offset: first.offset } : null
}

export function mountWordHighlights(root, { viewport = root, enabled = false, signal, forceOverlay = false, selection: selectionProvider } = {}) {
  const document = root.ownerDocument, window = document.defaultView
  document[idKey] = (document[idKey] || 0) + 1
  const name = `marknoria-selection-word-${document[idKey]}`
  const cssHighlights = !forceOverlay && window.CSS?.highlights && window.Highlight
  const style = document.createElement('style')
  style.textContent = `::highlight(${name}) { background-color: rgba(230,170,45,.34); text-decoration: underline rgba(180,120,20,.6); }`
  document.head.append(style)
  const layer = document.createElement('div')
  layer.dataset.wordHighlightOverlay = name
  layer.setAttribute('aria-hidden', 'true')
  layer.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:4;overflow:hidden;contain:strict'
  if (!cssHighlights) document.body.append(layer)
  let disposed = false, timer = 0, frame = 0, generation = 0
  let diagnostics = emptyState()
  function emptyState() { return { enabled: Boolean(enabled), pending: false, word: '', visitedNodes: 0, bytes: 0, matches: 0, marks: 0, maxFrameMs: 0, truncated: false, backend: cssHighlights ? 'css' : 'overlay' } }
  function erase() { if (cssHighlights) window.CSS.highlights.delete(name); layer.replaceChildren() }
  function clear() {
    generation++
    window.clearTimeout(timer); window.cancelAnimationFrame(frame)
    timer = frame = 0
    erase(); diagnostics = emptyState()
  }
  function bounds() {
    const rect = viewport?.getBoundingClientRect?.() || { top: 0, left: 0, right: window.innerWidth, bottom: window.innerHeight }
    if (!cssHighlights) layer.style.clipPath = `inset(${Math.max(0, rect.top)}px ${Math.max(0, window.innerWidth - rect.right)}px ${Math.max(0, window.innerHeight - rect.bottom)}px ${Math.max(0, rect.left)}px)`
    const height = Math.min(window.innerHeight, Math.max(1, rect.bottom - rect.top))
    return { visibleTop: Math.max(0, rect.top), visibleBottom: Math.min(window.innerHeight, rect.bottom), top: Math.max(0, rect.top) - height, bottom: Math.min(window.innerHeight, rect.bottom) + height, left: Math.max(0, rect.left), right: Math.min(window.innerWidth, rect.right) }
  }
  const intersects = (rect, band) => rect.bottom >= band.top && rect.top <= band.bottom && rect.right >= band.left && rect.left <= band.right
  function* collect(info, band, ranges) {
    // One explicit DFS cursor: never materialize root.querySelectorAll()/all text.
    let node = root.firstChild, pending = [], text = '', group = null
    function* flush() {
      if (!text) return
      let offset = 0, segment = 0
      while (offset <= text.length - info.word.length && diagnostics.matches < WORD_HIGHLIGHT_LIMITS.marks) {
        // Bound each substring search to a small slab, including lookahead.
        const stop = Math.min(text.length, offset + 1024)
        const found = text.slice(offset, Math.min(text.length, stop + info.word.length)).indexOf(info.word)
        if (found < 0) { offset = stop; yield; continue }
        const from = offset + found, to = from + info.word.length
        offset = from + Math.max(1, info.word.length)
        const first = pending[0], final = pending.at(-1)
        const before = from ? lastCharacter(text.slice(Math.max(0, from - 2), from)) : (first.offset ? lastCharacter(first.node.substringData(Math.max(0, first.offset - 2), Math.min(2, first.offset))) : '')
        const finalOffset = final.offset + final.end - final.start
        const after = to < text.length ? firstCharacter(text.slice(to, to + 2)) : firstCharacter(final.node.substringData(finalOffset, 2))
        if (!isHighlightWord(info.word, before, after)) { yield; continue }
        while (segment + 1 < pending.length && pending[segment].end <= from) segment++
        let last = segment
        while (last + 1 < pending.length && pending[last].end < to) last++
        const a = pending[segment], b = pending[last]
        const range = document.createRange()
        range.setStart(a.node, a.offset + from - a.start)
        range.setEnd(b.node, b.offset + to - b.start)
        const rect = range.getBoundingClientRect()
        // Overscan bounds traversal; reserve the paint cap for visible matches
        // so a dense screen above the viewport cannot starve the current one.
        if (intersects(rect, band) && rect.bottom >= band.visibleTop && rect.top <= band.visibleBottom) { ranges.push(range); diagnostics.matches++ }
        yield
      }
      if (diagnostics.matches >= WORD_HIGHLIGHT_LIMITS.marks) diagnostics.truncated = true
      pending = []; text = ''; group = null
    }
    while (node && diagnostics.visitedNodes < WORD_HIGHLIGHT_LIMITS.nodes && diagnostics.bytes < WORD_HIGHLIGHT_LIMITS.bytes && diagnostics.matches < WORD_HIGHLIGHT_LIMITS.marks) {
      diagnostics.visitedNodes++
      let descend = true
      if (node.nodeType === 1) {
        const element = node
        if (excludedTags.test(element.tagName) || element.hidden || element.getAttribute('aria-hidden') === 'true' || element.getAttribute('contenteditable') === 'true' || element.matches('.cm-gutters,.cm-panels,.table-controls,[role=toolbar],[data-word-highlight-ignore]')) descend = false
        else {
          const display = window.getComputedStyle(element)
          if (display.display === 'none' || display.visibility !== 'visible' || display.contentVisibility === 'hidden') descend = false
          else if (display.display !== 'contents' && !intersects(element.getBoundingClientRect(), band)) descend = false
        }
        if (!descend || blockTags.test(element.tagName) || element.classList.contains('cm-line')) yield* flush()
      } else if (node.nodeType === 3 && node.length) {
        const block = blockOf(node, root, () => diagnostics.visitedNodes < WORD_HIGHLIGHT_LIMITS.nodes && ++diagnostics.visitedNodes)
        if (!block || group !== block) yield* flush()
        group = block
        // Long single nodes are sampled in a bounded window around selection or
        // the actual visible caret, rather than copied or segmented in full.
        let start = 0
        const maxUnits = Math.min(16384, Math.floor((WORD_HIGHLIGHT_LIMITS.bytes - diagnostics.bytes) / 3))
        if (node.length > maxUnits) {
          yield* flush()
          group = block
          let center = info.node === node ? info.offset : 0
          if (info.node !== node) {
            const x = Math.max(0, band.left + 8), y = Math.max(0, Math.min(window.innerHeight - 1, band.top + (band.bottom - band.top) / 3))
            const caret = document.caretPositionFromPoint?.(x, y)
            const range = !caret && document.caretRangeFromPoint?.(x, y)
            if ((caret?.offsetNode || range?.startContainer) === node) center = caret?.offset ?? range.startOffset
          }
          start = Math.max(0, Math.min(node.length - maxUnits, center - Math.floor(maxUnits / 2)))
          diagnostics.truncated = true
        }
        let remaining = Math.min(node.length - start, maxUnits)
        while (remaining > 0) {
          const size = Math.min(1024, remaining)
          const chunk = node.substringData(start, size)
          const sizeBytes = encoder.encode(chunk).length
          if (diagnostics.bytes + sizeBytes > WORD_HIGHLIGHT_LIMITS.bytes) break
          diagnostics.bytes += sizeBytes
          pending.push({ node, offset: start, start: text.length, end: text.length + chunk.length })
          text += chunk
          start += size; remaining -= size
          yield
        }
        if (node.length > maxUnits) yield* flush()
      }
      yield
      if (descend && node.firstChild) node = node.firstChild
      else {
        // Ascending also counts toward the total traversal budget.
        while (node && node !== root && !node.nextSibling) {
          if (node.nodeType === 1 && (blockTags.test(node.tagName) || node.classList.contains('cm-line'))) yield* flush()
          node = node.parentNode
          if (diagnostics.visitedNodes >= WORD_HIGHLIGHT_LIMITS.nodes) break
          diagnostics.visitedNodes++
          yield
        }
        node = node && node !== root ? node.nextSibling : null
      }
    }
    if (node) diagnostics.truncated = true
    yield* flush()
  }
  function* paint(ranges, band) {
    if (cssHighlights) {
      const highlight = new window.Highlight()
      window.CSS.highlights.set(name, highlight)
      for (const range of ranges) { highlight.add(range); diagnostics.marks++; yield }
    } else {
      for (const range of ranges) {
        // getClientRects can explode for pathological wrapping. A selected word
        // has at most 128 codepoints, and the total output remains capped at 500.
        for (const rect of range.getClientRects()) {
          if (diagnostics.marks >= WORD_HIGHLIGHT_LIMITS.marks) { diagnostics.truncated = true; return }
          if (!intersects(rect, band) || !rect.width || !rect.height) continue
          const mark = document.createElement('span')
          mark.style.cssText = `position:absolute;pointer-events:none;left:${rect.left}px;top:${rect.top}px;width:${rect.width}px;height:${rect.height}px;background:rgba(230,170,45,.25);box-shadow:inset 0 -1px rgba(180,120,20,.6)`
          layer.append(mark); diagnostics.marks++; yield
        }
      }
    }
  }
  function refresh() {
    clear()
    if (!enabled || disposed || signal?.aborted) return
    diagnostics.pending = true
    const current = generation
    timer = window.setTimeout(() => {
      timer = 0
      const info = selectionProvider ? selectionProvider() : nativeWord(root, () => diagnostics.visitedNodes < WORD_HIGHLIGHT_LIMITS.nodes && ++diagnostics.visitedNodes)
      if (!info || !isHighlightWord(info.word, info.before, info.after)) { diagnostics.pending = false; return }
      diagnostics.word = info.word
      const band = bounds(), ranges = []
      const work = (function* () { yield* collect(info, band, ranges); yield* paint(ranges, band) })()
      const advance = () => {
        if (current !== generation || disposed || !enabled) return
        const start = window.performance.now()
        let done = false
        do { done = work.next().done } // Leave one millisecond for the final DOM operation/layout.
        while (!done && window.performance.now() - start < WORD_HIGHLIGHT_LIMITS.frameMs - 1)
        diagnostics.maxFrameMs = Math.max(diagnostics.maxFrameMs, window.performance.now() - start)
        if (done) { frame = 0; diagnostics.pending = false }
        else frame = window.requestAnimationFrame(advance)
      }
      frame = window.requestAnimationFrame(advance)
    }, 60)
  }
  document.addEventListener('selectionchange', refresh)
  root.addEventListener('dblclick', refresh)
  window.addEventListener('scroll', refresh, true)
  window.addEventListener('resize', refresh)
  const observer = new window.MutationObserver(refresh)
  observer.observe(root, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ['hidden', 'aria-hidden', 'style', 'class'] })
  function dispose() {
    if (disposed) return
    clear(); disposed = true
    observer.disconnect()
    document.removeEventListener('selectionchange', refresh)
    root.removeEventListener('dblclick', refresh)
    window.removeEventListener('scroll', refresh, true)
    window.removeEventListener('resize', refresh)
    signal?.removeEventListener('abort', dispose)
    layer.remove(); style.remove()
  }
  signal?.addEventListener('abort', dispose, { once: true })
  if (signal?.aborted) dispose()
  else if (enabled) refresh()
  return { update(options) { if ('enabled' in options) enabled = Boolean(options.enabled); refresh() }, clear, refresh, dispose, state: () => ({ ...diagnostics }) }
}
