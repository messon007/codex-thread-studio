import { pagerIcon } from './icons.mjs'

export function editorController(token, options) {
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
  const message = text => { status.textContent = text }
  const dirty = () => Boolean(code?.dirty)

  function update() {
    const changed = dirty()
    document.getElementById('edit-save').disabled = busy || !changed
    document.getElementById('edit-undo').disabled = busy || !code?.canUndo
    document.getElementById('edit-redo').disabled = busy || !code?.canRedo
    document.getElementById('edit-previous').disabled = busy || !data || data.window.start === 0
    document.getElementById('edit-next').disabled = busy || !data || data.window.end >= data.sourceBytes
    document.getElementById('edit-reload').disabled = busy
    input.disabled = busy
    document.querySelector('#edit-go button').disabled = busy
    document.getElementById('edit-mode').textContent = changed ? 'Edit *' : 'Edit'
    if (data) {
      input.max = data.sourceLines
      document.getElementById('edit-range').textContent = `${data.window.line.toLocaleString()}–${data.window.endLine.toLocaleString()} / ${data.sourceLines.toLocaleString()} lines · ${changed ? 'Unsaved' : 'Saved'}`
    }
    if ((changed || busy) !== lastDirty) {
      lastDirty = changed || busy
      window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:dirty', dirty: lastDirty }))
    }
    options.changed?.({ dirty: changed, busy, visible, savedSinceRead })
  }

  async function api(query = '', payload = null) {
    const response = await fetch(new URL(`./api/edit${query}`, import.meta.url), {
      method: payload ? 'POST' : 'GET', headers: { 'x-reader-token': token, 'x-reader-document': options.documentId(), 'Content-Type': 'application/json' },
      ...(payload ? { body: JSON.stringify(payload) } : {}),
    })
    if (!response.ok) throw Error((await response.text()).slice(0, 1200) || `Editor request failed (${response.status})`)
    return response.json()
  }

  async function load(query) {
    busy = true
    code?.lock(true)
    message('Loading editing page…')
    update()
    try {
      const next = await api(query)
      if (!code) {
        const module = await import('./vendor/editor.mjs')
        code = module.codeView(document.getElementById('editor-mount'), { maxBytes: next.maxPageBytes, save, changed: update, error: message })
      }
      if (query.includes('reload=true')) { savedSinceRead = true; options.saved(next) }
      data = next
      code.load(next.window.text, next.window.line)
      input.value = next.window.line
      message('Changes stay in memory. Save or change pages to write the file. Undo/redo applies to this page.')
      return true
    } catch (error) { message(error.message); return false }
    finally { busy = false; code?.lock(false); update() }
  }

  async function save() {
    if (busy || saving) return false
    if (!dirty()) return true
    if (code.view.composing) { message('Finish the current text composition before saving.'); return false }
    busy = true
    saving = true
    code.lock(true)
    message('Checking the file revision and saving…')
    update()
    try {
      const next = await api('', { revision: data.revision, start: data.window.start, end: data.window.end, changes: code.patches() })
      data = next
      savedSinceRead = true
      options.saved(next)
      if (code.text !== next.window.text) throw Error('File saved, but the editor page differs from the saved result. Keep this page and reopen the file to verify it.')
      code.saved()
      message(next.warning || 'Saved to the original file.')
      return true
    } catch (error) { message(error.message); return false }
    finally { busy = false; saving = false; code.lock(false); update() }
  }

  async function page(previous) {
    if (busy || !data || !await save()) return
    const offset = previous ? data.window.start : data.window.end
    await load(`?offset=${offset}&previous=${previous}`)
  }

  async function go(line, revision = data?.revision) {
    if (busy || !data || !code) return false
    if (revision !== data.revision) { message('The outline changed. Reopen Contents before jumping.'); return false }
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
  previous.addEventListener('click', () => page(true))
  next.addEventListener('click', () => page(false))
  document.getElementById('edit-save').addEventListener('click', save)
  document.getElementById('edit-undo').addEventListener('click', () => code?.undo())
  document.getElementById('edit-redo').addEventListener('click', () => code?.redo())
  document.getElementById('edit-go').addEventListener('submit', async event => {
    event.preventDefault()
    const line = Math.max(1, Math.trunc(Number(input.value)) || 1)
    if (!busy && await save()) await load(`?line=${line}`)
  })
  document.getElementById('edit-reload').addEventListener('click', async () => {
    if (busy || dirty() && !confirm('Discard the unsaved current page and reload the file?')) return
    await load(`?reload=true&line=${code?.cursorLine() || 1}`)
  })
  addEventListener('beforeunload', event => {
    if (!leaving && (dirty() || saving)) { event.preventDefault(); event.returnValue = '' }
  })
  addEventListener('marknoria:close-request', () => {
    const dialog = document.getElementById('edit-close-dialog')
    if (!dialog) return
    if (!dirty() && !busy) window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:close-confirmed' }))
    else if (!dialog.open) dialog.showModal()
  })
  document.getElementById('edit-keep-open').addEventListener('click', () => document.getElementById('edit-close-dialog').close())
  document.getElementById('edit-discard-close').addEventListener('click', () => {
    if (busy) { message('A file operation is still running; wait before closing.'); return }
    window.ipc?.postMessage(JSON.stringify({ type: 'marknoria:close-confirmed' }))
  })
  return {
    save, go, get busy() { return busy }, get dirty() { return dirty() }, get visible() { return visible },
    get needsRefresh() { return savedSinceRead }, get revision() { return data?.revision }, get data() { return data }, get code() { return code },
    refreshed() { savedSinceRead = false; update() },
    leaving() { leaving = true },
    line() { return code?.cursorLine() || data?.window.line || 1 },
    confirmLeave() { return !busy && (!dirty() || confirm('Discard the unsaved editing page? Save first if you want to keep it.')) },
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
    dispose() { code?.dispose() },
  }
}
