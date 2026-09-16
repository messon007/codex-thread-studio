export const MAX_ROWS = 10_000
export const MAX_COLUMNS = 200
export const MIN_COLUMN_WIDTH = 82
export const MAX_COLUMN_WIDTH = 720
const DELIMITER_SAMPLE_ROWS = 64
const COMMON_DELIMITERS = [',', '\t', ';'] as const

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

export function detectDelimitedSeparator(text: unknown, fallback = ',') {
  const source = String(text || '').replace(/^\uFEFF/u, '')
  const candidates = [...new Set([fallback, ...COMMON_DELIMITERS])]
  let best = { delimiter: fallback, consistency: 0, matchingRows: 0, width: 1 }
  for (const delimiter of candidates) {
    const widths = delimitedRowWidths(source, delimiter)
    if (!widths.length) continue
    const frequencies = new Map<number, number>()
    for (const width of widths) {
      if (width > 1) frequencies.set(width, (frequencies.get(width) || 0) + 1)
    }
    let width = 1
    let matchingRows = 0
    for (const [candidateWidth, count] of frequencies) {
      if (count > matchingRows || (count === matchingRows && candidateWidth > width)) {
        width = candidateWidth
        matchingRows = count
      }
    }
    if (!matchingRows) continue
    const consistency = matchingRows / widths.length
    const better = consistency > best.consistency
      || (consistency === best.consistency && matchingRows > best.matchingRows)
      || (consistency === best.consistency && matchingRows === best.matchingRows && width > best.width)
      || (consistency === best.consistency && matchingRows === best.matchingRows && width === best.width && delimiter === fallback)
    if (better) best = { delimiter, consistency, matchingRows, width }
  }
  return best.delimiter
}

function delimitedRowWidths(source: string, delimiter: string) {
  const widths: number[] = []
  let width = 1
  let quoted = false
  let hasContent = false
  for (let index = 0; index < source.length && widths.length < DELIMITER_SAMPLE_ROWS; index += 1) {
    const character = source[index]!
    if (quoted && character === '"' && source[index + 1] === '"') {
      hasContent = true
      index += 1
    } else if (character === '"') {
      quoted = !quoted
      hasContent = true
    } else if (!quoted && character === delimiter) {
      width += 1
      hasContent = true
    } else if (!quoted && (character === '\n' || character === '\r')) {
      if (character === '\r' && source[index + 1] === '\n') index += 1
      if (hasContent) widths.push(width)
      width = 1
      hasContent = false
    } else if (!/\s/u.test(character)) hasContent = true
  }
  if (hasContent && widths.length < DELIMITER_SAMPLE_ROWS) widths.push(width)
  return widths
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

