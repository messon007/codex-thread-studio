import { pagerIcon } from './icons.mjs'

export function sourceController(index, api, preview, anchor, pausePreview, options = {}) {
  const listeners = new AbortController()
  const on = (element, type, handler) => element.addEventListener(type, handler, { signal: listeners.signal })
  const panel = document.querySelector('#source-view')
  const viewport = document.querySelector('#source-viewport')
  const text = document.querySelector('#source-text')
  const numbers = document.querySelector('#source-numbers')
  const input = document.querySelector('#source-line')
  const range = document.querySelector('#source-range')
  const previous = document.querySelector('#source-previous')
  const next = document.querySelector('#source-next')
  previous.replaceChildren(pagerIcon('previous'))
  next.replaceChildren(pagerIcon('next'))
  let page = null
  let request = null
  let generation = 0
  let visible = false
  let loading = false
  let external = null
  let code = null
  const mount = document.createElement('div')
  mount.id = 'source-code-view'
  viewport.append(mount)
  input.max = index.sourceLines
  const lineHeight = () => parseFloat(getComputedStyle(text).lineHeight)
  const currentLine = () => external ? external.line() : code ? code.topLine() : page ? Math.min(index.sourceLines, page.line + Math.floor(viewport.scrollTop / lineHeight())) : 1
  const update = () => {
    previous.setAttribute('aria-disabled', String(loading || !page || page.start === 0))
    next.setAttribute('aria-disabled', String(loading || !page || page.end >= index.sourceBytes))
    input.readOnly = loading
  }
  async function load(query, target = null) {
    request?.abort()
    request = new AbortController()
    const ticket = ++generation
    loading = true
    panel.dataset.ready = ''
    text.setAttribute('aria-busy', 'true')
    update()
    range.textContent = 'Loading source…'
    try {
      const response = await api(`/api/source?${query}`, request.signal)
      if (ticket !== generation || !visible || response.revision !== index.revision) return
      page = response.result
      const module = await import('./vendor/editor.mjs')
      if (ticket !== generation || !visible) return
      if (!code) code = module.codeView(mount, { readOnly: true })
      code.load(page.text.replaceAll('\r\n', '\n'), page.line)
      document.querySelector('.source-content').hidden = true
      text.textContent = page.text
      const count = page.text.split('\n').length
      numbers.textContent = Array.from({ length: count }, (_, row) => `${page.line + row}${row === 0 && page.startsMidLine ? '↳' : ''}`).join('\n')
      input.value = target ?? page.line
      const continued = page.startsMidLine || page.endsMidLine ? ' · continued line' : ''
      range.textContent = `${page.line.toLocaleString()}–${page.endLine.toLocaleString()} / ${index.sourceLines.toLocaleString()} lines${continued}`
      viewport.scrollTop = Math.max(0, (target ?? page.line) - page.line) * lineHeight()
      viewport.scrollLeft = 0
      await code.reveal(target ?? page.line)
      panel.dataset.ready = 'true'
    } catch (error) {
      if (ticket === generation && error.name !== 'AbortError') range.textContent = `${error.message} · Go to retry`
    } finally {
      if (ticket === generation) { loading = false; text.removeAttribute('aria-busy'); update() }
    }
  }
  async function go(line) {
    line = Math.max(1, Math.min(index.sourceLines, Math.trunc(Number(line)) || 1))
    if (external) return external.go(line)
    await load(`line=${line}`, line)
  }
  function setVisible(open) {
    if (open) pausePreview()
    visible = open
    panel.hidden = !open || Boolean(external)
    if (external) external.viewport.hidden = !open
    const previewViewport = document.querySelector('#viewport')
    previewViewport.style.visibility = open ? 'hidden' : ''
    previewViewport.inert = open
    previewViewport.setAttribute('aria-hidden', String(open))
    document.querySelector('#preview-mode').setAttribute('aria-pressed', String(!open))
    document.querySelector('#source-mode').setAttribute('aria-pressed', String(open))
    document.querySelector('.reader-settings').hidden = open
    document.querySelector('.reader-identity p').textContent = open ? 'Markdown · Read-only source' : 'Markdown · Read-only preview'
    if (!open) {
      const line = currentLine()
      generation += 1
      request?.abort()
      loading = false
      text.removeAttribute('aria-busy')
      preview(line)
    }
  }
  async function show(line = anchor()) {
    setVisible(true)
    await go(line)
  }
  if (options.buttons !== false) {
    on(document.querySelector('#source-mode'), 'click', () => { if (!visible) show() })
    on(document.querySelector('#preview-mode'), 'click', () => { if (visible) setVisible(false) })
  }
  on(document.querySelector('#source-navigation'), 'submit', event => { event.preventDefault(); go(input.value) })
  on(previous, 'click', () => { if (!loading && page?.start > 0) load(`offset=${page.start}&previous=true`) })
  on(next, 'click', () => { if (!loading && page && page.end < index.sourceBytes) load(`offset=${page.end}`) })
  return {
    get visible() { return visible }, get page() { return page }, currentLine, show, go,
    hide() { if (visible) setVisible(false) },
    attach(externalView) {
      generation += 1
      request?.abort()
      loading = false
      text.removeAttribute('aria-busy')
      if (external) external.viewport.hidden = true
      external = externalView
      panel.hidden = !visible || Boolean(external)
      if (external) external.viewport.hidden = !visible
    },
    dispose() {
      generation += 1; request?.abort(); listeners.abort(); code?.dispose(); mount.remove()
      panel.hidden = true
      const previewViewport = document.querySelector('#viewport')
      previewViewport.style.visibility = ''
      previewViewport.inert = false
      previewViewport.setAttribute('aria-hidden', 'false')
    },
  }
}
