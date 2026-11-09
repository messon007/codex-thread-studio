import { Virtualizer, observeElementRect, observeElementOffset, elementScroll } from './vendor/virtual-core.mjs'

export function sourcePane(viewport, side, metadata, api, openLine) {
  const document = viewport.ownerDocument
  const space = viewport.firstElementChild
  const cache = new Map()
  let bytes = 0
  let request = null
  let stopped = false
  let frame = null
  let activeRow = null
  let hint = 1
  let failedRow = null
  let rowHeight = 28
  const schedule = () => { if (!stopped && frame == null) frame = requestAnimationFrame(render) }
  const virtualizer = new Virtualizer({ count: metadata.rows, getScrollElement: () => viewport, estimateSize: () => rowHeight, observeElementRect, observeElementOffset, scrollToFn: elementScroll, overscan: 5, onChange: schedule })
  const stop = virtualizer._didMount()
  virtualizer._willUpdate()
  const probe = document.createElement('span')
  probe.style.cssText = 'position:absolute;top:0;left:0;visibility:hidden;pointer-events:none;width:0;height:max(28px,calc(var(--code-font-size,12px) * 1.7))'
  probe.setAttribute('aria-hidden', 'true')
  viewport.append(probe)
  const resize = new viewport.ownerDocument.defaultView.ResizeObserver(() => {
    const height = probe.getBoundingClientRect().height
    if (stopped || height < 1 || Math.abs(height - rowHeight) < 0.1) return
    const position = viewport.scrollTop / rowHeight
    rowHeight = height
    virtualizer.measure()
    space.style.height = `${virtualizer.getTotalSize()}px`
    viewport.scrollTop = position * rowHeight
    schedule()
  })
  resize.observe(probe)

  async function load(row) {
    const controller = new AbortController()
    request = controller
    try {
      const result = await api(`/api/compare/window?row=${row}`, { signal: controller.signal })
      if (stopped) return
      for (const entry of result.rows) {
        if (cache.has(entry.row)) continue
        delete entry[side === 'left' ? 'right' : 'left']
        entry.bytes = (entry[side]?.text.length || 0) * 2 + 192
        cache.set(entry.row, entry)
        bytes += entry.bytes
      }
      while (bytes > 2 * 1048576 || cache.size > 1000) {
        const [key, entry] = cache.entries().next().value
        cache.delete(key)
        bytes -= entry.bytes
      }
    } catch (error) {
      if (error.name !== 'AbortError') { failedRow = row; viewport.dataset.error = error.message }
    } finally { if (request === controller) { request = null; schedule() } }
  }

  function render() {
    frame = null
    if (stopped || viewport.hidden) return
    const fragment = document.createDocumentFragment()
    let missing = null
    for (const item of virtualizer.getVirtualItems()) {
      const entry = cache.get(item.index)
      const cell = entry?.[side]
      const row = document.createElement('div')
      row.className = `compare-row ${entry?.kind || 'loading'}${activeRow === item.index ? ' active' : ''}`
      row.style.transform = `translateY(${item.start}px)`
      row.dataset.row = item.index
      if (!entry && missing == null) missing = item.index
      const element = document.createElement('div')
      element.className = `compare-cell ${side}${entry && !cell ? ' empty' : ''}`
      for (const [name, text] of [
        ['number', cell?.line ?? ''],
        ['sign', cell && entry.kind !== 'equal' ? side === 'left' ? '−' : '+' : ''],
        ['text', cell ? cell.text.replace(/\r?\n$/u, '') : entry ? '' : 'Loading…'],
        ['ending', cell?.ending || ''],
      ]) {
        const span = document.createElement('span')
        span.className = `compare-${name}`
        span.textContent = text
        element.append(span)
      }
      if (cell?.truncated) {
        const open = document.createElement('button')
        open.type = 'button'
        open.className = 'compare-open-line'
        open.textContent = '…'
        open.setAttribute('aria-label', `Open complete ${side} source line ${cell.line}`)
        open.title = 'Open complete source line'
        open.addEventListener('click', () => openLine(item.index, side))
        element.append(open)
      }
      row.append(element)
      fragment.append(row)
    }
    space.style.height = `${virtualizer.getTotalSize()}px`
    space.replaceChildren(fragment)
    if (missing != null && !request && missing !== failedRow) void load(missing)
  }

  const retry = () => { failedRow = null; delete viewport.dataset.error; schedule() }
  viewport.addEventListener('wheel', retry, { passive: true })
  viewport.addEventListener('keydown', retry)
  return {
    cache, virtualizer, viewport,
    selection() {
      if (stopped || viewport.hidden) return null
      const selected = document.getSelection(), range = selected?.rangeCount ? selected.getRangeAt(0) : null
      if (!range || range.collapsed) return null
      const spanFor = node => (node.nodeType === 1 ? node : node.parentElement)?.closest('.compare-text')
      const start = spanFor(range.startContainer), end = spanFor(range.endContainer)
      if (!start || !end || !viewport.contains(start) || !viewport.contains(end)) return null
      const offset = (node, value, span) => node === span.firstChild ? value : node === span ? (value ? span.textContent.length : 0) : null
      const from = offset(range.startContainer, range.startOffset, start), to = offset(range.endContainer, range.endOffset, end)
      if (from == null || to == null) return null
      const spans = [...space.querySelectorAll('.compare-text')]
      if (spans.length > 512) return null
      const parts = []
      let line = null
      for (const span of spans) {
        if (!range.intersectsNode(span)) continue
        const cell = cache.get(Number(span.closest('.compare-row').dataset.row))?.[side]
        if (!cell || !span.firstChild) continue
        const length = cell.text.replace(/\r?\n$/u, '').length
        // Diff truncation labels, gutters and +/- decorations are not source.
        if (span === start && from > length || span === end && to > length || cell.truncated && span !== end) return null
        const local = document.createRange()
        local.setStart(span.firstChild, span === start ? from : 0)
        local.setEnd(span.firstChild, span === end ? to : length)
        parts.push(local.toString())
        line ??= cell.line
      }
      const quote = parts.join('\n')
      if (!quote.trim() || line == null) return null
      const bounds = range.getBoundingClientRect()
      return { quote, quoteTruncated: false, precision: 'approximate', location: { kind: 'line', line },
        viewportRect: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } }
    },
    get visible() { return !viewport.hidden },
    get bytes() { return bytes },
    line() {
      for (const item of virtualizer.getVirtualItems()) {
        if (item.end <= viewport.scrollTop) continue
        const line = cache.get(item.index)?.[side]?.line
        if (line) return line
      }
      return hint
    },
    show(row = 0, line = 1, active = false) {
      hint = line
      activeRow = active ? row : null
      viewport.hidden = false
      space.style.height = `${virtualizer.getTotalSize()}px`
      virtualizer._willUpdate()
      virtualizer.scrollToIndex(Math.max(0, row - (active ? 3 : 0)), { align: 'start', behavior: 'auto' })
      retry()
    },
    dispose() {
      stopped = true
      request?.abort()
      if (frame != null) cancelAnimationFrame(frame)
      viewport.removeEventListener('wheel', retry)
      viewport.removeEventListener('keydown', retry)
      stop()
      resize.disconnect(); probe.remove()
      space.replaceChildren()
    },
  }
}
