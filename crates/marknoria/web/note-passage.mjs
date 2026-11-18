import { commentScopeKey, commentTextMap, commentTextHash, commentRangeAt } from './comment-ranges.mjs'

const invalid = (message, code = 'PARAMS') => Object.assign(Error(message), { code })
const integer = value => Number.isSafeInteger(value) && value >= 0
const boundedString = (value, bytes) => typeof value === 'string' && value.length <= bytes && new TextEncoder().encode(value).length <= bytes

export function validatePassage(anchor, index, formatState) {
  const { location, textRange, query } = anchor || {}
  if (query || location?.kind === 'queryCell' || location?.kind === 'query')
    throw invalid('This saved query snapshot cannot be navigated to in the original document', 'UNSUPPORTED_LOCATION')
  if (textRange != null) {
    if (index.renderer || textRange.version !== 1 || !Array.isArray(textRange.segments) || textRange.segments.length < 1 || textRange.segments.length > 32)
      throw invalid('The saved passage has an unsupported text range')
    const seen = new Set()
    for (const segment of textRange.segments) {
      if (!integer(segment?.startUtf16) || !integer(segment?.endUtf16) || segment.endUtf16 <= segment.startUtf16 ||
          segment.endUtf16 > 262144 || typeof segment.textHash !== 'string' || !/^[a-f0-9]{64}$/.test(segment.textHash))
        throw invalid('Invalid saved passage text endpoints or fingerprint')
      const scope = segment.scope
      if (scope?.kind === 'projection') {
        if (!integer(scope.page) || !index.pages[scope.page] || index.pages[scope.page].table != null ||
            location?.kind !== 'line' || !integer(location.line) || location.line < 1 || location.line > index.sourceLines)
          throw invalid('Invalid saved passage projection')
        if (!seen.size && (location.line < index.pages[scope.page].line ||
            location.line > (index.pages[scope.page + 1]?.line ?? index.sourceLines)))
          throw invalid('The saved source line does not belong to the first projection')
      } else if (scope?.kind === 'cell') {
        const table = integer(scope.table) ? index.tables[scope.table] : null
        if (!table || !integer(scope.row) || scope.row >= table.rows || !integer(scope.column) || scope.column >= table.columns.length ||
            location?.kind !== 'cell' || ['table', 'row', 'column'].some(key => location[key] !== scope[key]))
          throw invalid('The saved passage does not match its table cell')
      } else throw invalid('This saved text scope cannot be navigated to', 'UNSUPPORTED_LOCATION')
      const key = commentScopeKey(scope)
      if (seen.has(key)) throw invalid('Duplicate saved passage text scope')
      seen.add(key)
    }
    return 'renderedText'
  }
  if (location?.kind === 'source' && !index.renderer && ['markdown','csv','tsv','text','html'].includes(index.format)) {
    const { startByte, endByte } = location
    if (!integer(startByte) || !integer(endByte) || endByte <= startByte || endByte > index.sourceBytes || endByte - startByte > 128 * 1024)
      throw invalid('Invalid saved source byte range')
    return 'sourceBytes'
  }
  if (location?.kind === 'pdf' && formatState?.format === 'pdf') {
    const { pages } = location
    if (!Array.isArray(pages) || pages.length < 1 || pages.length > 64) throw invalid('Invalid saved PDF pages')
    let rectangles = 0
    const seen = new Set()
    for (const region of pages) {
      if (!integer(region?.page) || region.page < 1 || region.page > formatState.pages || seen.has(region.page) ||
          !Array.isArray(region.rects) || (rectangles += region.rects.length) > 256)
        throw invalid('Invalid saved PDF page or rectangle budget')
      seen.add(region.page)
      for (const rect of region.rects) {
        if (![rect?.x, rect?.y, rect?.width, rect?.height].every(Number.isFinite) ||
            rect.x < 0 || rect.y < 0 || rect.width <= 0 || rect.height <= 0 ||
            rect.x + rect.width > 1.001 || rect.y + rect.height > 1.001)
          throw invalid('Invalid saved PDF rectangle')
      }
    }
    return pages[0].rects.length ? 'pdfRegion' : 'pdfPage'
  }
  if (location?.kind === 'epub' && formatState?.format === 'epub') {
    if (!boundedString(location.cfi, 8192) || !/^epubcfi\([^\r\n]+\)$/u.test(location.cfi) ||
        location.href != null && !boundedString(location.href, 8192))
      throw invalid('Invalid saved EPUB CFI')
    return 'epubCfi'
  }
  throw invalid('This saved reference has no supported exact passage or format location', 'UNSUPPORTED_LOCATION')
}

// Resolve only the mounted, revision-owned scope at the start of the passage.
// Repeated quotes never participate in locating a saved occurrence.
export async function resolveNotePassage(segment, entries, current) {
  const key = commentScopeKey(segment.scope)
  if (entries.length > 512) throw Error('Note navigation exceeds its rendered scope budget')
  for (const entry of entries) {
    current()
    if (!entry.node?.isConnected || commentScopeKey(entry.scope) !== key) continue
    const map = commentTextMap(entry.node), hash = await commentTextHash(entry.node, map.text)
    current()
    const live = entry.node.isConnected ? commentTextMap(entry.node) : null
    if (live?.text !== map.text)
      throw new DOMException('The passage changed while navigating', 'AbortError')
    if (hash !== segment.textHash) continue
    const range = commentRangeAt(live, segment.startUtf16, segment.endUtf16)
    if (!range) throw invalid('The saved passage has invalid text boundaries')
    return { node: entry.node, range }
  }
  return null
}

function selectRange(range) {
  // Selection owns its own endpoints. Passing the saved Range to addRange()
  // would let selection changes also mutate the object used for the handoff.
  range.startContainer.ownerDocument.getSelection()?.setBaseAndExtent(
    range.startContainer, range.startOffset, range.endContainer, range.endOffset,
  )
}

function afterFocusFrame(window) {
  return new Promise(resolve => {
    const done = () => {
      window.cancelAnimationFrame(frame)
      window.clearTimeout(timer)
      resolve()
    }
    let frame = window.requestAnimationFrame(() => { frame = window.requestAnimationFrame(done) })
    const timer = window.setTimeout(done, 50)
  })
}

export async function focusNotePassage({ node, range }, viewport, isCurrent) {
  const document = node.ownerDocument
  const input = new AbortController()
  const live = () => isCurrent() && !input.signal.aborted && node.isConnected &&
    !range.collapsed && node.contains(range.commonAncestorContainer)
  if (!live()) return false
  const interrupt = { capture: true, once: true, signal: input.signal }
  for (const event of ['pointerdown', 'keydown']) document.addEventListener(event, () => input.abort(), interrupt)
  const selected = () => {
    const selection = document.getSelection()
    const activeRange = selection?.rangeCount === 1 ? selection.getRangeAt(0) : null
    return activeRange && !activeRange.collapsed &&
      activeRange.compareBoundaryPoints(Range.START_TO_START, range) === 0 &&
      activeRange.compareBoundaryPoints(Range.END_TO_END, range) === 0
  }
  try {
    viewport.focus({ preventScroll: true })
    selectRange(range)
    await afterFocusFrame(document.defaultView)
    if (!live()) return false
    if (selected()) return true
    // A delayed focus update may collapse the selection. Restore once while
    // this navigation still owns it. Leave a newer nonempty selection alone.
    if (!document.getSelection()?.isCollapsed) return false
    selectRange(range)
    await afterFocusFrame(document.defaultView)
    if (!live()) return false
    if (!selected()) throw invalid('Could not retain the saved passage selection during focus handoff', 'PASSAGE_SELECTION')
    return true
  } finally {
    input.abort()
  }
}

export function revealNotePassage({ node, range }, viewport) {
  if (!node.isConnected) throw Error('The saved passage is no longer mounted')
  const start = range.startContainer.nodeType === 3 ? range.startContainer.parentElement : range.startContainer
  if (!viewport.contains(start)) throw Error('The saved passage is outside its reading viewport')
  // A long multi-line range can have a bounding box wider than its scroller.
  // Measure only its first bounded text fragment, then retain the complete
  // exact saved selection. Its first line rectangle fits short saved phrases.
  const point = range.cloneRange()
  if (range.startContainer.nodeType === 3) {
    const text = range.startContainer.data
    let end = Math.min(range.startOffset + 256, text.length, range.endContainer === range.startContainer ? range.endOffset : text.length)
    if (end < text.length && /[\uD800-\uDBFF]/.test(text[end - 1]) && /[\uDC00-\uDFFF]/.test(text[end])) end--
    point.setEnd(range.startContainer, end)
  }
  const ancestors = []
  for (let target = start; target; target = target.parentElement) {
    if (ancestors.length === 64) throw Error('Saved passage exceeds its scroll-container budget')
    ancestors.push(target)
    if (target === viewport) break
  }
  if (ancestors.at(-1) !== viewport) throw Error('The saved passage is outside its reading viewport')
  for (const target of ancestors) {
    const style = node.ownerDocument.defaultView.getComputedStyle(target)
    const horizontal = target === viewport || /^(auto|scroll|hidden|overlay)$/.test(style.overflowX)
    const vertical = target === viewport || /^(auto|scroll|hidden|overlay)$/.test(style.overflowY)
    if (!horizontal && !vertical) continue
    const rect = point.getClientRects()[0] || point.getBoundingClientRect(), bounds = target.getBoundingClientRect()
    if (!rect.width && !rect.height) throw Error('The saved passage is not visible in this view')
    const left = bounds.left + target.clientLeft, top = bounds.top + target.clientTop
    if (horizontal && target.scrollWidth > target.clientWidth) {
      if (rect.left < left) target.scrollLeft += rect.left - left
      else if (rect.right > left + target.clientWidth) target.scrollLeft += Math.min(rect.right - left - target.clientWidth, rect.left - left)
    }
    if (vertical && target.scrollHeight > target.clientHeight) {
      if (target === viewport) target.scrollTop += rect.top - top - Math.min(48, target.clientHeight / 4)
      else if (rect.top < top) target.scrollTop += rect.top - top
      else if (rect.bottom > top + target.clientHeight) target.scrollTop += rect.bottom - top - target.clientHeight
    }
  }
  selectRange(range)
}
