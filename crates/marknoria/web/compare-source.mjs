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
  const schedule = () => { if (!stopped && frame == null) frame = requestAnimationFrame(render) }
  const virtualizer = new Virtualizer({ count: metadata.rows, getScrollElement: () => viewport, estimateSize: () => 28, observeElementRect, observeElementOffset, scrollToFn: elementScroll, overscan: 5, onChange: schedule })
  const stop = virtualizer._didMount()
  virtualizer._willUpdate()

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
      const button = document.createElement('button')
      button.type = 'button'
      button.className = `compare-cell ${side}${entry && !cell ? ' empty' : ''}`
      button.disabled = !cell
      for (const [name, text] of [
        ['number', cell?.line ?? ''],
        ['sign', cell && entry.kind !== 'equal' ? side === 'left' ? '−' : '+' : ''],
        ['text', cell ? cell.text.replace(/\r?\n$/u, '') + (cell.truncated ? ' … [open complete line]' : '') : entry ? '' : 'Loading…'],
        ['ending', cell?.ending || ''],
      ]) {
        const span = document.createElement('span')
        span.className = `compare-${name}`
        span.textContent = text
        button.append(span)
      }
      if (cell) {
        button.setAttribute('aria-label', `${side} line ${cell.line}. Open complete source line`)
        button.addEventListener('click', () => openLine(item.index, side))
      }
      row.append(button)
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
      space.replaceChildren()
    },
  }
}
