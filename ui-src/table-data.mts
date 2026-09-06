export const MAX_ROWS = 10_000
export const MAX_COLUMNS = 200
export const MIN_COLUMN_WIDTH = 82
export const MAX_COLUMN_WIDTH = 720

export function clampTableColumnWidth(value: unknown) {
  return Math.min(MAX_COLUMN_WIDTH, Math.max(MIN_COLUMN_WIDTH, Math.round(Number(value) || MIN_COLUMN_WIDTH)))
}

export function resizeTableColumnWidths(widths: readonly number[], columnIndex: number, delta: unknown) {
  const next = widths.map(clampTableColumnWidth)
  if (!Number.isInteger(columnIndex) || columnIndex < 0 || columnIndex >= next.length) return next
  next[columnIndex] = clampTableColumnWidth(next[columnIndex]! + Number(delta || 0))
  return next
}

export function chartData(rows: string[][]) {
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

export function parseDelimited(text: unknown, delimiter: string) {
  const rows: string[][] = []
  let row: string[] = []; let field = ''; let quoted = false
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
  while (rows.length && rows.at(-1)!.every((value) => value === '')) rows.pop()
  return { sheets: [{ name: 'Data', rows }] }
}

export function cellText(value: unknown) {
  if (value == null) return ''
  if (value instanceof Date) return value.toISOString()
  if (typeof value === 'object') { const cell = value as Record<string, unknown>; return String(cell.text ?? cell.result ?? cell.formula ?? cell.hyperlink ?? '') }
  return String(value)
}

export function columnName(number: number) { let value = ''; for (let n = number; n; n = Math.floor((n - 1) / 26)) value = String.fromCharCode(65 + ((n - 1) % 26)) + value; return value }


