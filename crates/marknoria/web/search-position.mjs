// DOM-only mapping of bounded Rust projections. All coordinates in data-source-*
// refer to the original snapshot, never to formatted JSON or syntax HTML.
import { literalSearchPattern } from './search-marker.mjs'
const selector = '[data-source-line]'
const MAX_TEXT = 262144, MAX_NODES = 32768, MAX_MATCHES = 500
function textMap(root) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  const entries = []; let text = '', node, count = 0
  while ((node = walker.nextNode()) && ++count <= MAX_NODES && text.length < MAX_TEXT) {
    if (node.parentElement.closest('button,select,textarea,input,script,style,[hidden],.table-controls,.asset-controls')) continue
    const value = node.data.slice(0, MAX_TEXT - text.length)
    entries.push({ node, start: text.length, end: text.length + value.length }); text += value
  }
  return { text, entries }
}
function rangeAt(map, start, end) {
  const first = map.entries.find(entry => entry.end > start)
  const last = map.entries.find(entry => entry.end >= end && entry.end > entry.start)
  if (!first || !last) return null
  const range = document.createRange()
  range.setStart(first.node, start - first.start); range.setEnd(last.node, end - last.start)
  return range
}
function codeLines(target) {
  if (!target?.dataset.sourceCodeLine) return null
  const map = textMap(target.querySelector('code') || target), starts = [0]
  for (let offset = 0; (offset = map.text.indexOf('\n', offset)) >= 0;) { starts.push(++offset) }
  const range = line => rangeAt(map, starts[line], Math.min(map.text.length, Math.max(starts[line] + 1, (starts[line + 1] ?? map.text.length) - 1)))
  return { map, starts, range, first: Number(target.dataset.sourceCodeLine) }
}
export function sourceTarget(article, location, query = '', { matchCase = true } = {}) {
  if (!article) return null
  const anchors = [...article.querySelectorAll(selector)]
  const byte = Number.isSafeInteger(location.start)
  const candidates = anchors.filter(node => byte
    ? Number(node.dataset.sourceStart) <= location.start && Number(node.dataset.sourceEnd) > location.start
    : Number(node.dataset.sourceLine) <= location.line && Number(node.dataset.sourceEndLine || node.dataset.sourceLine) >= location.line)
  const target = candidates.sort((a, b) => (Number(a.dataset.sourceEnd) - Number(a.dataset.sourceStart)) - (Number(b.dataset.sourceEnd) - Number(b.dataset.sourceStart)))[0]
    || anchors.find(node => Number(node.dataset.sourceLine) >= location.line) || article
  if (!query) {
    const lines = codeLines(target)
    if (lines) return lines.range(Math.min(lines.starts.length - 1, Math.max(0, location.line - lines.first))) || target
  }
  if (query) {
    const pattern = literalSearchPattern(query, matchCase)
    const lines = codeLines(target)
    if (lines) {
      const line = Math.min(lines.starts.length - 1, Math.max(0, location.line - lines.first))
      const end = lines.starts[line + 1] ?? lines.map.text.length
      pattern.lastIndex = lines.starts[line]
      for (let occurrence = 0; occurrence <= (location.occurrence || 0); occurrence++) {
        const found = pattern.exec(lines.map.text)
        if (!found || found.index + found[0].length > end) break
        if (occurrence === (location.occurrence || 0)) return rangeAt(lines.map, found.index, found.index + found[0].length) || target
      }
    }
    const map = textMap(target)
    // A verbatim token can contain repeated occurrences on one original line.
    if (byte && target.dataset.sourceStart != null && new TextEncoder().encode(map.text).length === Number(target.dataset.sourceEnd) - Number(target.dataset.sourceStart)) {
      const prefix = new TextEncoder().encode(map.text).subarray(0, location.start - Number(target.dataset.sourceStart))
      const exact = new TextDecoder().decode(prefix).length
      const suffix = new TextEncoder().encode(map.text).subarray(0, location.end - Number(target.dataset.sourceStart))
      const exactEnd = new TextDecoder().decode(suffix).length
      pattern.lastIndex = 0
      const found = pattern.exec(map.text.slice(exact, exactEnd))
      if (found?.index === 0 && found[0].length === exactEnd - exact) return rangeAt(map, exact, exactEnd) || target
    }
    pattern.lastIndex = 0
    const found = pattern.exec(map.text)
    if (found) return rangeAt(map, found.index, found.index + found[0].length) || target
  }
  return target
}
export function visibleSourceLocation(article, viewport, fallback = { line: 1 }) {
  if (!article) return fallback
  const top = viewport.getBoundingClientRect().top + 1
  const anchors = article.querySelectorAll(selector)
  let previous = null
  for (const node of anchors) {
    const bounds = node.getBoundingClientRect()
    if (!bounds.height && !bounds.width) continue
    if (bounds.bottom > top && node.dataset.sourceCodeLine) {
      const lines = codeLines(node)
      let first = 0, last = lines.starts.length - 1
      while (first < last) {
        const middle = Math.floor((first + last) / 2), box = lines.range(middle)?.getBoundingClientRect()
        if (box && box.bottom > top) last = middle
        else first = middle + 1
      }
      return { line: lines.first + first }
    }
    if (bounds.bottom > top) return { line: Number(node.dataset.sourceLine) || fallback.line, ...(node.dataset.sourceStart != null ? { start: Number(node.dataset.sourceStart) } : {}) }
    previous = node
  }
  return previous ? { line: Number(previous.dataset.sourceLine) || fallback.line, ...(previous.dataset.sourceStart != null ? { start: Number(previous.dataset.sourceStart) } : {}) } : fallback
}
export function searchHighlights(root) {
  let query = '', current = null, frame = null, matchCase = true, pattern = null
  const supported = Boolean(globalThis.CSS?.highlights && globalThis.Highlight)
  // Older WebKit uses a bounded, pointer-transparent paint layer. It never
  // rewrites projected text, source anchors, native selection or table cells.
  const overlay = supported ? null : document.createElement('div')
  if (overlay) { overlay.className = 'reader-search-paint'; document.body.append(overlay) }
  function refresh() {
    frame = null
    const all = [], selected = []
    if (query && getComputedStyle(root).visibility !== 'hidden') for (const article of root.querySelectorAll('article[data-ready]')) {
      const map = textMap(article)
      pattern.lastIndex = 0
      for (let found; all.length < MAX_MATCHES && (found = pattern.exec(map.text));) {
        const range = rangeAt(map, found.index, found.index + found[0].length)
        if (range) all.push(range)
      }
      if (current && (Number(article.dataset.page) === current.page || current.table != null && Number(article.dataset.table) === current.table)) {
        const target = sourceTarget(article, current, query, { matchCase })
        if (target instanceof Range) selected.push(target)
      }
    }
    if (supported) {
      CSS.highlights.set('reader-search', new Highlight(...all))
      CSS.highlights.set('reader-search-current', new Highlight(...selected))
    } else {
      const fragment = document.createDocumentFragment(), viewport = root.parentElement.getBoundingClientRect()
      let count = 0
      for (const [ranges, active] of [[selected, true], [all, false]]) for (const range of ranges) for (const box of range.getClientRects()) {
        const top = Math.max(box.top, viewport.top), bottom = Math.min(box.bottom, viewport.bottom)
        const left = Math.max(box.left, viewport.left), right = Math.min(box.right, viewport.right)
        if (bottom <= top || right <= left || count >= MAX_MATCHES) continue
        const mark = document.createElement('i'); count++
        mark.className = active ? 'current' : ''
        Object.assign(mark.style, { top: `${top}px`, left: `${left}px`, width: `${right-left}px`, height: `${bottom-top}px` })
        fragment.append(mark)
      }
      overlay.replaceChildren(fragment)
    }
  }
  function schedule() { if (frame == null) frame = requestAnimationFrame(refresh) }
  const observer = new MutationObserver(schedule)
  observer.observe(root, { childList: true, subtree: true })
  if (overlay) { root.parentElement.addEventListener('scroll', schedule, { passive: true, capture: true }); window.addEventListener('resize', schedule) }
  return {
    set(value, match = null, options = {}) { query = value; current = match; matchCase = options.matchCase ?? true; pattern = value ? literalSearchPattern(value, matchCase) : null; refresh() },
    refresh: schedule,
    clear() { query = ''; current = null; pattern = null; refresh() },
    dispose() { observer.disconnect(); overlay?.remove(); root.parentElement.removeEventListener('scroll', schedule, true); window.removeEventListener('resize', schedule); if (frame != null) cancelAnimationFrame(frame); if (supported) { CSS.highlights.delete('reader-search'); CSS.highlights.delete('reader-search-current') } },
  }
}
