export function createLargeMarkdownHost(frame, onState, onSelection = () => {}) {
  let disposed = false
  let timer = null
  let signature = ''
  let request = null
  let searchGeneration = 0
  let observer = null
  const document = () => frame.contentDocument
  let selectionDocument = null
  const captureSelection = () => {
    const selection = frame.contentWindow?.getSelection()
    if (!selection?.rangeCount || selection.isCollapsed) return
    const quote = String(selection).trim().slice(0, 16000)
    if (!quote) return
    const range = selection.getRangeAt(0)
    const element = node => node?.nodeType === 1 ? node : node?.parentElement
    const start = element(range.startContainer)
    const end = element(range.endContainer)
    if (!start?.closest('#viewport, #source-view') || start.closest('dialog')) return
    const index = frame.contentWindow?.readerExperiment?.index
    const line = node => Number(node?.closest('[data-source-line]')?.dataset.sourceLine)
      || index?.pages[Number(node?.closest('[data-page]')?.dataset.page)]?.line
      || null
    const startLine = line(start)
    const endPage = end?.closest('[data-page]')
    const endLine = Number(end?.closest('[data-source-end-line]')?.dataset.sourceEndLine)
      || (endPage ? (index?.pages[Number(endPage.dataset.page) + 1]?.line || index?.sourceLines) : line(end))
    const rect = range.getBoundingClientRect(), outer = frame.getBoundingClientRect()
    onSelection({ quote, startLine, endLine: startLine ? Math.max(startLine, endLine || startLine) : null,
      revision: index?.revision || '', rect: { left: outer.left + rect.left, right: outer.left + rect.right,
        top: outer.top + rect.top, bottom: outer.top + rect.bottom, width: rect.width, height: rect.height } })
  }
  const bindSelection = () => {
    const next = document()
    if (!next || next === selectionDocument) return
    selectionDocument?.removeEventListener?.('mouseup', captureSelection)
    selectionDocument?.removeEventListener?.('keyup', captureSelection)
    selectionDocument = next
    next.addEventListener?.('mouseup', captureSelection)
    next.addEventListener?.('keyup', captureSelection)
  }
  frame.addEventListener?.('load', bindSelection)
  bindSelection()
  const bridge = () => frame.contentWindow?.marknoriaHost
  const snapshot = () => bridge()?.state || { ready: false }
  const endpoint = path => new URL(`./api/${path}`, new URL(frame.src))
  const headers = () => ({ 'x-reader-token': new URL(frame.src).hash.slice(1), ...(snapshot().documentId ? { 'x-reader-document': snapshot().documentId } : {}) })
  const publish = () => {
    if (disposed) return
    clearTimeout(timer)
    const next = snapshot()
    const key = JSON.stringify(next)
    if (key !== signature) { signature = key; onState(next) }
    if (bridge() && !observer) {
      observer = new MutationObserver(publish)
      for (const selector of ['#view-modes', '#contents-toggle', '#edit-range', '#edit-status', '#document-summary', '#welcome-title']) {
        const target = document()?.querySelector(selector)
        if (target) observer.observe(target, { attributes: true, childList: true, subtree: true, attributeFilter: ['aria-pressed', 'aria-expanded', 'disabled', 'hidden'] })
      }
    }
    if (!bridge()) timer = setTimeout(publish, 80)
  }
  const heartbeat = setInterval(() => { if (!disposed && bridge()) fetch(endpoint('renderer'), { headers: headers() }).catch(() => {}) }, 30_000)
  publish()
  return {
    get busy() { return Boolean(snapshot().busy) },
    get dirty() { return Boolean(snapshot().dirty) },
    leave() { bridge()?.leave() },
    async setView(mode) {
      if (!bridge() || disposed) return
      await bridge().setView(mode)
      publish()
    },
    async save() {
      if (!bridge() || disposed) return false
      const saved = await bridge().save()
      publish()
      if (!saved) throw Error(snapshot().status || 'Unable to save the editing page')
      return true
    },
    setOutline(open) {
      if (!bridge() || disposed) return
      const button = document().querySelector('#contents-toggle')
      if ((button.getAttribute('aria-expanded') === 'true') !== open) button.click()
    },
    async search(query) {
      request?.abort()
      const generation = ++searchGeneration
      if (!bridge() || !query.trim() || disposed) return []
      request = new AbortController()
      const response = await fetch(endpoint(`search?query=${encodeURIComponent(query)}`), { signal: request.signal, headers: headers() })
      if (!response.ok) throw new Error(`Reader request failed (${response.status})`)
      const result = await response.json()
      return generation === searchGeneration && !disposed ? result.matches : []
    },
    navigateMatch(match) {
      if (!bridge() || disposed) return
      return bridge().navigate(match)
    },
    dispose() { frame.removeEventListener?.('load', bindSelection); selectionDocument?.removeEventListener?.('mouseup', captureSelection); selectionDocument?.removeEventListener?.('keyup', captureSelection); disposed = true; bridge()?.leave(); clearTimeout(timer); clearInterval(heartbeat); observer?.disconnect(); request?.abort() },
  }
}
