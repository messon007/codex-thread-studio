import { Workbook } from './vendor/artifact-table.mjs'
import { MAX_ROWS, MAX_COLUMNS, MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH, clampTableColumnWidth, resizeTableColumnWidths, chartData, defaultTableSql, detectDelimitedSeparator, parseDelimited, tableSqlSource, cellText, columnName } from './table-data.mjs'
export { clampTableColumnWidth, resizeTableColumnWidths } from './table-data.mjs'

const COLUMN_RESIZE_STEP = 12

export async function parseTabularArtifact({ bytes, path, text = '' }) {
  if (/\.(csv|tsv)$/iu.test(path)) {
    const fallback = /\.tsv$/iu.test(path) ? '\t' : ','
    return parseDelimited(text, detectDelimitedSeparator(text, fallback))
  }
  const workbook = new Workbook()
  await workbook.xlsx.load(bytes)
  return {
    sheets: workbook.worksheets.map((sheet) => {
      const rows = []
      sheet.eachRow({ includeEmpty: true }, (row, number) => {
        if (number > MAX_ROWS) return
        const values = []
        for (let column = 1; column <= Math.min(sheet.columnCount, MAX_COLUMNS); column += 1) {
          values.push(cellText(row.getCell(column).value))
        }
        rows.push(values)
      })
      return { name: sheet.name, rows }
    }),
  }
}

export function renderTableArtifact({ container, workbook, initialSheet = 0, onSelection, executeQuery, translate = (value) => value }) {
  let sheetIndex = Math.min(workbook.sheets.length - 1, Math.max(0, Number(initialSheet) || 0))
  let activeView = 'data'
  let wrap = false
  let queryResult = null
  let queryError = ''
  let queryRunning = false
  let queryGeneration = 0
  let destroyed = false
  const columnWidthsByView = new Map()
  const querySqlBySheet = new Map()
  const sqlSources = new Map()
  const shell = document.createElement('div')
  shell.className = 'table-reader'
  container.replaceChildren(shell)
  let clearActiveResize = () => {}
  let copyFeedbackTimer = null
  let closeCellDialog = () => {}

  function sourceSheet() {
    return workbook.sheets[sheetIndex] || { name: 'Sheet', rows: [] }
  }

  function sqlSource() {
    if (!sqlSources.has(sheetIndex)) sqlSources.set(sheetIndex, tableSqlSource(sourceSheet().rows))
    return sqlSources.get(sheetIndex)
  }

  function currentQuerySql() {
    if (!querySqlBySheet.has(sheetIndex)) querySqlBySheet.set(sheetIndex, defaultTableSql(sqlSource().columns))
    return querySqlBySheet.get(sheetIndex)
  }

  function gridMarkup(sheet, labels = null) {
    const width = labels?.length || Math.max(1, ...sheet.rows.map((row) => row.length))
    const columns = Array.from({ length: width }, (_, index) => labels?.[index] || columnName(index + 1))
    return {
      width,
      html: `<div class="table-grid-wrap"><table class="table-grid${wrap ? ' wrapped' : ''}"><colgroup><col data-table-row-column>${columns.map((_, index) => `<col data-table-column-width="${index}">`).join('')}</colgroup><thead><tr><th></th>${columns.map((label, index) => `<th class="table-column-header" data-table-column-header="${index}" title="${escapeHtml(label)}"><span>${escapeHtml(label)}</span><span class="table-column-resizer" data-table-column-resizer="${index}" role="separator" aria-orientation="vertical" aria-label="${escapeHtml(translate('Resize column {column}', { column: label }))}" aria-valuemin="${MIN_COLUMN_WIDTH}" aria-valuemax="${MAX_COLUMN_WIDTH}" tabindex="0"></span></th>`).join('')}</tr></thead><tbody>${sheet.rows.map((row, rowIndex) => `<tr><th>${rowIndex + 1}</th>${columns.map((_, columnIndex) => `<td tabindex="0" data-row="${rowIndex + 1}" data-column="${columnIndex + 1}">${escapeHtml(row[columnIndex] ?? '')}</td>`).join('')}</tr>`).join('')}</tbody></table></div>`,
    }
  }

  function render() {
    closeCellDialog()
    clearActiveResize()
    clearTimeout(copyFeedbackTimer)
    copyFeedbackTimer = null
    const rawSheet = sourceSheet()
    const showingQuery = activeView === 'sql' && typeof executeQuery === 'function'
    const displayedSheet = showingQuery ? { name: 'Query', rows: queryResult?.rows || [] } : rawSheet
    const labels = showingQuery ? queryResult?.columns || null : null
    const gridInfo = !showingQuery || queryResult ? gridMarkup(displayedSheet, labels) : null
    const source = typeof executeQuery === 'function' ? sqlSource() : null
    const width = gridInfo?.width || source?.columns.length || 1
    const dimensions = translate('{rows} rows · {columns} columns', { rows: displayedSheet.rows.length, columns: width })
    const queryStatus = queryRunning
      ? translate('Running query…')
      : queryError || (queryResult?.truncated
        ? translate('Result limited to {count} rows', { count: queryResult.rows.length })
        : queryResult ? translate('Query returned {rows} rows', { rows: queryResult.rows.length }) : '')
    const querySql = source ? currentQuerySql() : ''
    const viewSwitch = typeof executeQuery === 'function'
      ? `<div class="segmented-control table-view-switch" role="tablist" aria-label="${escapeHtml(translate('Table view'))}"><button data-table-view="data" type="button" role="tab" aria-selected="${String(!showingQuery)}" class="${!showingQuery ? 'active' : ''}">${escapeHtml(translate('Data'))}</button><button data-table-view="sql" type="button" role="tab" aria-selected="${String(showingQuery)}" class="${showingQuery ? 'active' : ''}">${escapeHtml(translate('SQL'))}</button></div>`
      : ''
    const toolbar = `<div class="table-toolbar">${viewSwitch}<select data-table-sheet>${workbook.sheets.map((item, index) => `<option value="${index}"${index === sheetIndex ? ' selected' : ''}>${escapeHtml(item.name)}</option>`).join('')}</select><span>${escapeHtml(dimensions)}</span><button data-table-chart type="button"${gridInfo ? '' : ' disabled'}>${escapeHtml(translate('Chart'))}</button><button data-table-copy type="button" disabled>${escapeHtml(translate('Copy'))}</button><button data-table-wrap type="button" class="${wrap ? 'active' : ''}" aria-pressed="${String(wrap)}" title="${escapeHtml(translate(wrap ? 'Disable wrapping' : 'Enable wrapping'))}">${escapeHtml(translate('Wrap'))}</button></div>`
    const queryView = `<div class="table-query-view"><section class="table-query-editor"><label><span>${escapeHtml(translate('SQLite query'))}</span><textarea data-table-sql spellcheck="false">${escapeHtml(querySql)}</textarea></label><div class="table-query-actions"><span class="table-query-status${queryError ? ' error' : ''}" role="status">${escapeHtml(queryStatus)}</span><button data-table-run class="primary-button" type="button"${queryRunning ? ' disabled' : ''}>${escapeHtml(translate('Run query'))}</button></div></section>${gridInfo?.html || `<div class="table-query-empty"><strong>${escapeHtml(translate('Query CSV data with SQLite'))}</strong><p>${escapeHtml(translate('Paste a read-only SELECT or WITH query, then run it with Ctrl/Cmd+Enter.'))}</p></div>`}<div class="table-chart hidden"></div></div>`
    shell.innerHTML = `${toolbar}${showingQuery ? queryView : `${gridInfo.html}<div class="table-chart hidden"></div>`}`

    shell.querySelector('[data-table-sheet]').addEventListener('change', (event) => {
      sheetIndex = Number(event.target.value)
      queryGeneration += 1
      queryRunning = false
      queryResult = null
      queryError = ''
      render()
    })
    shell.querySelectorAll('[data-table-view]').forEach((button) => button.addEventListener('click', () => {
      activeView = button.dataset.tableView
      render()
      if (activeView === 'sql') requestAnimationFrame(() => shell.querySelector('[data-table-sql]')?.focus())
    }))
    shell.querySelector('[data-table-wrap]').addEventListener('click', () => { wrap = !wrap; render() })
    const sqlInput = shell.querySelector('[data-table-sql]')
    if (sqlInput) {
      sqlInput.addEventListener('input', (event) => { querySqlBySheet.set(sheetIndex, event.target.value) })
      sqlInput.addEventListener('keydown', (event) => {
        if (!(event.ctrlKey || event.metaKey) || event.key !== 'Enter') return
        event.preventDefault()
        void runQuery()
      })
      shell.querySelector('[data-table-run]').addEventListener('click', () => { void runQuery() })
    }
    if (!gridInfo) return

    const grid = shell.querySelector('.table-grid')
    grid.title = translate('Double-click a cell or press Enter to view its full content.')
    const copyButton = shell.querySelector('[data-table-copy]')
    const columnHeaders = [...grid.querySelectorAll('[data-table-column-header]')]
    const columnElements = [...grid.querySelectorAll('[data-table-column-width]')]
    const resizeHandles = [...grid.querySelectorAll('[data-table-column-resizer]')]
    const rowHeaderWidth = Math.max(42, Math.round(grid.querySelector('thead th').getBoundingClientRect().width))
    const viewKey = `${activeView}:${sheetIndex}`
    let selectedCell = null

    function viewCell(cell) {
      closeCellDialog()
      const dialog = document.createElement('dialog')
      dialog.className = 'dialog table-cell-dialog'
      dialog.setAttribute('aria-label', translate('Cell content'))
      dialog.innerHTML = `<header class="dialog-header"><div><h2>${escapeHtml(translate('Cell content'))}</h2><p data-cell-address data-no-i18n></p></div><button class="icon-button" data-cell-close type="button" aria-label="${escapeHtml(translate('Close'))}">×</button></header><div class="table-cell-content" tabindex="0" data-no-i18n></div>`
      dialog.querySelector('[data-cell-address]').textContent = `${displayedSheet.name} · ${columnName(Number(cell.dataset.column))}${cell.dataset.row}`
      dialog.querySelector('.table-cell-content').textContent = cell.textContent || ''
      closeCellDialog = () => { dialog.close(); dialog.remove(); closeCellDialog = () => {} }
      dialog.querySelector('[data-cell-close]').addEventListener('click', closeCellDialog)
      dialog.addEventListener('cancel', event => { event.preventDefault(); closeCellDialog() })
      shell.append(dialog)
      dialog.showModal()
    }

    grid.addEventListener('dblclick', event => {
      const cell = event.target.closest('td[data-row]')
      if (!cell) return
      event.preventDefault()
      viewCell(cell)
    })
    grid.addEventListener('keydown', event => {
      const cell = event.target.closest('td[data-row]')
      if (!cell || event.key !== 'Enter' || event.isComposing) return
      event.preventDefault()
      event.stopPropagation()
      viewCell(cell)
    })

    function currentColumnWidths() {
      return columnHeaders.map((header) => clampTableColumnWidth(header.getBoundingClientRect().width))
    }

    function applyColumnWidths(values) {
      const widths = Array.from({ length: width }, (_, index) => clampTableColumnWidth(values[index]))
      columnWidthsByView.set(viewKey, widths)
      grid.classList.add('columns-resized')
      columnElements.forEach((column, index) => { column.style.width = `${widths[index]}px` })
      resizeHandles.forEach((handle, index) => handle.setAttribute('aria-valuenow', String(widths[index])))
      grid.style.width = `${rowHeaderWidth + widths.reduce((sum, value) => sum + value, 0)}px`
      return widths
    }

    const savedWidths = columnWidthsByView.get(viewKey)
    if (savedWidths?.length === width) applyColumnWidths(savedWidths)

    function stopColumnResize() {
      window.removeEventListener('mousemove', resizeColumn)
      window.removeEventListener('mouseup', stopColumnResize)
      window.removeEventListener('blur', stopColumnResize)
      document.body.classList.remove('resizing-table-column')
      resizeHandles.forEach((handle) => handle.classList.remove('active'))
      activeResize = null
      clearActiveResize = () => {}
    }

    let activeResize = null
    function resizeColumn(event) {
      if (!activeResize) return
      event.preventDefault()
      applyColumnWidths(resizeTableColumnWidths(activeResize.widths, activeResize.columnIndex, event.clientX - activeResize.startX))
    }

    grid.addEventListener('mousedown', (event) => {
      const handle = event.target.closest('[data-table-column-resizer]')
      if (!handle || event.button !== 0) return
      event.preventDefault()
      event.stopPropagation()
      stopColumnResize()
      const columnIndex = Number(handle.dataset.tableColumnResizer)
      const widths = currentColumnWidths()
      applyColumnWidths(widths)
      activeResize = { columnIndex, startX: event.clientX, widths }
      handle.classList.add('active')
      document.body.classList.add('resizing-table-column')
      window.addEventListener('mousemove', resizeColumn)
      window.addEventListener('mouseup', stopColumnResize)
      window.addEventListener('blur', stopColumnResize)
      clearActiveResize = stopColumnResize
    })

    grid.addEventListener('keydown', (event) => {
      const handle = event.target.closest('[data-table-column-resizer]')
      if (!handle || !['ArrowLeft', 'ArrowRight'].includes(event.key)) return
      event.preventDefault()
      event.stopPropagation()
      const columnIndex = Number(handle.dataset.tableColumnResizer)
      const direction = event.key === 'ArrowLeft' ? -1 : 1
      const step = COLUMN_RESIZE_STEP * (event.shiftKey ? 4 : 1)
      applyColumnWidths(resizeTableColumnWidths(currentColumnWidths(), columnIndex, direction * step))
    })

    function setCopyFeedback(label) {
      clearTimeout(copyFeedbackTimer)
      copyButton.textContent = translate(label)
      copyFeedbackTimer = setTimeout(() => {
        if (copyButton.isConnected) copyButton.textContent = translate('Copy')
        copyFeedbackTimer = null
      }, 1_200)
    }

    async function copySelectedCell() {
      if (!selectedCell) return
      try {
        await navigator.clipboard.writeText(selectedCell.textContent || '')
        setCopyFeedback('Copied')
      } catch (error) {
        console.error('Unable to copy table cell', error)
        setCopyFeedback('Copy failed')
      }
    }

    grid.addEventListener('click', (event) => {
      const cell = event.target.closest('td[data-row]')
      if (!cell) return
      shell.querySelectorAll('td.selected').forEach((node) => node.classList.remove('selected'))
      cell.classList.add('selected')
      selectedCell = cell
      copyButton.disabled = false
      onSelection?.({
        quote: cell.textContent || '', sheet: displayedSheet.name,
        range: `${columnName(Number(cell.dataset.column))}${cell.dataset.row}`,
        rect: cell.getBoundingClientRect(),
      })
    })
    grid.addEventListener('keydown', (event) => {
      const cell = event.target.closest('td[data-row]')
      const copyShortcut = (event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'c'
      if (!cell || !copyShortcut || window.getSelection()?.toString()) return
      event.preventDefault()
      if (selectedCell !== cell) {
        shell.querySelectorAll('td.selected').forEach((node) => node.classList.remove('selected'))
        cell.classList.add('selected')
        selectedCell = cell
        copyButton.disabled = false
      }
      copySelectedCell()
    })
    copyButton.addEventListener('click', copySelectedCell)
    const chartSheet = showingQuery ? { ...displayedSheet, rows: [queryResult.columns, ...displayedSheet.rows] } : displayedSheet
    shell.querySelector('[data-table-chart]').addEventListener('click', () => toggleChart(shell, chartSheet, translate))
  }

  async function runQuery() {
    if (queryRunning || typeof executeQuery !== 'function') return
    const generation = ++queryGeneration
    const source = sqlSource()
    queryRunning = true
    queryError = ''
    render()
    try {
      const result = await executeQuery({ ...source, sql: currentQuerySql() })
      if (destroyed || generation !== queryGeneration) return
      queryResult = {
        columns: Array.isArray(result?.columns) ? result.columns.map(String) : [],
        rows: Array.isArray(result?.rows) ? result.rows.map((row) => Array.isArray(row) ? row.map(String) : []) : [],
        truncated: Boolean(result?.truncated),
      }
    } catch (error) {
      if (destroyed || generation !== queryGeneration) return
      queryResult = null
      queryError = error?.message || String(error)
    } finally {
      if (!destroyed && generation === queryGeneration) {
        queryRunning = false
        render()
      }
    }
  }

  render()
  return {
    destroy: () => {
      destroyed = true
      queryGeneration += 1
      closeCellDialog()
      clearActiveResize()
      clearTimeout(copyFeedbackTimer)
      shell.remove()
    },
  }
}

function toggleChart(shell, sheet, translate) {
  const grid = shell.querySelector('.table-grid-wrap')
  const chart = shell.querySelector('.table-chart')
  const button = shell.querySelector('[data-table-chart]')
  if (!grid || !chart || !button) return
  const showing = chart.classList.contains('hidden')
  grid.classList.toggle('hidden', showing)
  chart.classList.toggle('hidden', !showing)
  button.textContent = translate(showing ? 'Grid' : 'Chart')
  if (!showing) return
  const data = chartData(sheet.rows)
  if (!data.length) { chart.innerHTML = `<p>${escapeHtml(translate('No numeric column is available for a chart.'))}</p>`; return }
  const max = Math.max(...data.map((item) => Math.abs(item.value)), 1)
  chart.innerHTML = `<svg viewBox="0 0 720 360" role="img" aria-label="Bar chart">${data.map((item, index) => {
    const width = Math.max(3, 620 / data.length - 4); const height = Math.abs(item.value) / max * 270; const x = 70 + index * (620 / data.length); const y = 310 - height
    return `<g><rect x="${x}" y="${y}" width="${width}" height="${height}" rx="2"></rect><title>${escapeHtml(item.label)}: ${item.value}</title>${data.length <= 18 ? `<text x="${x + width / 2}" y="330" text-anchor="middle">${escapeHtml(item.label.slice(0, 10))}</text>` : ''}</g>`
  }).join('')}<line x1="60" y1="310" x2="710" y2="310"></line></svg>`
}

function escapeHtml(value) { return String(value).replace(/[&<>"']/gu, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char])) }
