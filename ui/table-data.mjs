// Generated from ui-src; run npm run build:ui. Do not edit.
export const MAX_ROWS = 10_000;
export const MAX_COLUMNS = 200;
export const MIN_COLUMN_WIDTH = 82;
export const MAX_COLUMN_WIDTH = 720;
const DELIMITER_SAMPLE_ROWS = 64;
const COMMON_DELIMITERS = [',', '\t', ';'];
export function clampTableColumnWidth(value) {
    return Math.min(MAX_COLUMN_WIDTH, Math.max(MIN_COLUMN_WIDTH, Math.round(Number(value) || MIN_COLUMN_WIDTH)));
}
export function resizeTableColumnWidths(widths, columnIndex, delta) {
    const next = widths.map(clampTableColumnWidth);
    if (!Number.isInteger(columnIndex) || columnIndex < 0 || columnIndex >= next.length)
        return next;
    next[columnIndex] = clampTableColumnWidth(next[columnIndex] + Number(delta || 0));
    return next;
}
export function chartData(rows) {
    if (!rows.length)
        return [];
    const width = Math.max(...rows.map((row) => row.length));
    let valueColumn = -1;
    for (let column = 0; column < width; column += 1) {
        const numeric = rows.slice(1).filter((row) => row[column] !== '' && Number.isFinite(Number(row[column])));
        if (numeric.length >= Math.min(2, Math.max(1, rows.length - 1))) {
            valueColumn = column;
            break;
        }
    }
    if (valueColumn < 0)
        return [];
    const labelColumn = valueColumn === 0 ? -1 : 0;
    return rows.slice(1).map((row, index) => ({ label: labelColumn >= 0 ? String(row[labelColumn] || index + 1) : String(index + 1), value: Number(row[valueColumn]) })).filter((item) => Number.isFinite(item.value)).slice(0, 100);
}
export function tableSqlSource(rows) {
    const width = Math.max(1, ...rows.map((row) => row.length));
    const header = rows[0] || [];
    const used = new Set();
    const columns = Array.from({ length: width }, (_, index) => {
        let base = String(header[index] || '').replaceAll('\0', '').trim().slice(0, 128);
        if (!base)
            base = `column_${index + 1}`;
        let candidate = base;
        let suffix = 2;
        while (used.has(candidate.toLowerCase()))
            candidate = `${base}_${suffix++}`;
        used.add(candidate.toLowerCase());
        return candidate;
    });
    return {
        columns,
        rows: rows.slice(1).map((row) => Array.from({ length: width }, (_, index) => String(row[index] || ''))),
    };
}
export function defaultTableSql(columns) {
    const selection = columns.length
        ? columns.map((column) => `  "${String(column).replaceAll('"', '""')}"`).join(',\n')
        : '  *';
    return `SELECT\n${selection}\nFROM data\nLIMIT 1000`;
}
export function formatTableSqlColumns(sql, compact) {
    const source = String(sql || '');
    const parsed = parseSelectColumns(source);
    if (!parsed || parsed.columns.length < 2)
        return source;
    const columns = parsed.columns.map(compactSqlFragment);
    const selection = compact ? ` ${columns.join(', ')}\n` : `\n  ${columns.join(',\n  ')}\n`;
    return `${source.slice(0, parsed.selectEnd)}${selection}${source.slice(parsed.fromStart)}`;
}
function parseSelectColumns(source) {
    const select = /^\s*SELECT\b/iu.exec(source);
    if (!select)
        return null;
    const selectEnd = select[0].length;
    const columns = [];
    let itemStart = selectEnd;
    let quote = '';
    let depth = 0;
    for (let index = selectEnd; index < source.length; index += 1) {
        const character = source[index];
        if (quote) {
            if (quote === '[' && character === ']') {
                if (source[index + 1] === ']')
                    index += 1;
                else
                    quote = '';
            }
            else if (character === quote) {
                if (source[index + 1] === quote)
                    index += 1;
                else
                    quote = '';
            }
            continue;
        }
        if ((character === '-' && source[index + 1] === '-') || (character === '/' && source[index + 1] === '*'))
            return null;
        if (character === "'" || character === '"' || character === '`' || character === '[') {
            quote = character;
            continue;
        }
        if (character === '(') {
            depth += 1;
            continue;
        }
        if (character === ')') {
            depth = Math.max(0, depth - 1);
            continue;
        }
        if (depth === 0 && character === ',') {
            columns.push(source.slice(itemStart, index).trim());
            itemStart = index + 1;
            continue;
        }
        if (depth === 0 && source.slice(index, index + 4).toUpperCase() === 'FROM' && !/[\p{L}\p{N}_]/u.test(source[index - 1] || '') && !/[\p{L}\p{N}_]/u.test(source[index + 4] || '')) {
            columns.push(source.slice(itemStart, index).trim());
            if (columns.some((column) => !column))
                return null;
            return { selectEnd, fromStart: index, columns };
        }
    }
    return null;
}
function compactSqlFragment(source) {
    let output = '';
    let quote = '';
    let pendingSpace = false;
    for (let index = 0; index < source.length; index += 1) {
        const character = source[index];
        if (quote) {
            output += character;
            if (quote === '[' && character === ']') {
                if (source[index + 1] === ']')
                    output += source[++index];
                else
                    quote = '';
            }
            else if (character === quote) {
                if (source[index + 1] === quote)
                    output += source[++index];
                else
                    quote = '';
            }
            continue;
        }
        if (character === "'" || character === '"' || character === '`' || character === '[') {
            if (pendingSpace && output)
                output += ' ';
            pendingSpace = false;
            quote = character;
            output += character;
        }
        else if (/\s/u.test(character))
            pendingSpace = true;
        else {
            if (pendingSpace && output)
                output += ' ';
            pendingSpace = false;
            output += character;
        }
    }
    return output.trim();
}
export function detectDelimitedSeparator(text, fallback = ',') {
    const source = String(text || '').replace(/^\uFEFF/u, '');
    const candidates = [...new Set([fallback, ...COMMON_DELIMITERS])];
    let best = { delimiter: fallback, consistency: 0, matchingRows: 0, width: 1 };
    for (const delimiter of candidates) {
        const widths = delimitedRowWidths(source, delimiter);
        if (!widths.length)
            continue;
        const frequencies = new Map();
        for (const width of widths) {
            if (width > 1)
                frequencies.set(width, (frequencies.get(width) || 0) + 1);
        }
        let width = 1;
        let matchingRows = 0;
        for (const [candidateWidth, count] of frequencies) {
            if (count > matchingRows || (count === matchingRows && candidateWidth > width)) {
                width = candidateWidth;
                matchingRows = count;
            }
        }
        if (!matchingRows)
            continue;
        const consistency = matchingRows / widths.length;
        const better = consistency > best.consistency
            || (consistency === best.consistency && matchingRows > best.matchingRows)
            || (consistency === best.consistency && matchingRows === best.matchingRows && width > best.width)
            || (consistency === best.consistency && matchingRows === best.matchingRows && width === best.width && delimiter === fallback);
        if (better)
            best = { delimiter, consistency, matchingRows, width };
    }
    return best.delimiter;
}
function delimitedRowWidths(source, delimiter) {
    const widths = [];
    let width = 1;
    let quoted = false;
    let hasContent = false;
    for (let index = 0; index < source.length && widths.length < DELIMITER_SAMPLE_ROWS; index += 1) {
        const character = source[index];
        if (quoted && character === '"' && source[index + 1] === '"') {
            hasContent = true;
            index += 1;
        }
        else if (character === '"') {
            quoted = !quoted;
            hasContent = true;
        }
        else if (!quoted && character === delimiter) {
            width += 1;
            hasContent = true;
        }
        else if (!quoted && (character === '\n' || character === '\r')) {
            if (character === '\r' && source[index + 1] === '\n')
                index += 1;
            if (hasContent)
                widths.push(width);
            width = 1;
            hasContent = false;
        }
        else if (!/\s/u.test(character))
            hasContent = true;
    }
    if (hasContent && widths.length < DELIMITER_SAMPLE_ROWS)
        widths.push(width);
    return widths;
}
export function parseDelimited(text, delimiter) {
    const rows = [];
    let row = [];
    let field = '';
    let quoted = false;
    const source = String(text || '').replace(/^\uFEFF/u, '');
    for (let index = 0; index <= source.length; index += 1) {
        const char = source[index] ?? '\n';
        if (quoted && char === '"' && source[index + 1] === '"') {
            field += '"';
            index += 1;
        }
        else if (char === '"')
            quoted = !quoted;
        else if (!quoted && char === delimiter) {
            row.push(field);
            field = '';
        }
        else if (!quoted && (char === '\n' || char === '\r')) {
            if (char === '\r' && source[index + 1] === '\n')
                index += 1;
            row.push(field);
            field = '';
            rows.push(row.slice(0, MAX_COLUMNS));
            row = [];
            if (rows.length >= MAX_ROWS)
                break;
        }
        else
            field += char;
    }
    while (rows.length && rows.at(-1).every((value) => value === ''))
        rows.pop();
    return { sheets: [{ name: 'Data', rows }] };
}
export function cellText(value) {
    if (value == null)
        return '';
    if (value instanceof Date)
        return value.toISOString();
    if (typeof value === 'object') {
        const cell = value;
        return String(cell.text ?? cell.result ?? cell.formula ?? cell.hyperlink ?? '');
    }
    return String(value);
}
export function columnName(number) { let value = ''; for (let n = number; n; n = Math.floor((n - 1) / 26))
    value = String.fromCharCode(65 + ((n - 1) % 26)) + value; return value; }
