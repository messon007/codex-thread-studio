import { uiLabels } from './translation.mjs'
import { pagerIcon } from './icons.mjs'

export function editorController(token, options) {
  const lifetime = new AbortController()
  const labels = uiLabels(options.translate, {signal:lifetime.signal})
  const on = (target, type, listener) => target.addEventListener(type, listener, { signal: lifetime.signal })
  let disposed = false
  let failure = null
  const panel = document.getElementById('edit-view')
  const status = document.getElementById('edit-status')
  const input = document.getElementById('edit-line')
  let code = null
  let data = null
  let visible = false
  let busy = false
  let saving = false
  let savedSinceRead = false
  let lastDirty = false
  let leaving = false
  const message = text => { if (!disposed) { labels.clear(status); status.textContent = text } }
  const uiMessage = key => { if (!disposed) labels.set(status,key) }
  const dirty = () => Boolean(code?.dirty)

  function update() {
    if (disposed) return
    const changed = dirty()
    document.getElementById('edit-save').disabled = busy || !changed
    document.getElementById('edit-undo').disabled = busy || !code?.canUndo
    document.getElementById('edit-redo').disabled = busy || !code?.canRedo
    document.getElementById('edit-previous').disabled = busy || !data || data.window.start === 0
    document.getElementById('edit-next').disabled = busy || !data || data.window.end >= data.sourceBytes
    document.getElementById('edit-reload').disabled = busy
    input.disabled = busy
    document.querySelector('#edit-go button').disabled = busy
    labels.set(document.getElementById('edit-mode'), changed ? 'Edit *' : 'Edit')
    if (data) {
      input.max = data.sourceLines
      labels.set(document.getElementById('edit-range'), changed ? '{start}–{end} / {total} lines · Unsaved' : '{start}–{end} / {total} lines · Saved', {start:data.window.line,end:data.window.endLine,total:data.sourceLines})
    }
    if ((changed || busy) !== lastDirty) {
      lastDirty = changed || busy
      if (options.nativeNotifications !== false) window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:dirty', dirty: lastDirty }))
    }
    options.changed?.({ dirty: changed, busy, visible, savedSinceRead })
  }

  async function api(query = '', payload = null) {
    const response = await fetch(new URL(`./api/edit${query}`, import.meta.url), {
      signal: lifetime.signal,
      method: payload ? 'POST' : 'GET', headers: { 'x-reader-token': token, 'x-reader-document': options.documentId(), 'Content-Type': 'application/json' },
      ...(payload ? { body: JSON.stringify(payload) } : {}),
    })
    if (!response.ok) throw Object.assign(Error((await response.text()).slice(0, 1200) || `Editor request failed (${response.status})`), { code: response.status === 409 ? 'REVISION' : response.status === 403 ? 'CAPABILITY' : response.status === 429 ? 'BUSY' : 'BACKEND' })
    return response.json()
  }

  async function load(query) {
    if (disposed) return false
    failure = null
    busy = true
    code?.lock(true)
    uiMessage('Loading editing page…')
    update()
    try {
      const next = await api(query)
      if (disposed) return false
      if (!code) {
        const module = await import('./vendor/editor.mjs')
        if (disposed) return false
        code = module.codeView(document.getElementById('editor-mount'), { translate: options.translate, maxBytes: next.maxPageBytes, save, changed: update, error: message })
      }
      if (query.includes('reload=true')) { savedSinceRead = true; options.saved(next) }
      data = next
      code.load(next.window.text, next.window.line, next.language)
      input.value = next.window.line
      uiMessage('Changes stay in memory. Save or change pages to write the file. Undo/redo applies to this page.')
      return true
    } catch (error) { if (!disposed) failure = error; message(error.message); return false }
    finally { busy = false; code?.lock(false); update() }
  }

  async function save() {
    if (disposed || busy || saving) return false
    if (!dirty()) return true
    if (code.view.composing) { uiMessage('Finish the current text composition before saving.'); return false }
    failure = null
    busy = true
    saving = true
    code.lock(true)
    uiMessage('Checking the file revision and saving…')
    update()
    try {
      const next = await api('', { revision: data.revision, start: data.window.start, end: data.window.end, changes: code.patches() })
      if (disposed) return false
      data = next
      savedSinceRead = true
      options.saved(next)
      if (code.text !== next.window.text) throw Error('File saved, but the editor page differs from the saved result. Keep this page and reopen the file to verify it.')
      code.saved()
      next.warning ? message(next.warning) : uiMessage('Saved to the original file.')
      return true
    } catch (error) { if (!disposed) failure = error; message(error.message); return false }
    finally { busy = false; saving = false; code.lock(false); update() }
  }

  async function page(previous) {
    if (busy || !data || !await save()) return
    const offset = previous ? data.window.start : data.window.end
    await load(`?offset=${offset}&previous=${previous}`)
  }

  async function go(line, revision = data?.revision) {
    if (busy || !data || !code) return false
    if (revision !== data.revision) { uiMessage('The outline changed. Reopen Contents before jumping.'); return false }
    line = Math.max(1, Math.min(data.sourceLines, Math.trunc(Number(line)) || 1))
    const current = data.window
    if (line >= current.line && line <= current.endLine && !(line === current.line && current.startsMidLine)) {
      await code.reveal(code.mappedLine(line))
      code.focus()
      return true
    }
    const target = line > current.endLine ? line + code.lineDelta : line
    if (!await save()) return false
    if (!await load(`?line=${Math.max(1, target)}`)) return false
    await code.reveal(target)
    code.focus()
    return true
  }

  const previous = document.getElementById('edit-previous')
  const next = document.getElementById('edit-next')
  previous.replaceChildren(pagerIcon('previous'))
  next.replaceChildren(pagerIcon('next'))
  on(previous, 'click', () => page(true))
  on(next, 'click', () => page(false))
  on(document.getElementById('edit-save'), 'click', save)
  on(document.getElementById('edit-undo'), 'click', () => code?.undo())
  on(document.getElementById('edit-redo'), 'click', () => code?.redo())
  on(document.getElementById('edit-go'), 'submit', async event => {
    event.preventDefault()
    const line = Math.max(1, Math.trunc(Number(input.value)) || 1)
    if (!busy && await save()) await load(`?line=${line}`)
  })
  on(document.getElementById('edit-reload'), 'click', async () => {
    if (busy || dirty() && !confirm((options.translate || String)('Discard the unsaved current page and reload the file?'))) return
    await load(`?reload=true&line=${code?.cursorLine() || 1}`)
  })
  on(window, 'beforeunload', event => {
    if (!leaving && (dirty() || saving)) { event.preventDefault(); event.returnValue = '' }
  })
  on(window, 'marknoria:close-request', () => {
    const dialog = document.getElementById('edit-close-dialog')
    if (!dialog) return
    if (!dirty() && !busy) window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:close-confirmed' }))
    else if (!dialog.open) dialog.showModal()
  })
  on(document.getElementById('edit-keep-open'), 'click', () => document.getElementById('edit-close-dialog').close())
  on(document.getElementById('edit-discard-close'), 'click', () => {
    if (busy) { uiMessage('A file operation is still running; wait before closing.'); return }
    window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:close-confirmed' }))
  })
  return {
    save, go, search: () => code?.search(), get error() { return failure }, get busy() { return busy }, get dirty() { return dirty() }, get visible() { return visible },
    get needsRefresh() { return savedSinceRead }, get revision() { return data?.revision }, get data() { return data }, get code() { return code },
    refreshed() { savedSinceRead = false; update() },
    leaving() { leaving = true; lastDirty = false; if (options.nativeNotifications !== false) window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:dirty', dirty: false })) },
    line() { return code?.cursorLine() || data?.window.line || 1 },
    confirmLeave() { return !busy && (!dirty() || confirm((options.translate || String)('Discard the unsaved editing page? Save first if you want to keep it.'))) },
    async show(line = 1) {
      visible = true
      panel.hidden = false
      options.pause()
      if (!data) await load(`?line=${Math.max(1, line)}`)
      else if (!dirty() && (line < data.window.line || line > data.window.endLine)) await load(`?line=${Math.max(1, line)}`)
      if (code && !dirty()) await code.reveal(line)
      code?.focus()
      update()
    },
    hide() { visible = false; panel.hidden = true; update() },
    dispose() {
      if (disposed) return
      if (lastDirty && options.nativeNotifications !== false) window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:dirty', dirty: false }))
      lastDirty = false
      disposed = true; lifetime.abort(); visible = false
      code?.dispose(); code = null; data = null; failure = null
      document.getElementById('edit-close-dialog')?.close()
    },
  }
}
