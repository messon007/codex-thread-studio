export function createLargeMarkdownHost(frame, onState) {
  let disposed = false
  let timer = null
  let signature = ''
  let request = null
  let searchGeneration = 0
  let observer = null
  const document = () => frame.contentDocument
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
    dispose() { disposed = true; bridge()?.leave(); clearTimeout(timer); clearInterval(heartbeat); observer?.disconnect(); request?.abort() },
  }
}
