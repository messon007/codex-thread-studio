import { createPdfOutline } from './pdf-outline.mjs'
// DOM/layout adapter around pinned PDF.js. See third-party/README.md.
import { uiLabels } from './translation.mjs'
import { getDocument, GlobalWorkerOptions, TextLayer } from './vendor/pdf/pdf.mjs'

GlobalWorkerOptions.workerSrc = new URL('./vendor/pdf/pdf.worker.mjs', import.meta.url).href
const MAX_PAGES = 2000,
  MAX_PIXELS = 8 * 1024 * 1024,
  MAX_TEXT = 512 * 1024
const clamp = (value) => Math.max(0, Math.min(1, value))
const aborted = () => new DOMException('PDF reader disposed', 'AbortError')
const pause = () => new Promise((resolve) => setTimeout(resolve, 0))

export function mountPdfReader(root, options) {
  const { documentId, revision, resourceId, onSelection = () => {}, onState = () => {} } = options
  if (!documentId || !revision || !resourceId)
    throw Error('PDF requires its owning document, revision and resource identity')
  const viewport = options.viewport || root.parentElement
  const lifetime = new AbortController(),
    signal = lifetime.signal,
    mounted = new Map(),
    pending = new Set()
  let labels, pageLabels
  try {
    labels = uiLabels(options.translate, { signal })
    pageLabels = [uiLabels(options.translate, { signal }), uiLabels(options.translate, { signal })]
  } catch (error) {
    lifetime.abort()
    throw error
  }
  const sheet = document.createElement('section')
  sheet.className = 'marknoria-pdf'
  const controls = document.createElement('div')
  controls.className = 'pdf-controls'
  controls.setAttribute('role', 'toolbar')
  labels.set(controls, 'PDF reading controls', {}, 'aria-label')
  const pagesRoot = document.createElement('div')
  pagesRoot.className = 'pdf-pages'
  const status = document.createElement('span')
  status.className = 'pdf-status'
  status.setAttribute('role', 'status')
  const css = document.createElement('link')
  css.rel = 'stylesheet'
  css.href = new URL('./pdf-reader.css', import.meta.url)
  const textCss = document.createElement('link')
  textCss.rel = 'stylesheet'
  textCss.href = new URL('./vendor/pdf/pdf-viewer.css', import.meta.url)
  sheet.append(css, textCss, controls, pagesRoot)
  root.append(sheet)
  let pdf,
    loading,
    current = 1,
    zoom = 'width',
    continuous = true,
    disposed = false,
    drawing = false,
    generation = 0,
    firstContentMs = null
  let pageWidth = 612,
    pageHeight = 792,
    selection = null,
    readyState = false,
    searchTicket = 0,
    lastError = null,
    disposal
  const pages = []
  const outlines = createPdfOutline({ document: () => pdf, ensure: () => ensure(), bounded: (...args) => bounded(...args), pause: () => pause() })
  const started = performance.now()
  const state = () => ({
    format: 'pdf',
    page: current,
    pages: pdf?.numPages ?? 0,
    zoom,
    continuous,
    ready: readyState,
    busy: drawing || !readyState,
    error: lastError,
    firstUsefulContentMs: firstContentMs,
    mountedPages: mounted.size,
    position: {
      version: 1,
      resourceId,
      revision,
      view: 'preview',
      location: {
        kind: 'pdf',
        pages: [{ page: current, rects: [{ x: 0, y: topFraction(), width: 1, height: 0.000001 }] }]
      }
    }
  })
  function emit() {
    if (!disposed) {
      previous.disabled = current <= 1
      next.disabled = current >= (pdf?.numPages || 1)
      pageInput.value = String(current)
      labels.set(pageTotal, '/ {pages}', { pages: pdf?.numPages ?? '…' })
      onState(state())
    }
  }
  function button(label, action) {
    const value = document.createElement('button')
    value.type = 'button'
    labels.set(value, label)
    value.addEventListener(
      'click',
      () => {
        try {
          Promise.resolve(action()).catch(fail)
        } catch (error) {
          fail(error)
        }
      },
      { signal }
    )
    controls.append(value)
    return value
  }
  const previous = button('Previous', () => go(current - 1)),
    next = button('Next', () => go(current + 1))
  const pageInput = document.createElement('input')
  pageInput.type = 'number'
  pageInput.min = '1'
  pageInput.value = '1'
  labels.set(pageInput, 'PDF page', {}, 'aria-label')
  pageInput.addEventListener('change', () => go(Number(pageInput.value)).catch(fail), { signal })
  const pageTotal = document.createElement('span')
  controls.append(pageInput, pageTotal)
  button('Fit width', () => apply({ zoom: 'width' }))
  button('−', () =>
    apply({
      zoom: Math.max(0.25, (typeof zoom === 'number' ? zoom : effectiveScale() / (4 / 3)) - 0.25)
    })
  )
  button('+', () =>
    apply({
      zoom: Math.min(4, (typeof zoom === 'number' ? zoom : effectiveScale() / (4 / 3)) + 0.25)
    })
  )
  button('100%', () => apply({ zoom: 1 }))
  const flow = document.createElement('label'),
    flowLabel = document.createElement('span'),
    checkFlow = document.createElement('input')
  checkFlow.type = 'checkbox'
  checkFlow.checked = true
  labels.set(flowLabel, 'Continuous reading')
  flow.append(checkFlow, flowLabel)
  controls.append(flow, status)
  checkFlow.addEventListener('change', () => apply({ continuous: checkFlow.checked }).catch(fail), {
    signal
  })
  function fail(error) {
    if (disposed || error?.name === 'AbortError') return
    lastError = String(error.message || error).slice(0, 800)
    labels.clear(status)
    status.textContent = lastError
    emit()
  }
  function ensure() {
    if (disposed) throw aborted()
  }
  async function bounded(promise, label, milliseconds = 20000) {
    ensure()
    let timer, stop
    try {
      return await Promise.race([
        promise,
        new Promise((_, reject) => {
          stop = () => reject(aborted())
          signal.addEventListener('abort', stop, { once: true })
          timer = setTimeout(() => {
            const error = Error(`${label} exceeded ${milliseconds / 1000} seconds`)
            fail(error)
            dispose()
            reject(error)
          }, milliseconds)
        })
      ])
    } finally {
      clearTimeout(timer)
      signal.removeEventListener('abort', stop)
    }
  }
  function effectiveScale(width = pageWidth) {
    return zoom === 'width'
      ? Math.min(4, Math.max(0.1, (viewport.clientWidth - 32) / width))
      : zoom * (4 / 3)
  }
  function dimensions(node, width = pageWidth, height = pageHeight) {
    const scale = effectiveScale(width)
    node.style.width = `${width * scale}px`
    node.style.height = `${height * scale}px`
    node.style.setProperty('--total-scale-factor', scale)
    node.style.setProperty('--scale-factor', scale)
    node.style.setProperty('--scale-round-x', '1px')
    node.style.setProperty('--scale-round-y', '1px')
    return scale
  }
  function topFraction() {
    const node = pages[current - 1]
    if (!node || !continuous) return 0
    const bounds = node.getBoundingClientRect(),
      top = viewport.getBoundingClientRect().top + controls.getBoundingClientRect().height
    return Math.min(0.999999, clamp((top - bounds.top) / bounds.height))
  }
  function release(number) {
    const entry = mounted.get(number)
    if (!entry) return
    mounted.delete(number)
    entry.cancelled = true
    entry.task?.cancel()
    entry.layer?.cancel()
    if (entry.layerNode) labels.clear(entry.layerNode, 'aria-label')
    for (const canvas of entry.node.querySelectorAll('canvas')) {
      canvas.width = canvas.height = 0
    }
    entry.node.replaceChildren()
    entry.node.dataset.state = 'pending'
    entry.page?.cleanup()
  }
  function prune() {
    const ordered = [...mounted.keys()].sort(
      (a, b) => Math.abs(a - current) - Math.abs(b - current)
    )
    for (const number of ordered.slice(3)) release(number)
  }
  async function text(page) {
    const content = await bounded(page.getTextContent(), 'PDF text extraction')
    if (
      content.items.length > 5000 ||
      content.items.reduce((sum, item) => sum + (item.str?.length || 0), 0) > MAX_TEXT
    )
      throw Error('PDF page exceeds the text-layer budget')
    return content
  }
  async function paint(number) {
    if (mounted.has(number) || !pages[number - 1] || disposed) return
    const node = pages[number - 1],
      entry = { node, cancelled: false, page: null, task: null, layer: null },
      ticket = generation
    mounted.set(number, entry)
    prune()
    const live = () =>
      !disposed && !entry.cancelled && ticket === generation && mounted.get(number) === entry
    try {
      const page = (entry.page = await bounded(pdf.getPage(number), 'PDF page loading'))
      if (!live()) return
      const base = page.getViewport({ scale: 1 }),
        scale = dimensions(node, base.width, base.height),
        display = page.getViewport({ scale })
      node.dataset.pdfWidth = base.width
      node.dataset.pdfHeight = base.height
      const canvas = document.createElement('canvas'),
        pixels = Math.min(
          devicePixelRatio || 1,
          2,
          Math.sqrt(MAX_PIXELS / (display.width * display.height))
        )
      canvas.width = Math.max(1, Math.floor(display.width * pixels))
      canvas.height = Math.max(1, Math.floor(display.height * pixels))
      canvas.style.width = `${display.width}px`
      canvas.style.height = `${display.height}px`
      canvas.setAttribute('aria-hidden', 'true')
      const layerNode = document.createElement('div')
      layerNode.className = 'textLayer'
      labels.set(layerNode, 'Page {page} text', { page: number }, 'aria-label')
      entry.layerNode = layerNode
      node.replaceChildren(canvas, layerNode)
      entry.task = page.render({
        canvasContext: canvas.getContext('2d'),
        viewport: display,
        transform: [pixels, 0, 0, pixels, 0, 0]
      })
      await bounded(entry.task.promise, 'PDF page rendering')
      if (!live()) return
      entry.layer = new TextLayer({
        textContentSource: await text(page),
        container: layerNode,
        viewport: display
      })
      await bounded(entry.layer.render(), 'PDF text-layer layout')
      if (!live()) return
      node.dataset.state = 'ready'
      firstContentMs ??= performance.now() - started
      status.textContent = ''
      emit()
    } catch (error) {
      if (live()) {
        node.dataset.state = 'error'
        node.textContent = String(error.message || error).slice(0, 800)
        fail(error)
      }
    }
  }
  async function drain() {
    if (drawing || !pdf || disposed) return
    drawing = true
    emit()
    try {
      while (pending.size && !disposed) {
        const number = [...pending].sort((a, b) => Math.abs(a - current) - Math.abs(b - current))[0]
        pending.delete(number)
        if (Math.abs(number - current) > 1 && continuous) continue
        await paint(number)
        await pause()
      }
    } finally {
      drawing = false
      emit()
    }
  }
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries)
        if (entry.isIntersecting && !entry.target.hidden)
          pending.add(Number(entry.target.dataset.pdfPage))
      void drain()
    },
    { root: viewport, rootMargin: '400px 0px' }
  )
  async function go(number, fraction = 0) {
    ensure()
    await ready
    if (!Number.isSafeInteger(number) || number < 1 || number > pdf.numPages)
      throw Error('PDF page out of range')
    current = number
    selection = null
    if (!continuous) for (const [i, node] of pages.entries()) node.hidden = i + 1 !== current
    pending.add(current)
    if (continuous && current < pdf.numPages) pending.add(current + 1)
    void drain()
    emit()
    const node = pages[current - 1],
      relative = node.getBoundingClientRect().top - viewport.getBoundingClientRect().top
    viewport.scrollTo({
      top:
        viewport.scrollTop +
        relative -
        controls.getBoundingClientRect().height +
        fraction * node.clientHeight,
      behavior: 'auto'
    })
  }
  async function apply(patch) {
    ensure()
    if (
      patch.zoom != null &&
      patch.zoom !== 'width' &&
      (!Number.isFinite(patch.zoom) || patch.zoom < 0.25 || patch.zoom > 4)
    )
      throw Error('PDF zoom must be fit width or 25–400%')
    if (patch.continuous != null && typeof patch.continuous !== 'boolean')
      throw Error('Invalid PDF reading flow')
    const fraction = topFraction()
    zoom = patch.zoom ?? zoom
    continuous = patch.continuous ?? continuous
    checkFlow.checked = continuous
    generation++
    pending.clear()
    for (const number of [...mounted.keys()]) release(number)
    for (const [i, node] of pages.entries()) {
      node.hidden = !continuous && i + 1 !== current
      dimensions(
        node,
        Number(node.dataset.pdfWidth) || pageWidth,
        Number(node.dataset.pdfHeight) || pageHeight
      )
    }
    if (pdf) await go(current, fraction)
    emit()
  }
  let scrollFrame = null
  viewport.addEventListener(
    'scroll',
    () => {
      if (scrollFrame != null || !continuous || !readyState) return
      scrollFrame = requestAnimationFrame(() => {
        scrollFrame = null
        const top = viewport.getBoundingClientRect().top + controls.clientHeight
        const node = pages.find((node) => node.getBoundingClientRect().bottom > top + 2)
        if (node) current = Number(node.dataset.pdfPage)
        pending.add(current)
        prune()
        void drain()
        emit()
      })
    },
    { passive: true, signal }
  )
  const resize = new ResizeObserver(() => {
    if (zoom === 'width' && readyState && !disposed) void apply({}).catch(fail)
  })
  resize.observe(viewport)
  function normalized(rect, bounds) {
    const x = clamp((rect.left - bounds.left) / bounds.width),
      y = clamp((rect.top - bounds.top) / bounds.height)
    const right = clamp((rect.right - bounds.left) / bounds.width),
      bottom = clamp((rect.bottom - bounds.top) / bounds.height)
    return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null
  }
  function captured() {
    const value = getSelection()
    if (!value?.rangeCount || value.isCollapsed) return selection
    const range = value.getRangeAt(0),
      regions = [],
      quotes = [],
      rectangles = []
    for (const [number, entry] of [...mounted].sort(([a], [b]) => a - b)) {
      const layer = entry.node.querySelector('.textLayer')
      if (!layer || !range.intersectsNode(layer)) continue
      // Physical triple-click selections can end just outside the text layer.
      // Clamp to this layer so no host/UI text or canvas geometry enters a quote.
      const local = range.cloneRange(),
        contents = document.createRange()
      contents.selectNodeContents(layer)
      if (local.compareBoundaryPoints(Range.START_TO_START, contents) < 0)
        local.setStart(contents.startContainer, contents.startOffset)
      if (local.compareBoundaryPoints(Range.END_TO_END, contents) > 0)
        local.setEnd(contents.endContainer, contents.endOffset)
      const bounds = entry.node.getBoundingClientRect(),
        selected = [...local.getClientRects()],
        rects = selected.map((rect) => normalized(rect, bounds)).filter(Boolean)
      if (rects.length) {
        regions.push({ page: number, rects })
        quotes.push(local.toString())
        rectangles.push(...selected)
      }
    }
    if (!regions.length) return null
    if (regions.reduce((sum, region) => sum + region.rects.length, 0) > 256)
      throw Error('PDF selection exceeds 256 rectangles; select a smaller passage')
    regions.sort((a, b) => a.page - b.page)
    const text = quotes.join('\n'),
      end = Math.min(text.length, 16384),
      quote = text.slice(0, end && /[\uD800-\uDBFF]/.test(text[end - 1]) ? end - 1 : end)
    const bounds = {
      x: Math.min(...rectangles.map((r) => r.x)),
      y: Math.min(...rectangles.map((r) => r.y)),
      right: Math.max(...rectangles.map((r) => r.right)),
      bottom: Math.max(...rectangles.map((r) => r.bottom))
    }
    bounds.width = bounds.right - bounds.x
    bounds.height = bounds.bottom - bounds.y
    return {
      documentId,
      revision,
      quote,
      quoteTruncated: quote.length < text.length,
      precision: 'exact',
      location: { kind: 'pdf', pages: regions },
      viewportRect: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }
    }
  }
  document.addEventListener(
    'selectionchange',
    () => {
      if (!disposed) {
        try {
          onSelection(captured())
        } catch (error) {
          fail(error)
        }
      }
    },
    { signal }
  )
  let regionStart = null,
    regionBox = null
  pagesRoot.addEventListener(
    'pointerdown',
    (event) => {
      const node = event.target.closest('[data-pdf-page]')
      if (!node || !event.shiftKey || event.button !== 0) return
      event.preventDefault()
      node.setPointerCapture(event.pointerId)
      regionStart = { node, x: event.clientX, y: event.clientY }
      selection = null
      regionBox?.remove()
      regionBox = document.createElement('div')
      regionBox.className = 'pdf-region-box'
      node.append(regionBox)
    },
    { signal }
  )
  pagesRoot.addEventListener(
    'pointermove',
    (event) => {
      if (!regionStart || !regionBox) return
      const bounds = regionStart.node.getBoundingClientRect(),
        x = Math.max(bounds.left, Math.min(bounds.right, event.clientX)),
        y = Math.max(bounds.top, Math.min(bounds.bottom, event.clientY))
      Object.assign(regionBox.style, {
        left: `${Math.min(regionStart.x, x) - bounds.left}px`,
        top: `${Math.min(regionStart.y, y) - bounds.top}px`,
        width: `${Math.abs(x - regionStart.x)}px`,
        height: `${Math.abs(y - regionStart.y)}px`
      })
    },
    { signal }
  )
  pagesRoot.addEventListener(
    'pointercancel',
    () => {
      regionStart = null
      regionBox?.remove()
      regionBox = null
    },
    { signal }
  )
  pagesRoot.addEventListener(
    'pointerup',
    (event) => {
      if (!regionStart) return
      const start = regionStart
      regionStart = null
      const bounds = {
          left: Math.min(start.x, event.clientX),
          top: Math.min(start.y, event.clientY),
          right: Math.max(start.x, event.clientX),
          bottom: Math.max(start.y, event.clientY)
        },
        rect = normalized(bounds, start.node.getBoundingClientRect())
      if (!rect) {
        regionBox?.remove()
        regionBox = null
        return
      }
      selection = {
        documentId,
        revision,
        quote: '',
        quoteTruncated: false,
        precision: 'exact',
        location: {
          kind: 'pdf',
          pages: [{ page: Number(start.node.dataset.pdfPage), rects: [rect] }]
        },
        viewportRect: {
          x: bounds.left,
          y: bounds.top,
          width: bounds.right - bounds.left,
          height: bounds.bottom - bounds.top
        }
      }
      getSelection()?.removeAllRanges()
      onSelection(selection)
    },
    { signal }
  )
  viewport.addEventListener(
    'keydown',
    (event) => {
      if (
        event.target.closest('input,textarea,button,[contenteditable]') ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey
      )
        return
      if (!continuous && ['PageDown', 'ArrowRight', 'PageUp', 'ArrowLeft'].includes(event.key)) {
        event.preventDefault()
        go(current + (['PageDown', 'ArrowRight'].includes(event.key) ? 1 : -1)).catch(fail)
      }
    },
    { signal }
  )
  function dispose() {
    if (disposed) return disposal
    disposed = true
    lifetime.abort()
    searchTicket++
    generation++
    pending.clear()
    observer.disconnect()
    resize.disconnect()
    if (scrollFrame != null) cancelAnimationFrame(scrollFrame)
    for (const number of [...mounted.keys()]) release(number)
    disposal = Promise.resolve(loading?.destroy())
    void disposal.catch(() => {})
    pdf = null
    regionStart = null
    regionBox = null
    selection = null
    readyState = false
    pages.length = 0
    outlines.dispose()
    sheet.replaceChildren()
    sheet.remove()
    return disposal
  }
  const ready = (async () => {
    if (options.data && options.data.byteLength > 128 * 1024 * 1024)
      throw Error('PDF exceeds 128 MiB')
    const base = new URL('./vendor/pdf/', import.meta.url)
    loading = getDocument({
      ...(options.data
        ? { data: options.data.slice() }
        : { url: options.url, httpHeaders: options.headers }),
      isEvalSupported: false,
      enableXfa: false,
      disableAutoFetch: true,
      disableStream: true,
      rangeChunkSize: 256 * 1024,
      cMapUrl: new URL('cmaps/', base).href,
      cMapPacked: true,
      standardFontDataUrl: new URL('standard_fonts/', base).href,
      wasmUrl: new URL('wasm/', base).href,
      maxImageSize: MAX_PIXELS,
      canvasMaxAreaInBytes: MAX_PIXELS * 4,
      stopAtErrors: true
    })
    pdf = await bounded(loading.promise, 'PDF loading', 60000)
    ensure()
    if (!pdf.numPages || pdf.numPages > MAX_PAGES) throw Error('PDF exceeds the 2,000-page budget')
    const first = await bounded(pdf.getPage(1), 'PDF first page'),
      bounds = first.getViewport({ scale: 1 })
    pageWidth = bounds.width
    pageHeight = bounds.height
    for (let number = 1; number <= pdf.numPages; number++) {
      const node = document.createElement('div')
      node.className = 'pdf-page'
      node.dataset.pdfPage = String(number)
      pageLabels[Math.floor((number - 1) / 1000)].set(
        node,
        'Page {page}',
        { page: number },
        'aria-label'
      )
      node.tabIndex = 0
      dimensions(node)
      pages.push(node)
      pagesRoot.append(node)
      observer.observe(node)
    }
    pageInput.max = String(pdf.numPages)
    readyState = true
    pending.add(1)
    void drain()
    emit()
    return state()
  })().catch((error) => {
    fail(error)
    dispose()
    throw error
  })
  void ready.catch(() => {})
  return {
    ready,
    state,
    selection: captured,
    settings: apply,
    dispose,
    async navigate(location) {
      if (location?.kind !== 'pdf' || !Array.isArray(location.pages) || !location.pages.length)
        throw Error('A PDF page location is required')
      const target = location.pages[0],
        fraction = target.rects?.[0]?.y ?? 0
      if (!Number.isFinite(fraction) || fraction < 0 || fraction > 1)
        throw Error('Invalid PDF page coordinate')
      await go(target.page, fraction)
    },
    async search(query, { signal: requestSignal } = {}) {
      await ready
      const ticket = ++searchTicket
      if (typeof query !== 'string' || !query || new TextEncoder().encode(query).length > 256)
        throw Error('Search requires 1–256 UTF-8 bytes')
      const matches = [],
        needle = query.toLocaleLowerCase()
      let complete = true
      for (let number = 1; number <= pdf.numPages; number++) {
        ensure()
        requestSignal?.throwIfAborted()
        if (ticket !== searchTicket) throw aborted()
        const page = await bounded(pdf.getPage(number), 'PDF search page'),
          content = await text(page),
          value = content.items.map((item) => item.str || '').join(' ')
        const offset = value.toLocaleLowerCase().indexOf(needle)
        if (offset >= 0)
          matches.push({
            page: number,
            excerpt: value.slice(
              Math.max(0, offset - 100),
              offset + Math.min(query.length, 256) + 100
            ),
            location: {
              kind: 'pdf',
              pages: [{ page: number, rects: [{ x: 0, y: 0, width: 1, height: 1 }] }]
            }
          })
        if (!mounted.has(number)) page.cleanup()
        if (matches.length >= 500) {
          complete = number === pdf.numPages
          break
        }
        await pause()
      }
      return { revision, matches, complete, limit: 500 }
    },
    async navigateOutline(id, { signal } = {}) {
      await ready
      const location = await outlines.location(id, signal)
      if (!location) throw Error('This outline group has no destination')
      return this.navigate(location)
    },
    async outline(options = {}) {
      await ready
      return { revision, ...await outlines.page(options) }
    }
  }
}
