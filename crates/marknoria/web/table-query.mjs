import { queryOperation } from './query-operation.mjs'
import { dialogClose } from './icons.mjs'
import { tableLayout } from './table-layout.mjs'
import { tableControls } from './table-controls.mjs'
import { mountTableCells } from './table-cells.mjs'
import { componentStyles } from './component-styles.mjs'
import { uiLabels } from './translation.mjs'
import { mountTablePlotDialog } from './table-plot.mjs'

function columnNames(names) {
  return (
    Array.isArray(names) &&
    names.length > 0 &&
    names.length <= 256 &&
    names.every((name) => typeof name === 'string') &&
    names.reduce((sum, name) => sum + name.length, 0) <= 65536
  )
}
function checkedWindow(value) {
  if (
    !value ||
    !columnNames(value.columns) ||
    typeof value.id !== 'string' ||
    !/^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(value.id) ||
    ![value.rowStart, value.rowEnd, value.totalRows, value.pageSize].every(Number.isSafeInteger) ||
    value.rowStart < 0 ||
    value.rowEnd < value.rowStart ||
    value.rowEnd > value.totalRows ||
    value.pageSize < 1 ||
    value.pageSize > 50 ||
    !Array.isArray(value.rows) ||
    value.rows.length > 50 ||
    value.rows.length !== value.rowEnd - value.rowStart ||
    value.rows.length * value.columns.length > 512 ||
    !Number.isFinite(value.elapsedMs) ||
    value.elapsedMs < 0 ||
    value.elapsedMs > Number.MAX_SAFE_INTEGER
  )
    throw Error('Invalid bounded SQL result window')
  let units = 0
  for (const row of value.rows) {
    if (!Array.isArray(row) || row.length !== value.columns.length)
      throw Error('SQL result columns do not match')
    for (const cell of row) {
      if (cell !== null && typeof cell !== 'string')
        throw Error('SQL result cells must preserve text or null')
      units += cell?.length || 0
      if (units > 131072) throw Error('SQL result exceeds its text budget')
    }
  }
  if (
    new TextEncoder().encode(JSON.stringify({ columns: value.columns, rows: value.rows })).length >
    131072
  )
    throw Error('SQL result exceeds its transport budget')
  return value
}

export function mountTableQuery(container, { provider, revision, ...options }) {
  if (
    !provider ||
    ['schema', 'run', 'page', 'close'].some((name) => typeof provider[name] !== 'function') ||
    typeof revision !== 'string'
  )
    throw new TypeError('SQL requires a scoped complete-input provider and revision')
  const lifetime = new AbortController()
  const labels = uiLabels(options.translate, { signal: lifetime.signal })
  const releaseStyles = componentStyles(
    container.ownerDocument,
    new URL('./table-query.css', import.meta.url)
  )
  const releaseTableStyles = componentStyles(
    container.ownerDocument,
    new URL('./table-view.css', import.meta.url)
  )
  const previousFocus = document.activeElement
  const table = options.table ?? 0
  const dialog = document.createElement('dialog')
  dialog.className = 'table-query-dialog'
  labels.set(dialog, 'Query table with SQL', {}, 'aria-label')
  const shell = document.createElement('section')
  shell.className = 'table-query-shell'
  shell.setAttribute('data-marknoria-table-view', '')
  const parentShell = options.presentationRoot || container
  shell.classList.toggle('no-wrap', parentShell.classList.contains('no-wrap'))
  const presentation = getComputedStyle(parentShell)
  for (const name of [
    '--column-width',
    '--reader-panel',
    '--reader-soft',
    '--reader-text',
    '--reader-muted',
    '--reader-faint',
    '--reader-border',
    '--reader-brand',
    '--reader-brand-soft',
    '--ui-font-family',
    '--ui-font-size',
    '--code-font-family'
  ]) {
    const value = presentation.getPropertyValue(name)
    if (value) shell.style.setProperty(name, value)
  }
  const header = document.createElement('header')
  header.className = 'table-query-heading'
  const title = document.createElement('h2')
  labels.set(title, 'Query table with SQL')
  const close = dialogClose(() => dialog.close())
  labels.set(close, 'Close', {}, 'aria-label')
  labels.set(close, 'Close', {}, 'title')
  header.append(title, close)
  const editor = document.createElement('textarea')
  editor.className = 'table-query-editor'
  editor.maxLength = 16384
  labels.set(editor, 'SQL query', {}, 'aria-label')
  editor.spellcheck = false
  const schema = document.createElement('details')
  schema.className = 'table-query-schema'
  const summary = document.createElement('summary')
  labels.set(summary, 'Columns and query help')
  const help = document.createElement('pre')
  schema.append(summary, help)
  const actions = document.createElement('div')
  actions.className = 'table-query-actions'
  const run = document.createElement('button')
  labels.set(run, 'Run query')
  run.className = 'table-query-run'
  const reset = document.createElement('button')
  labels.set(reset, 'Reset query')
  const message = document.createElement('span')
  message.setAttribute('role', 'status')
  actions.append(run, reset, message)
  const controlsSlot = document.createElement('div')
  controlsSlot.className = 'table-query-controls'
  const workspace = document.createElement('div')
  workspace.className = 'table-query-workspace'
  const viewport = document.createElement('div')
  viewport.className = 'table-query-viewport'
  viewport.tabIndex = 0
  const article = document.createElement('article')
  article.className = 'page'
  article.dataset.table = `query-${table}`
  const content = document.createElement('div')
  content.className = 'table-content'
  labels.set(content, 'Preparing table…')
  article.append(content)
  viewport.append(article)
  workspace.append(viewport)
  const results = document.createElement('section')
  results.className = 'table-query-result'
  results.append(controlsSlot, workspace)
  shell.append(header, editor, schema, actions, results)
  dialog.append(shell)
  container.append(dialog)
  let chartDialog = null
  const chart = document.createElement('button')
  chart.type = 'button'
  labels.set(chart, 'Chart')
  chart.className = 'table-chart-open'
  if (provider.chart) actions.append(chart)
  chart.addEventListener('click', () => {
    if (!current || loading) return
    const result = current
    chartDialog?.dispose()
    chartDialog = mountTablePlotDialog(container, {
      ...options,
      presentationRoot: shell,
      columns: result.columns,
      load: ({ x, y, signal }) => provider.chart({ id: result.id, x, y, signal })
    })
  })
  let request,
    ticket = 0,
    loading = false,
    metadata,
    current,
    layout,
    tools,
    names = [],
    widths = new Map(),
    stopped = false
  const state = { columns: [], horizontal: 0 }
  let cells
  try {
    cells = mountTableCells(content, {
      controls: actions,
      locate: (cell) =>
        current
          ? {
              kind: 'queryCell',
              queryId: current.id,
              row: current.rowStart + cell.parentElement.sectionRowIndex,
              column: state.columns[cell.cellIndex]
            }
          : null,
      onSelection: options.onSelection,
      translate: options.translate
    })
  } catch (error) {
    lifetime.abort()
    releaseStyles()
    releaseTableStyles()
    throw error
  }
  const release = (id) =>
    Promise.resolve()
      .then(() => provider.close({ id }))
      .catch(() => {})
  const update = () => {
    chart.disabled = loading || !current
    run.disabled = reset.disabled = loading || !metadata
    editor.disabled = !metadata
    editor.setAttribute('aria-busy', String(loading))
    if (!tools) return
    tools.previous.disabled = loading || !current || current.rowStart === 0
    tools.next.disabled = loading || !current || current.rowEnd >= current.totalRows
    tools.pageInput.disabled = loading || !current
  }
  function renderGrid(resetScroll = false) {
    const oldRows = [...content.querySelectorAll('tbody tr')],
      top = viewport.getBoundingClientRect().top
    const anchor = oldRows.findIndex((row) => row.getBoundingClientRect().bottom > top)
    const anchorTop = oldRows[anchor]?.getBoundingClientRect().top
    const grid = document.createElement('table'),
      head = grid.createTHead().insertRow(),
      body = grid.createTBody()
    for (const column of state.columns) {
      const th = document.createElement('th')
      th.textContent = current.columns[column]
      head.append(th)
    }
    for (const row of current.rows) {
      const tr = body.insertRow()
      for (const column of state.columns) {
        const value = row[column],
          td = tr.insertCell()
        td.textContent = value ?? 'NULL'
        if (value == null) td.className = 'query-null'
      }
    }
    labels.clear(content)
    content.replaceChildren(grid)
    cells.update()
    article.style.setProperty('--columns', state.columns.length)
    tools.columns(state.columns)
    if (!layout)
      layout = tableLayout(
        article,
        content,
        state,
        () => new Map(widths),
        (value) => {
          widths = value
        },
        () => {},
        { viewport, workspace, shell, translate: options.translate }
      )
    layout.update()
    if (resetScroll) viewport.scrollTop = 0
    else if (anchor >= 0)
      viewport.scrollTop += body.rows[anchor].getBoundingClientRect().top - anchorTop
    layout.position()
  }
  function paint(window) {
    if (
      names.length !== window.columns.length ||
      names.some((name, index) => name !== window.columns[index])
    ) {
      const nextNames = window.columns,
        nextColumns = nextNames.map((_, index) => index)
      const nextTools = tableControls({
        names: nextNames,
        selected: nextColumns,
        label: 'Results',
        labelKey: 'Results',
        translate: options.translate,
        previousLabel: 'Previous query page',
        nextLabel: 'Next query page',
        pageName: 'Query page',
        bounds: () => dialog,
        onApply: (columns) => {
          state.columns = columns
          renderGrid()
        }
      })
      layout?.dispose()
      tools?.dispose()
      layout = null
      widths = new Map()
      names = nextNames
      state.columns = nextColumns
      state.horizontal = 0
      tools = nextTools
      tools.previous.addEventListener('click', () =>
        load(Math.max(0, current.rowStart - current.pageSize))
      )
      tools.next.addEventListener('click', () => load(current.rowEnd))
      tools.pageInput.addEventListener('change', () =>
        load(
          (Math.max(1, Math.min(Number(tools.pageInput.max), Number(tools.pageInput.value) || 1)) -
            1) *
            current.pageSize
        )
      )
      tools.resetWidths.addEventListener('click', () => {
        tools.picker.open = false
        layout?.reset()
      })
      controlsSlot.replaceChildren(tools.controls)
    }
    current = window
    renderGrid(true)
    tools.pageInput.value = Math.floor(window.rowStart / window.pageSize) + 1
    tools.pageInput.max = Math.max(1, Math.ceil(window.totalRows / window.pageSize))
    tools.counts(
      window.totalRows ? window.rowStart + 1 : 0,
      window.rowEnd,
      window.totalRows,
      Number(tools.pageInput.max),
      state.columns
    )
    content.dataset.queryId = window.id
    content.dataset.rowStart = window.rowStart
    content.dataset.totalRows = window.totalRows
  }
  async function load(row, sql) {
    if (loading || stopped) return
    loading = true
    update()
    if (sql == null) labels.set(message, 'Loading results…')
    else labels.set(message, 'Querying all {rows} source rows…', { rows: metadata.sourceRows })
    request?.abort()
    request = new AbortController()
    const generation = ++ticket,
      previousResult = current,
      isRun = sql != null
    try {
      const result = isRun
        ? await provider.run({ sql, signal: request.signal })
        : await provider.page({ id: previousResult.id, row, signal: request.signal })
      const discard = () => {
        if (isRun && result?.id && result.id !== previousResult?.id) release(result.id)
      }
      if (stopped || generation !== ticket) {
        discard()
        return
      }
      try {
        checkedWindow(result)
        if (result.revision !== revision) throw Error('Document revision changed; reopen SQL')
        if (
          !isRun &&
          (result.id !== previousResult.id ||
            result.totalRows !== previousResult.totalRows ||
            result.columns.length !== previousResult.columns.length ||
            result.columns.some((name, index) => name !== previousResult.columns[index]))
        )
          throw Error('Query result identity or schema changed; run the query again')
        if (result.totalRows > 0 && (row < result.rowStart || row >= result.rowEnd))
          throw Error('Query window does not contain the requested row')
        paint(result)
      } catch (error) {
        discard()
        throw error
      }
      if (previousResult && previousResult.id !== result.id) {
        chartDialog?.dispose()
        chartDialog = null
        release(previousResult.id)
      }
      if (result.totalRows)
        labels.set(message, 'Read-only results · {milliseconds} ms', {
          milliseconds: Math.round(result.elapsedMs)
        })
      else labels.set(message, 'No matching rows')
    } catch (error) {
      if (!stopped && error.name !== 'AbortError') {
        labels.clear(message)
        message.textContent = error.message
        if (/no such column:\s*c\d+\b/i.test(error.message)) {
          const hint = document.createElement('span')
          labels.set(hint, 'For c1, c2, … use FROM t; FROM data uses named columns.')
          message.append(' · ', hint)
        }
        if (!current) labels.set(content, 'No results yet. Correct the query and run it again.')
      }
    } finally {
      loading = false
      update()
    }
  }
  run.addEventListener('click', () => load(0, editor.value))
  reset.addEventListener('click', () => {
    editor.value = metadata.defaultSql
    load(0, editor.value)
  })
  editor.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault()
      if (!run.disabled) run.click()
    }
  })
  let closing = Promise.resolve()
  function dispose() {
    if (stopped) return closing
    stopped = true
    lifetime.abort()
    chartDialog?.dispose()
    ticket++
    request?.abort()
    cells.dispose()
    layout?.dispose()
    tools?.dispose()
    releaseStyles()
    releaseTableStyles()
    if (current) closing = release(current.id)
    dialog.remove()
    if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true })
    return closing
  }
  dialog.addEventListener('close', dispose, { once: true })
  update()
  dialog.showModal()
  request = new AbortController()
  const ready = Promise.resolve()
    .then(() => provider.schema({ signal: request.signal }))
    .then((value) => {
      if (stopped) return
      if (
        !value ||
        !columnNames(value.columns) ||
        !Number.isSafeInteger(value.sourceRows) ||
        value.sourceRows < 0 ||
        typeof value.defaultSql !== 'string' ||
        value.defaultSql.length > 16384
      )
        throw Error('Invalid bounded SQL schema')
      metadata = value
      editor.value = value.defaultSql
      const instructions = document.createElement('span')
      labels.set(
        instructions,
        'SELECT c1 FROM t  →  positional aliases c1, c2, …\nSELECT * FROM data  →  named columns\n_row: original record · _line: source line\nValues remain text. Use CAST(c2 AS REAL) for numeric comparisons.\n\n'
      )
      help.replaceChildren(
        instructions,
        value.columns.map((name, i) => `c${i + 1} ↔ "${name.replaceAll('"', '""')}"`).join('\n'),
        "\n\nSELECT c1, c2 FROM t WHERE c1 LIKE '%text%'\nSELECT c1, SUM(CAST(c2 AS REAL)) FROM t GROUP BY c1"
      )
      const loading = load(0, value.defaultSql)
      editor.focus()
      return loading
    })
    .catch((error) => {
      if (!stopped) {
        labels.clear(message)
        message.textContent = error.message
        labels.set(content, 'Table unavailable.')
      }
    })
  return {
    ready,
    state: () => ({
      loading,
      stopped,
      id: current?.id || null,
      totalRows: current?.totalRows ?? null,
      rowStart: current?.rowStart ?? null,
      message: message.textContent
    }),
    execute: async (sql) => {
      if (stopped || loading || !metadata)
        throw Error(stopped ? 'SQL reader disposed' : 'SQL reader busy or preparing schema')
      if (typeof sql !== 'string' || sql.length > 16384) throw Error('Invalid SQL text')
      editor.value = sql
      await load(0, sql)
    },
    go: async (row) => {
      if (stopped || loading) throw Error(stopped ? 'SQL reader disposed' : 'SQL reader busy')
      if (
        !current ||
        !Number.isSafeInteger(row) ||
        row < 0 ||
        row >= Math.max(1, current.totalRows)
      )
        throw Error('SQL row out of range')
      await load(row)
    },
    dispose,
    isOpen: () => !stopped,
    selection: () => (chartDialog?.isOpen() ? chartDialog.selection() : cells.selection())
  }
}

export function tableQuery(table, index, api, options = {}) {
  return mountTableQuery(document.body, {
    ...options,
    table,
    revision: index.revision,
    presentationRoot: options.presentationRoot || document.querySelector('.reader-shell'),
    provider: {
      schema: ({ signal }) => api(`/api/table-query/schema?table=${table}`, signal),
      run: ({ sql, signal }) =>
        queryOperation(api, index.revision, signal, (operationId) =>
          api('/api/table-query', signal, {
            method: 'POST',
            body: JSON.stringify({ table, sql, revision: index.revision, operationId })
          })
        ),
      page: ({ id, row, signal }) =>
        api(`/api/table-query?id=${encodeURIComponent(id)}&row=${row}`, signal),
      close: ({ id }) =>
        api('/api/table-query/close', undefined, { method: 'POST', body: JSON.stringify({ id }) }),
      chart: ({ id, x, y, signal }) =>
        queryOperation(api, index.revision, signal, (operationId) =>
          api('/api/table-chart', signal, {
            method: 'POST',
            body: JSON.stringify({
              revision: index.revision,
              source: { kind: 'query', queryId: id },
              x,
              y,
              operationId
            })
          })
        )
    }
  })
}

export function readerQueryProvider(client, owner, table) {
  const identity = { documentId: owner.documentId, revision: owner.revision }
  const request = (method, params, signal) =>
    client.request(method, { ...identity, ...params }, { signal })
  return {
    schema: ({ signal }) => request('sqlSchema', { table }, signal),
    run: ({ sql, signal }) => request('sql', { table, sql }, signal),
    page: ({ id, row, signal }) => request('sqlPage', { id, row }, signal),
    close: ({ id }) => request('sqlClose', { id }),
    chart: ({ id, x, y, signal }) =>
      request('tableChart', { source: { kind: 'query', queryId: id }, x, y }, signal)
  }
}
