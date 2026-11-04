import { dialogClose } from './icons.mjs'
// jq execution and result storage live in Rust; this module owns the dialog.
export function jqController(index, api) {
  if (!/\.(json|jsonl|ndjson)$/i.test(index.name)) return { dispose() {} }
  const button = document.createElement('button'); button.type = 'button'; button.className = 'reader-action'; button.textContent = 'jq'; button.title = 'Query JSON with jq'
  document.querySelector('.reader-document-tools').append(button)
  button.setAttribute('aria-label', 'jq')
  const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); icon.setAttribute('viewBox', '0 0 24 24'); icon.setAttribute('aria-hidden', 'true')
  const path = document.createElementNS(icon.namespaceURI, 'path'); path.setAttribute('d', 'M8 4H6v5l-3 3 3 3v5h2 M16 4h2v5l3 3-3 3v5h-2'); icon.append(path)
  const label = document.createElement('span'); label.textContent = 'jq'; button.replaceChildren(icon, label)
  let disposeDialog = null
  button.addEventListener('click', () => {
    disposeDialog?.()
    const dialog = document.createElement('dialog'); dialog.className = 'table-query-dialog jq-query-dialog'; dialog.setAttribute('aria-label', 'Query JSON with jq')
    const shell = document.createElement('section'); shell.className = 'reader-shell jq-query-shell'
    const heading = document.createElement('header'); heading.className = 'table-query-heading'
    const title = document.createElement('h2'); title.textContent = 'Query JSON with jq'
    const close = dialogClose(() => dialog.close()); heading.append(title, close)
    const editor = document.createElement('textarea'); editor.className = 'table-query-editor'; editor.setAttribute('aria-label', 'jq filter'); editor.value = '.'; editor.maxLength = 16384; editor.spellcheck = false
    const help = document.createElement('p'); help.className = 'jq-help'; help.textContent = 'JSON: .items[] | select(.active) · JSONL: select(.level == "error") · Slurp: map(.value) | add'
    const actions = document.createElement('div'); actions.className = 'table-query-actions'
    const run = document.createElement('button'); run.textContent = 'Run filter'; run.className = 'jq-run'
    const reset = document.createElement('button'); reset.textContent = 'Reset filter'
    const slurpLabel = document.createElement('label'), slurp = document.createElement('input'); slurp.type = 'checkbox'; slurpLabel.append(slurp, ' Slurp input into an array')
    const status = document.createElement('span'); status.role = 'status'; actions.append(run, reset, slurpLabel, status)
    const result = document.createElement('pre'); result.className = 'jq-results'; result.tabIndex = 0; result.setAttribute('aria-label', 'jq results'); result.textContent = 'Run a filter to view formatted results.'
    const footer = document.createElement('footer'); footer.className = 'table-query-actions'
    const previous = document.createElement('button'); previous.textContent = 'Previous'; previous.disabled = true
    const position = document.createElement('span'); position.textContent = 'No results'
    const next = document.createElement('button'); next.textContent = 'Next'; next.disabled = true
    const copy = document.createElement('button'); copy.textContent = 'Copy result page'; copy.disabled = true
    footer.append(previous, position, next, copy); shell.append(heading, editor, help, actions, result, footer); dialog.append(shell); document.body.append(dialog)
    let current = null, stopped = false, busy = false, request = null
    const release = id => api('/api/jq-query', undefined, { method: 'POST', body: JSON.stringify({ id, revision: index.revision, close: true }) }).catch(() => {})
    const update = () => { run.disabled = reset.disabled = busy; previous.disabled = busy || !current || current.page === 0; next.disabled = busy || !current || current.page + 1 >= current.pages; copy.disabled = busy || !current; editor.setAttribute('aria-busy', String(busy)) }
    async function load(page, filter) {
      if (busy || stopped) return
      busy = true; update(); status.textContent = 'Evaluating…'; request = new AbortController()
      try {
        const value = await api('/api/jq-query', request.signal, { method: 'POST', body: JSON.stringify({ revision: index.revision, ...(filter == null ? { id: current.id, page } : { filter, slurp: slurp.checked }) }) })
        if (stopped) { release(value.id); return }
        if (value.revision !== index.revision) { release(value.id); throw Error('Document revision changed; reopen jq') }
        const old = current?.id; current = value; result.textContent = value.text || 'No matching values'; result.scrollTop = 0
        position.textContent = `Page ${value.page + 1} / ${value.pages} · ${value.totalValues.toLocaleString()} values`; status.textContent = `Read-only results · ${value.elapsedMs.toFixed(0)} ms`
        if (old && old !== value.id) release(old)
      } catch (error) { if (!stopped && error.name !== 'AbortError') status.textContent = error.message }
      finally { busy = false; update() }
    }
    run.addEventListener('click', () => load(0, editor.value)); reset.addEventListener('click', () => { editor.value = '.'; slurp.checked = false; load(0, '.') })
    previous.addEventListener('click', () => load(current.page - 1)); next.addEventListener('click', () => load(current.page + 1))
    copy.addEventListener('click', async () => { try { await navigator.clipboard.writeText(current.text); status.textContent = 'Copied result page' } catch { status.textContent = 'Clipboard unavailable' } })
    editor.addEventListener('keydown', event => { if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); run.click() } })
    disposeDialog = () => { if (stopped) return; stopped = true; request?.abort(); if (current) release(current.id); dialog.remove() }
    dialog.addEventListener('close', disposeDialog, { once: true }); dialog.showModal(); editor.focus()
    if (index.sourceBytes <= 1024 * 1024) load(0, '.')
    else status.textContent = 'Enter a filter to narrow the results, then Run filter.'
  })
  return { dispose() { disposeDialog?.(); button.remove() } }
}
