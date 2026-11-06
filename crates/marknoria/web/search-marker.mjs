// One owned search range. Never wraps source text or changes an EPUB CFI tree.
// Unicode simple case folding, on original text; never lowercase offset maps.
export function literalSearchPattern(query, matchCase = true) {
  if (typeof matchCase !== 'boolean') throw TypeError('matchCase must be boolean')
  return new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), matchCase ? 'gu' : 'giu')
}

export function createSearchMarker(owner) {
  const name = 'marknoria-search-' + crypto.randomUUID().replaceAll('-', '')
  let active = null, style = null, overlay = null, frameRequest = null, registry = null
  const lifetime = new AbortController()
  const clear = () => {
    registry?.delete(name)
    registry = null
    style?.remove(); style = null
    overlay?.remove(); overlay = null
    active = null
    owner.removeAttribute('data-search-current')
    owner.removeAttribute('data-search-highlight')
  }
  const refresh = () => {
    if (!active) return
    const { range, frame, clip } = active
    if (!range.startContainer.isConnected || (frame && !frame.isConnected)) { clear(); return }
    if (registry) return
    const origin = owner.getBoundingClientRect(), bounds = (clip || owner).getBoundingClientRect()
    const frameBounds = frame?.getBoundingClientRect()
    const wrapperBounds = frame?.parentElement?.getBoundingClientRect()
    const rectangles = [...range.getClientRects()]
    if (rectangles.length > 256) { clear(); throw Error('Search highlight exceeds 256 rectangles') }
    overlay.replaceChildren()
    for (const rect of rectangles) {
      const x = (frameBounds?.left || 0) + rect.left, y = (frameBounds?.top || 0) + rect.top
      const left = Math.max(x, bounds.left, wrapperBounds?.left ?? -Infinity)
      const top = Math.max(y, bounds.top, wrapperBounds?.top ?? -Infinity)
      const right = Math.min(x + rect.width, bounds.right, wrapperBounds?.right ?? Infinity)
      const bottom = Math.min(y + rect.height, bounds.bottom, wrapperBounds?.bottom ?? Infinity)
      if (right <= left || bottom <= top) continue
      const mark = owner.ownerDocument.createElement('i')
      mark.dataset.searchCurrentRect = ''
      mark.style.cssText = `position:absolute;left:${left - origin.left}px;top:${top - origin.top}px;width:${right - left}px;height:${bottom - top}px;background:#facc1577;box-shadow:inset 0 0 0 1px #b45309;pointer-events:none`
      overlay.append(mark)
    }
  }
  const schedule = () => {
    if (!active || frameRequest != null) return
    frameRequest = requestAnimationFrame(() => { frameRequest = null; refresh() })
  }
  owner.ownerDocument.addEventListener('scroll', schedule, { capture: true, passive: true, signal: lifetime.signal })
  owner.ownerDocument.defaultView.addEventListener('resize', schedule, { passive: true, signal: lifetime.signal })
  return {
    name,
    clear,
    refresh,
    show(range, { id, frame = null, clip = owner } = {}) {
      clear()
      if (!range || range.collapsed || !range.startContainer.isConnected) throw Error('Search occurrence has no rendered text range')
      if ([...range.getClientRects()].length > 256) throw Error('Search highlight exceeds 256 rectangles')
      active = { range, frame, clip }
      const document = range.startContainer.ownerDocument, view = document.defaultView
      if (view?.CSS?.highlights && typeof view.Highlight === 'function') {
        style = document.createElement('style')
        style.textContent = `::highlight(${name}) { background-color: #facc15; color: #111827; text-decoration: underline #b45309; }`
        document.head.append(style)
        registry = view.CSS.highlights
        registry.set(name, new view.Highlight(range))
      } else {
        overlay = owner.ownerDocument.createElement('div')
        overlay.dataset.searchOverlay = ''
        overlay.setAttribute('aria-hidden', 'true')
        overlay.style.cssText = 'position:absolute;inset:0;pointer-events:none;z-index:4;overflow:hidden'
        owner.append(overlay)
      }
      owner.dataset.searchCurrent = id
      owner.dataset.searchHighlight = name
      refresh()
    },
    dispose() {
      lifetime.abort()
      if (frameRequest != null) cancelAnimationFrame(frameRequest)
      clear()
    }
  }
}
