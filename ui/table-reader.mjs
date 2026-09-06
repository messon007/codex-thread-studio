import { Workbook } from './vendor/artifact-table.mjs'

const MAX_ROWS = 10_000
const MAX_COLUMNS = 200
const MIN_COLUMN_WIDTH = 82
const MAX_COLUMN_WIDTH = 720
const COLUMN_RESIZE_STEP = 12

export async function parseTabularArtifact({ bytes, path, text = '' }) {
  if (/\.csv$/iu.test(path)) return parseDelimited(text, ',')
  if (/\.tsv$/iu.test(path)) return parseDelimited(text, '\t')
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

export function renderTableArtifact({ container, workbook, initialSheet = 0, onSelection, translate = (value) => value }) {
  let sheetIndex = Math.min(workbook.sheets.length - 1, Math.max(0, Number(initialSheet) || 0))
  const columnWidthsBySheet = new Map()
  const shell = document.createElement('div')
  shell.className = 'table-reader'
  container.replaceChildren(shell)
  let clearActiveResize = () => {}
  let copyFeedbackTimer = null

  function render() {
    clearActiveResize()
    clearTimeout(copyFeedbackTimer)
    copyFeedbackTimer = null
    const sheet = workbook.sheets[sheetIndex] || { name: 'Sheet', rows: [] }
    const width = Math.max(1, ...sheet.rows.map((row) => row.length))
    const dimensions = translate('{rows} rows · {columns} columns', { rows: sheet.rows.length, columns: width })
    shell.innerHTML = `<div class="table-toolbar"><select data-table-sheet>${workbook.sheets.map((item, index) => `<option value="${index}"${index === sheetIndex ? ' selected' : ''}>${escapeHtml(item.name)}</option>`).join('')}</select><span>${escapeHtml(dimensions)}</span><button data-table-chart type="button">${escapeHtml(translate('Chart'))}</button><button data-table-copy type="button" disabled>${escapeHtml(translate('Copy'))}</button></div><div class="table-grid-wrap"><table class="table-grid"><colgroup><col data-table-row-column>${Array.from({ length: width }, (_, index) => `<col data-table-column-width="${index}">`).join('')}</colgroup><thead><tr><th></th>${Array.from({ length: width }, (_, index) => { const name = columnName(index + 1); return `<th class="table-column-header" data-table-column-header="${index}"><span>${name}</span><span class="table-column-resizer" data-table-column-resizer="${index}" role="separator" aria-orientation="vertical" aria-label="${escapeHtml(translate('Resize column {column}', { column: name }))}" aria-valuemin="${MIN_COLUMN_WIDTH}" aria-valuemax="${MAX_COLUMN_WIDTH}" tabindex="0"></span></th>` }).join('')}</tr></thead><tbody>${sheet.rows.map((row, rowIndex) => `<tr><th>${rowIndex + 1}</th>${Array.from({ length: width }, (_, columnIndex) => `<td tabindex="0" data-row="${rowIndex + 1}" data-column="${columnIndex + 1}">${escapeHtml(row[columnIndex] ?? '')}</td>`).join('')}</tr>`).join('')}</tbody></table></div><div class="table-chart hidden"></div>`
    shell.querySelector('[data-table-sheet]').addEventListener('change', (event) => { sheetIndex = Number(event.target.value); render() })
    const grid = shell.querySelector('.table-grid')
    const copyButton = shell.querySelector('[data-table-copy]')
    const columnHeaders = [...grid.querySelectorAll('[data-table-column-header]')]
    const columnElements = [...grid.querySelectorAll('[data-table-column-width]')]
    const resizeHandles = [...grid.querySelectorAll('[data-table-column-resizer]')]
    const rowHeaderWidth = Math.max(42, Math.round(grid.querySelector('thead th').getBoundingClientRect().width))
    let selectedCell = null

    function currentColumnWidths() {
      return columnHeaders.map((header) => clampTableColumnWidth(header.getBoundingClientRect().width))
    }

    function applyColumnWidths(values) {
      const widths = Array.from({ length: width }, (_, index) => clampTableColumnWidth(values[index]))
      columnWidthsBySheet.set(sheetIndex, widths)
      grid.classList.add('columns-resized')
      columnElements.forEach((column, index) => { column.style.width = `${widths[index]}px` })
      resizeHandles.forEach((handle, index) => handle.setAttribute('aria-valuenow', String(widths[index])))
      grid.style.width = `${rowHeaderWidth + widths.reduce((sum, value) => sum + value, 0)}px`
      return widths
    }

    const savedWidths = columnWidthsBySheet.get(sheetIndex)
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
      const widths = resizeTableColumnWidths(
        activeResize.widths,
        activeResize.columnIndex,
        event.clientX - activeResize.startX,
      )
      applyColumnWidths(widths)
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
        quote: cell.textContent || '', sheet: sheet.name,
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
    shell.querySelector('[data-table-chart]').addEventListener('click', () => toggleChart(shell, sheet, translate))
  }
  render()
  return {
    destroy: () => {
      clearActiveResize()
      clearTimeout(copyFeedbackTimer)
      shell.remove()
    },
  }
}

export function clampTableColumnWidth(value) {
  return Math.min(MAX_COLUMN_WIDTH, Math.max(MIN_COLUMN_WIDTH, Math.round(Number(value) || MIN_COLUMN_WIDTH)))
}

export function resizeTableColumnWidths(widths, columnIndex, delta) {
  const next = widths.map(clampTableColumnWidth)
  if (!Number.isInteger(columnIndex) || columnIndex < 0 || columnIndex >= next.length) return next
  next[columnIndex] = clampTableColumnWidth(next[columnIndex] + Number(delta || 0))
  return next
}

function toggleChart(shell, sheet, translate) {
  const grid = shell.querySelector('.table-grid-wrap')
  const chart = shell.querySelector('.table-chart')
  const button = shell.querySelector('[data-table-chart]')
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

function chartData(rows) {
  if (!rows.length) return []
  const width = Math.max(...rows.map((row) => row.length))
  let valueColumn = -1
  for (let column = 0; column < width; column += 1) {
    const numeric = rows.slice(1).filter((row) => row[column] !== '' && Number.isFinite(Number(row[column])))
    if (numeric.length >= Math.min(2, Math.max(1, rows.length - 1))) { valueColumn = column; break }
  }
  if (valueColumn < 0) return []
  const labelColumn = valueColumn === 0 ? -1 : 0
  return rows.slice(1).map((row, index) => ({ label: labelColumn >= 0 ? String(row[labelColumn] || index + 1) : String(index + 1), value: Number(row[valueColumn]) })).filter((item) => Number.isFinite(item.value)).slice(0, 100)
}

function parseDelimited(text, delimiter) {
  const rows = []
  let row = []; let field = ''; let quoted = false
  const source = String(text || '').replace(/^\uFEFF/u, '')
  for (let index = 0; index <= source.length; index += 1) {
    const char = source[index] ?? '\n'
    if (quoted && char === '"' && source[index + 1] === '"') { field += '"'; index += 1 }
    else if (char === '"') quoted = !quoted
    else if (!quoted && char === delimiter) { row.push(field); field = '' }
    else if (!quoted && (char === '\n' || char === '\r')) {
      if (char === '\r' && source[index + 1] === '\n') index += 1
      row.push(field); field = ''; rows.push(row.slice(0, MAX_COLUMNS)); row = []
      if (rows.length >= MAX_ROWS) break
    } else field += char
  }
  while (rows.length && rows.at(-1).every((value) => value === '')) rows.pop()
  return { sheets: [{ name: 'Data', rows }] }
}

function cellText(value) {
  if (value == null) return ''
  if (value instanceof Date) return value.toISOString()
  if (typeof value === 'object') return String(value.text ?? value.result ?? value.formula ?? value.hyperlink ?? '')
  return String(value)
}
function columnName(number) { let value = ''; for (let n = number; n; n = Math.floor((n - 1) / 26)) value = String.fromCharCode(65 + ((n - 1) % 26)) + value; return value }
function escapeHtml(value) { return String(value).replace(/[&<>"']/gu, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char])) }
