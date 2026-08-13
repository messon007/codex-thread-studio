import { Workbook } from './vendor/artifact-table.mjs'

const MAX_ROWS = 10_000
const MAX_COLUMNS = 200

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
  const shell = document.createElement('div')
  shell.className = 'table-reader'
  container.replaceChildren(shell)

  function render() {
    const sheet = workbook.sheets[sheetIndex] || { name: 'Sheet', rows: [] }
    const width = Math.max(1, ...sheet.rows.map((row) => row.length))
    const dimensions = translate('{rows} 行 · {columns} 列', { rows: sheet.rows.length, columns: width })
    shell.innerHTML = `<div class="table-toolbar"><select data-table-sheet>${workbook.sheets.map((item, index) => `<option value="${index}"${index === sheetIndex ? ' selected' : ''}>${escapeHtml(item.name)}</option>`).join('')}</select><span>${escapeHtml(dimensions)}</span><button data-table-chart type="button">${escapeHtml(translate('图表'))}</button></div><div class="table-grid-wrap"><table class="table-grid"><thead><tr><th></th>${Array.from({ length: width }, (_, index) => `<th>${columnName(index + 1)}</th>`).join('')}</tr></thead><tbody>${sheet.rows.map((row, rowIndex) => `<tr><th>${rowIndex + 1}</th>${Array.from({ length: width }, (_, columnIndex) => `<td tabindex="0" data-row="${rowIndex + 1}" data-column="${columnIndex + 1}">${escapeHtml(row[columnIndex] ?? '')}</td>`).join('')}</tr>`).join('')}</tbody></table></div><div class="table-chart hidden"></div>`
    shell.querySelector('[data-table-sheet]').addEventListener('change', (event) => { sheetIndex = Number(event.target.value); render() })
    shell.querySelector('.table-grid').addEventListener('click', (event) => {
      const cell = event.target.closest('td[data-row]')
      if (!cell) return
      shell.querySelectorAll('td.selected').forEach((node) => node.classList.remove('selected'))
      cell.classList.add('selected')
      onSelection?.({
        quote: cell.textContent || '', sheet: sheet.name,
        range: `${columnName(Number(cell.dataset.column))}${cell.dataset.row}`,
        rect: cell.getBoundingClientRect(),
      })
    })
    shell.querySelector('[data-table-chart]').addEventListener('click', () => toggleChart(shell, sheet, translate))
  }
  render()
  return { destroy: () => shell.remove() }
}

function toggleChart(shell, sheet, translate) {
  const grid = shell.querySelector('.table-grid-wrap')
  const chart = shell.querySelector('.table-chart')
  const button = shell.querySelector('[data-table-chart]')
  const showing = chart.classList.contains('hidden')
  grid.classList.toggle('hidden', showing)
  chart.classList.toggle('hidden', !showing)
  button.textContent = translate(showing ? '网格' : '图表')
  if (!showing) return
  const data = chartData(sheet.rows)
  if (!data.length) { chart.innerHTML = `<p>${escapeHtml(translate('没有可用于绘图的数值列。'))}</p>`; return }
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
