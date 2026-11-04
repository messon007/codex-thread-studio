// Geometry and input only: Rust still supplies the bounded row/column projection.
const MIN = 48, MAX = 8192
const clamp = width => Math.min(MAX, Math.max(MIN, Math.round(width)))

export function tableLayout(article, content, state, readWidths, saveWidths, changed, context = {}) {
  // The virtualizer constructs articles before attaching them to #space.
  const viewport = context.viewport || document.querySelector('#viewport')
  const shell = context.shell || document.querySelector('.reader-shell')
  const workspace = context.workspace || document.querySelector('#workspace')
  const signal = new AbortController()
  const track = document.createElement('div')
  track.className = 'table-scrollbar'
  track.hidden = true
  track.tabIndex = 0
  track.role = 'scrollbar'
  track.setAttribute('aria-label', 'Table horizontal scroll')
  track.setAttribute('aria-orientation', 'horizontal')
  track.setAttribute('aria-valuemin', '0')
  content.id = `table-content-${article.dataset.table}`
  track.setAttribute('aria-controls', content.id)
  const thumb = document.createElement('div')
  thumb.className = 'table-scrollbar-thumb'
  track.append(thumb)
  // A transformed virtual article offsets CSS sticky positioning. Keep the
  // control in the stable workspace and place it after virtualizer layout.
  workspace.append(track)
  let table, cols = [], selected = [], natural = [], handles = [], manual = readWidths()
  let drag = null, thumbDrag = null, layoutKey = '', stopped = false
  const options = { signal: signal.signal }
  // WebKit does not expose a layout box for <col>; measure real header cells.
  const widths = () => [...table.rows[0].cells].map(cell => cell.getBoundingClientRect().width)
  const key = () => `${content.clientWidth}:${getComputedStyle(shell).getPropertyValue('--column-width')}:${shell.classList.contains('no-wrap')}`
  const rows = () => [...(table?.tBodies[0]?.rows || [])]
  const anchor = y => rows().find(row => row.getBoundingClientRect().bottom > y) || null
  function keepRow(row, update) {
    const top = row?.getBoundingClientRect().top
    update()
    changed(article)
    if (row?.isConnected) viewport.scrollTop += row.getBoundingClientRect().top - top
  }
  function position() {
    if (!table || stopped || !article.isConnected || viewport.getAttribute('aria-hidden') === 'true') { track.hidden = true; return }
    const contentBounds = content.getBoundingClientRect(), viewBounds = viewport.getBoundingClientRect(), parentBounds = workspace.getBoundingClientRect()
    const top = Math.min(contentBounds.bottom - 1, viewBounds.bottom) - 18
    const left = Math.max(contentBounds.left + 1, viewBounds.left)
    const right = Math.min(contentBounds.right - 1, viewBounds.right)
    track.hidden = content.scrollWidth - content.clientWidth < 1 || top < Math.max(contentBounds.top, viewBounds.top) || contentBounds.top >= viewBounds.bottom || right <= left
    if (track.hidden) return
    track.style.left = `${left - parentBounds.left}px`
    track.style.top = `${top - parentBounds.top}px`
    track.style.width = `${right - left}px`
  }
  function sync() {
    if (!table || stopped) return
    const maximum = Math.max(0, content.scrollWidth - content.clientWidth)
    state.horizontal = content.scrollLeft
    content.classList.toggle('has-horizontal-overflow', maximum >= 1)
    position()
    track.setAttribute('aria-valuemax', String(Math.round(maximum)))
    track.setAttribute('aria-valuenow', String(Math.round(content.scrollLeft)))
    if (track.hidden) return
    const size = Math.min(track.clientWidth, Math.max(32, track.clientWidth * content.clientWidth / content.scrollWidth))
    thumb.style.width = `${size}px`
    thumb.style.transform = `translateX(${(track.clientWidth - size) * content.scrollLeft / maximum}px)`
  }
  function apply() {
    if (!table || !content.clientWidth) return
    const limit = Math.max(80, Number.parseFloat(getComputedStyle(shell).getPropertyValue('--column-width')) || 180)
    const sizes = natural.map(width => Math.max(64, Math.min(limit, width)))
    if (!manual.size) {
      const flexible = natural.flatMap((width, i) => width > limit ? [i] : [])
      const spare = Math.max(0, content.clientWidth - sizes.reduce((a, b) => a + b, 0))
      for (const i of flexible) sizes[i] += spare / flexible.length
    }
    for (let i = 0; i < cols.length; i++) {
      const width = manual.get(selected[i]) ?? sizes[i]
      cols[i].style.width = `${width}px`
      sizes[i] = width
    }
    table.style.width = `${sizes.reduce((a, b) => a + b, 0)}px`
    const height = table.offsetHeight
    let edge = 0
    for (let i = 0; i < handles.length; i++) {
      edge += sizes[i]
      handles[i].style.left = `${edge - (i + 1 === handles.length ? 8 : 4)}px`
      handles[i].style.height = `${height}px`
      handles[i].setAttribute('aria-valuenow', String(Math.round(sizes[i])))
    }
    layoutKey = key()
    sync()
  }
  function freeze() {
    widths().forEach((width, i) => manual.set(selected[i], clamp(width)))
  }
  function remember() {
    // Keep hidden original columns too, but bound each remembered table.
    while (manual.size > 256) {
      const hidden = [...manual.keys()].find(id => !selected.includes(id))
      manual.delete(hidden ?? manual.keys().next().value)
    }
    saveWidths(new Map(manual))
  }
  function resize(column, width, row) {
    keepRow(row, () => {
      freeze()
      manual.set(selected[column], clamp(width))
      apply()
      remember()
    })
  }
  function fit(column, row) {
    // Measure at most one projected column. Clone formatting, not source HTML.
    const measure = document.createElement('div')
    measure.className = 'table-measurement'
    const sample = document.createElement('table')
    sample.style.cssText = 'width:max-content;table-layout:auto'
    for (const source of table.rows) {
      const cell = source.cells[column]?.cloneNode(true)
      if (!cell) continue
      cell.style.cssText = 'width:auto;min-width:0;max-width:none;white-space:pre;overflow:visible'
      sample.insertRow().append(cell)
    }
    measure.append(sample)
    article.append(measure)
    try { resize(column, sample.getBoundingClientRect().width, row) }
    finally { measure.remove() }
  }
  const columnAt = event => {
    const handle = event.target.closest('.table-column-resize')
    return handle && content.contains(handle) ? Number(handle.dataset.column) : null
  }
  content.addEventListener('pointerdown', event => {
    const column = columnAt(event)
    if (column == null || event.button !== 0 || !table) return
    event.preventDefault()
    drag = { column, x: event.clientX, horizontal: content.scrollLeft, width: widths()[column], row: anchor(event.clientY) }
    content.setPointerCapture(event.pointerId)
    article.classList.add('table-resizing')
  }, options)
  content.addEventListener('pointermove', event => {
    if (!drag) return
    const dx = event.clientX - drag.x
    if (Math.abs(dx) >= 1) resize(drag.column, drag.width + dx + content.scrollLeft - drag.horizontal, drag.row)
  }, options)
  const endDrag = () => { drag = null; article.classList.remove('table-resizing') }
  content.addEventListener('lostpointercapture', endDrag, options)
  content.addEventListener('pointerup', event => {
    if (content.hasPointerCapture(event.pointerId)) content.releasePointerCapture(event.pointerId)
    endDrag()
  }, options)
  content.addEventListener('dblclick', event => {
    const column = columnAt(event)
    if (column == null || !table) return
    event.preventDefault()
    fit(column, anchor(event.clientY))
  }, options)
  content.addEventListener('keydown', event => {
    const column = columnAt(event)
    if (column == null || !table) return
    if (['ArrowLeft', 'ArrowRight', 'Home'].includes(event.key)) {
      event.preventDefault()
      const row = anchor(viewport.getBoundingClientRect().top)
      if (event.key === 'Home') fit(column, row)
      else resize(column, widths()[column] + (event.key === 'ArrowLeft' ? -16 : 16), row)
    }
  }, options)
  content.addEventListener('scroll', sync, { ...options, passive: true })
  const wheel = event => {
    if (!event.shiftKey && !(event.currentTarget === track && event.deltaX)) return
    if (content.scrollWidth <= content.clientWidth) return
    const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? content.clientWidth : 1
    content.scrollLeft += (event.deltaX || event.deltaY) * unit
    event.preventDefault()
  }
  content.addEventListener('wheel', wheel, { ...options, passive: false })
  track.addEventListener('wheel', wheel, { ...options, passive: false })
  track.addEventListener('pointerdown', event => {
    if (event.button !== 0) return
    event.preventDefault()
    track.focus({ preventScroll: true })
    if (event.target === thumb) {
      thumbDrag = { x: event.clientX, left: content.scrollLeft }
      track.setPointerCapture(event.pointerId)
    } else {
      content.scrollLeft += content.clientWidth * (event.clientX < thumb.getBoundingClientRect().left ? -1 : 1)
    }
  }, options)
  track.addEventListener('pointermove', event => {
    if (!thumbDrag) return
    const travel = track.clientWidth - thumb.offsetWidth
    if (travel > 0) content.scrollLeft = thumbDrag.left + (event.clientX - thumbDrag.x) * (content.scrollWidth - content.clientWidth) / travel
  }, options)
  track.addEventListener('pointerup', event => {
    thumbDrag = null
    if (track.hasPointerCapture(event.pointerId)) track.releasePointerCapture(event.pointerId)
  }, options)
  track.addEventListener('lostpointercapture', () => { thumbDrag = null }, options)
  track.addEventListener('keydown', event => {
    const maximum = content.scrollWidth - content.clientWidth
    const targets = { ArrowLeft: content.scrollLeft - 40, ArrowRight: content.scrollLeft + 40, PageUp: content.scrollLeft - content.clientWidth, PageDown: content.scrollLeft + content.clientWidth, Home: 0, End: maximum }
    if (event.key in targets) { event.preventDefault(); content.scrollLeft = targets[event.key] }
  }, options)
  const observer = new ResizeObserver(() => {
    if (stopped || !article.isConnected) return
    if (layoutKey !== key() && !drag) apply()
    sync()
  })
  observer.observe(content)
  observer.observe(viewport)
  const visibility = new MutationObserver(sync)
  visibility.observe(viewport, { attributes: true, attributeFilter: ['style', 'aria-hidden'] })
  viewport.addEventListener('scroll', sync, { ...options, passive: true })
  return {
    update() {
      table = content.querySelector('table')
      if (!table) return
      // Column requests are asynchronous; the old cells remain usable while
      // loading. Their widths must keep the IDs of the displayed projection.
      selected = [...state.columns]
      handles.forEach(handle => handle.remove())
      const group = document.createElement('colgroup')
      cols = selected.map(() => group.appendChild(document.createElement('col')))
      table.prepend(group)
      const canvas = document.createElement('canvas').getContext('2d')
      const font = getComputedStyle(table.querySelector('td') || table.querySelector('th')).font
      canvas.font = font
      natural = selected.map((_, column) => Math.max(64, ...[...table.rows].map(row => {
        const cell = row.cells[column]
        return Math.min(MAX, Math.max(...cell.textContent.split('\n').map(line => canvas.measureText(line).width)) + 25)
      })))
      handles = selected.map((original, column) => {
        const handle = document.createElement('div')
        handle.className = 'table-column-resize'
        handle.dataset.column = column
        handle.dataset.originalColumn = original
        handle.role = 'separator'
        handle.tabIndex = 0
        handle.setAttribute('aria-label', `Resize column ${original + 1}`)
        handle.setAttribute('aria-orientation', 'vertical')
        handle.setAttribute('aria-valuemin', String(MIN))
        handle.setAttribute('aria-valuemax', String(MAX))
        handle.title = 'Drag to resize column · Double-click to fit contents'
        content.append(handle)
        return handle
      })
      const horizontal = state.horizontal || 0
      apply()
      content.scrollLeft = horizontal
      sync()
    },
    reset() {
      keepRow(anchor(viewport.getBoundingClientRect().top), () => {
        manual.clear()
        saveWidths(new Map())
        apply()
      })
    },
    position: sync,
    refresh() { if (table && layoutKey !== key() && !drag) apply(); sync() },
    dispose() { stopped = true; observer.disconnect(); visibility.disconnect(); signal.abort(); endDrag(); track.remove() },
  }
}
