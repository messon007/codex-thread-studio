import { dialogClose } from './icons.mjs'
import { tableLayout } from './table-layout.mjs'
import { tableControls } from './table-controls.mjs'

export function tableQuery(table, index, api) {
  const dialog = document.createElement('dialog')
  dialog.className = 'table-query-dialog'
  dialog.setAttribute('aria-label', 'Query table with SQL')
  const shell = document.createElement('section')
  shell.className = 'reader-shell table-query-shell'
  const parentShell = document.querySelector('.reader-shell')
  shell.classList.toggle('no-wrap', parentShell.classList.contains('no-wrap'))
  shell.style.setProperty('--column-width', getComputedStyle(parentShell).getPropertyValue('--column-width'))
  const header = document.createElement('header')
  header.className = 'table-query-heading'
  const title = document.createElement('h2'); title.textContent = 'Query table with SQL'
  const close = dialogClose(() => dialog.close())
  header.append(title, close)
  const editor = document.createElement('textarea')
  editor.className = 'table-query-editor'; editor.maxLength = 16384
  editor.setAttribute('aria-label', 'SQL query'); editor.spellcheck = false
  const schema = document.createElement('details'); schema.className = 'table-query-schema'
  const summary = document.createElement('summary'); summary.textContent = 'Columns and query help'
  const help = document.createElement('pre'); schema.append(summary, help)
  const actions = document.createElement('div'); actions.className = 'table-query-actions'
  const run = document.createElement('button'); run.textContent = 'Run query'; run.className = 'table-query-run'
  const reset = document.createElement('button'); reset.textContent = 'Reset query'
  const message = document.createElement('span'); message.setAttribute('role', 'status'); actions.append(run, reset, message)
  const controlsSlot = document.createElement('div'); controlsSlot.className = 'table-query-controls'
  const workspace = document.createElement('div'); workspace.className = 'table-query-workspace'
  const viewport = document.createElement('div'); viewport.className = 'table-query-viewport'; viewport.tabIndex = 0
  const article = document.createElement('article'); article.className = 'page'; article.dataset.table = `query-${table}`
  const content = document.createElement('div'); content.className = 'table-content'; content.textContent = 'Preparing table…'
  article.append(content); viewport.append(article); workspace.append(viewport)
  const results = document.createElement('section'); results.className = 'table-query-result'; results.append(controlsSlot,workspace)
  shell.append(header, editor, schema, actions, results); dialog.append(shell); document.body.append(dialog)
  let request, ticket = 0, loading = false, metadata, current, layout, tools, names = [], widths = new Map(), stopped = false
  const state = { columns: [], horizontal: 0 }
  const release = id => api('/api/table-query/close', undefined, { method: 'POST', body: JSON.stringify({ id }) }).catch(() => {})
  const update = () => {
    run.disabled = reset.disabled = loading || !metadata
    editor.setAttribute('aria-busy', String(loading))
    if (!tools) return
    tools.previous.disabled = loading || !current || current.rowStart === 0
    tools.next.disabled = loading || !current || current.rowEnd >= current.totalRows
    tools.pageInput.disabled = loading || !current
  }
  function renderGrid(resetScroll = false) {
    const oldRows = [...content.querySelectorAll('tbody tr')], top = viewport.getBoundingClientRect().top
    const anchor = oldRows.findIndex(row => row.getBoundingClientRect().bottom > top)
    const anchorTop = oldRows[anchor]?.getBoundingClientRect().top
    const grid = document.createElement('table'), head = grid.createTHead().insertRow(), body = grid.createTBody()
    for (const column of state.columns) { const th = document.createElement('th'); th.textContent = current.columns[column]; head.append(th) }
    for (const row of current.rows) {
      const tr = body.insertRow()
      for (const column of state.columns) { const value = row[column], td = tr.insertCell(); td.textContent = value ?? 'NULL'; if (value == null) td.className = 'query-null' }
    }
    content.replaceChildren(grid)
    article.style.setProperty('--columns', state.columns.length)
    tools.columnCount.textContent = `${state.columns.length}/${names.length}`
    if (!layout) layout = tableLayout(article, content, state, () => new Map(widths), value => { widths = value }, () => {}, { viewport, workspace, shell })
    layout.update()
    if (resetScroll) viewport.scrollTop = 0
    else if (anchor >= 0) viewport.scrollTop += body.rows[anchor].getBoundingClientRect().top - anchorTop
    layout.position()
  }
  function paint(window) {
    if (names.join('\0') !== window.columns.join('\0')) {
      layout?.dispose(); tools?.dispose(); layout = null; widths = new Map(); names = window.columns
      state.columns = names.map((_, i) => i); state.horizontal = 0
      tools = tableControls({ names, selected:state.columns, label:'Results', previousLabel:'Previous query page', nextLabel:'Next query page', pageName:'Query page', bounds:()=>dialog, onApply:columns=>{state.columns=columns;renderGrid()} })
      tools.previous.addEventListener('click', () => load(Math.max(0, current.rowStart - current.pageSize)))
      tools.next.addEventListener('click', () => load(current.rowEnd))
      tools.pageInput.addEventListener('change', () => load((Math.max(1, Math.min(Number(tools.pageInput.max), Number(tools.pageInput.value) || 1)) - 1) * current.pageSize))
      tools.resetWidths.addEventListener('click', () => { tools.picker.open = false; layout?.reset() })
      controlsSlot.replaceChildren(tools.controls)
    }
    current = window
    renderGrid(true)
    tools.position.textContent = `${window.totalRows ? window.rowStart + 1 : 0}–${window.rowEnd} of ${window.totalRows} rows`
    tools.pageInput.value = Math.floor(window.rowStart / window.pageSize) + 1
    tools.pageInput.max = Math.max(1, Math.ceil(window.totalRows / window.pageSize)); tools.pageCount.textContent = ` / ${tools.pageInput.max}`
    content.dataset.queryId = window.id; content.dataset.rowStart = window.rowStart; content.dataset.totalRows = window.totalRows
  }
  async function load(row, sql) {
    if (loading || stopped) return
    loading = true; update(); message.textContent = sql == null ? 'Loading results…' : `Querying all ${metadata.sourceRows.toLocaleString()} source rows…`
    request?.abort(); request = new AbortController(); const generation = ++ticket
    try {
      const result = sql == null
        ? await api(`/api/table-query?id=${encodeURIComponent(current.id)}&row=${row}`, request.signal)
        : await api('/api/table-query', request.signal, { method: 'POST', body: JSON.stringify({ table, sql, revision: index.revision }) })
      if (stopped || generation !== ticket) { release(result.id); return }
      if (result.revision !== index.revision) { release(result.id); throw Error('Document revision changed; reopen SQL') }
      const old = current?.id
      paint(result)
      if (old && old !== result.id) release(old)
      message.textContent = result.totalRows ? `Read-only results · ${result.elapsedMs.toFixed(0)} ms` : 'No matching rows'
    } catch (error) {
      if (!stopped && error.name !== 'AbortError') {
        message.textContent = error.message
        if (/no such column:\s*c\d+\b/i.test(error.message)) message.textContent += ' · For c1, c2, … use FROM t; FROM data uses named columns.'
        if (!current) content.textContent = 'No results yet. Correct the query and run it again.'
      }
    } finally { loading = false; update() }
  }
  run.addEventListener('click', () => load(0, editor.value))
  reset.addEventListener('click', () => { editor.value = metadata.defaultSql; load(0, editor.value) })
  editor.addEventListener('keydown', event => { if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); if (!run.disabled) run.click() } })
  function dispose() { if (stopped) return; stopped = true; ticket++; request?.abort(); layout?.dispose(); tools?.dispose(); if (current) release(current.id); dialog.remove() }
  dialog.addEventListener('close', dispose, { once: true })
  update(); dialog.showModal()
  request = new AbortController()
  api(`/api/table-query/schema?table=${table}`, request.signal).then(value => {
    if (stopped) return
    metadata = value; editor.value = value.defaultSql
    help.textContent = 'SELECT c1 FROM t  →  positional aliases c1, c2, …\nSELECT * FROM data  →  named columns\n_row: original record · _line: source line\nValues remain text. Use CAST(c2 AS REAL) for numeric comparisons.\n\n' + value.columns.map((name, i) => `c${i + 1} ↔ "${name.replaceAll('"', '""')}"`).join('\n') + '\n\nSELECT c1, c2 FROM t WHERE c1 LIKE \'%text%\'\nSELECT c1, SUM(CAST(c2 AS REAL)) FROM t GROUP BY c1'
    load(0,value.defaultSql); editor.focus()
  }).catch(error => { if (!stopped) { message.textContent = error.message; content.textContent = 'Table unavailable.' } })
  return { dispose }
}
