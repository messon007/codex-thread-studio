// Presentation only: Rust/SQLite and Rust/jaq remain the query authorities.
export function sqlExamples(columns) {
  const field = columns.length > 1 ? 'c2' : 'c1'
  const named = '"' + columns[0].replaceAll('"', '""') + '"'
  return [
    ['Preview this table', 'SELECT c1 FROM t LIMIT 20'],
    ['Use a named column', `SELECT ${named} AS value FROM data LIMIT 20`],
    ['Exact text match', "SELECT c1 FROM t WHERE c1 = 'text' LIMIT 20"],
    ['Match any value in a list', "SELECT c1 FROM t WHERE c1 IN ('first', 'second') LIMIT 20"],
    ['Literal substring (case sensitive)', "SELECT c1 FROM t WHERE instr(c1, 'text') > 0 LIMIT 20"],
    ['Wildcard substring', "SELECT c1 FROM t WHERE c1 LIKE '%text%' LIMIT 20"],
    ['Match a literal percent sign', "SELECT c1 FROM t WHERE c1 LIKE '%!%%' ESCAPE '!' LIMIT 20"],
    ['Numeric comparison and sort', `SELECT ${field} FROM t WHERE CAST(${field} AS REAL) > 10 ORDER BY CAST(${field} AS REAL) DESC LIMIT 20`],
    ['Group and count', 'SELECT c1, COUNT(*) AS count FROM t GROUP BY c1 HAVING COUNT(*) > 1 ORDER BY count DESC LIMIT 20'],
    ['Numeric total', `SELECT SUM(CAST(${field} AS REAL)) AS total FROM t`],
    ['Source coordinates', 'SELECT _row, _line, c1 FROM t ORDER BY _row LIMIT 20']
  ]
}
export const jqExamples = [
  ['Any JSON value; slurp off', '.'],
  ['Inspect the input type; slurp off', 'type'],
  ['Object keys; requires an object or array', 'keys'],
  ['Array items; requires an array', '.[]'],
  ['Object field; requires an object', '.["field name"]'],
  ['Filter an array of objects; slurp off', '.[] | select(.active == true)'],
  ['Project an array of objects; slurp off', 'map({name: .name, value: .value})'],
  ['Filter JSONL / NDJSON records; slurp off', 'select(.level == "error")'],
  ['Sum values across JSONL records; slurp on', 'map(.value | tonumber) | add'],
  ['Sort JSONL records; slurp on', 'sort_by(.value)'],
  ['Count JSONL records; slurp on', 'length']
]
function paragraph(root, labels, text) {
  const p = document.createElement('p')
  for (const sentence of text.split(/(?<=\.) (?=[A-Z])/)) {
    const span = document.createElement('span')
    labels.set(span, sentence)
    p.append(span, ' ')
  }
  root.append(p)
}
function examples(root, labels, items, select) {
  paragraph(root, labels, 'Choose an example to place it in the editor, then adjust values and run it. Examples do not run automatically.')
  for (const [label, code] of items) {
    const row = document.createElement('div'), button = document.createElement('button'), sample = document.createElement('code')
    row.className = 'query-help-example'
    button.type = 'button'
    labels.set(button, label)
    sample.textContent = code
    button.addEventListener('click', () => select(code))
    row.append(button, sample)
    root.append(row)
  }
}
export function sqlHelp(root, labels, columns, select) {
  paragraph(root, labels, 'SQLite SQL over every row of the current table. t provides positional aliases c1, c2, …; data provides the quoted column names below. Other document tables are not available in this query.')
  const names = document.createElement('pre')
  names.textContent = columns.map((name, i) => `t.c${i + 1} ↔ data."${name.replaceAll('"', '""')}"`).join('\n')
  root.append(names)
  paragraph(root, labels, 'Double quotes name columns; single quotes enclose text. Escape a quote by doubling it: \'it\'\'s\'. Blank or duplicate column names are normalized in the mapping above. _row is the original record number; _line is its source line. Both start at 1.')
  paragraph(root, labels, 'Source cells remain TEXT, including numbers and empty strings. Use CAST(column AS REAL) or CAST(column AS INTEGER) for numeric comparisons, sorting and sums. SQLite may convert nonnumeric text to 0; check your data first. Empty text is not SQL NULL; use = \'\' for empty text and IS NULL for query-produced nulls.')
  paragraph(root, labels, 'IN tests membership: c1 IN (\'first\', \'second\'). The form \'text\' IN c2 treats c2 as a table name and fails; it does not search a cell. Use c2 = \'text\' for equality or instr(c2, \'text\') > 0 for a literal substring. LIKE uses % for any sequence and _ for one character; it ignores ASCII case by default, but non-ASCII case is significant. instr is case sensitive. To match literal % or _, choose an escape character: LIKE \'%!%%\' ESCAPE \'!\' matches a percent sign.')
  paragraph(root, labels, 'Supported: read-only SELECT, WHERE with AND / OR / NOT, comparisons, IN, LIKE, CASE, CAST, DISTINCT, ORDER BY, LIMIT / OFFSET, GROUP BY and HAVING. Allowed aggregate functions include count, sum, avg, min and max. Other allowed functions include coalesce, nullif, lower, upper, length, instr, substr, replace, trim, round, abs, date, datetime, strftime, json_extract, json_valid and row_number. Function access is restricted; a SQLite function outside the allowlist is rejected.')
  paragraph(root, labels, 'Limits: one statement, 16 KiB SQL, 256 source/result columns, 5 seconds of query execution and 32 MiB of results. Preparation has a separate 15-second limit. Large expressions, cells and pages have additional budgets. No writes, PRAGMA, ATTACH, filesystem access or extensions. Narrow columns/rows if a limit is reached. Ctrl+Enter or Command+Enter runs the editor.')
  examples(root, labels, sqlExamples(columns), select)
}
export function jqHelp(root, labels, select) {
  paragraph(root, labels, 'jq-compatible filters run with jaq (core 3.1.0, std 3.0.1, JSON 2.0.1), not the jq command-line program. This embedded dialect includes standard and JSON filters, with environment and I/O facilities disabled; full jq CLI compatibility is not promised.')
  paragraph(root, labels, 'With slurp off, the filter runs once per top-level JSON value. A JSON array is one value: use .[] to iterate it. JSONL / NDJSON normally supplies one object per line: use select(...) directly on each record. The reader accepts a sequence of JSON values, not raw text lines.')
  paragraph(root, labels, 'With slurp on, all top-level input values become one array before filtering. JSONL objects become an array of records. An existing JSON array becomes a nested array containing that array; slurp does not flatten it. Sorting or aggregating across JSONL records requires slurp. Slurp uses memory for the complete input.')
  paragraph(root, labels, 'Use double quotes for strings and .["field name"] for keys containing spaces or punctuation. Operators include |, comparisons, arithmetic, // defaults, ? for optional access, if/then/else/end, arrays and object construction. Useful filters include select, map, keys, length, type, has, contains, sort_by, group_by, unique, add, tonumber, tostring, fromjson, tojson, test and capture. Check type and keys first if a field access or iteration fails.')
  paragraph(root, labels, 'Enter only a filter, without jq, shell quotes or command-line flags. There are no --arg bindings, input/inputs, env/$ENV, module imports, file, shell or network access. Slurp is the checkbox; raw-input/raw-output flags are unavailable. Results retain JSON types and strings are quoted; multiple outputs are displayed separately and paged, not combined into a new array.')
  paragraph(root, labels, 'Limits: 16 KiB filter, 128 MiB source, 5 seconds, 512 MiB worker RSS and source-scaled output from 8 to 256 MiB. Pages contain at most 48 KiB and may split a large value. Reduce input/output or simplify the filter after a budget error. Ctrl+Enter or Command+Enter runs the editor.')
  examples(root, labels, jqExamples, select)
}
export function queryError(error, dialect) {
  const raw = String(error?.message || error).slice(0, 4000)
  const clean = raw.replace(/^(?:Reader request failed|HTTP request failed|Request failed) \(\d{3}\):\s*/i, '').replace(/^HTTP \d{3}(?: [^:\n]+)?:\s*/i, '')
  let hint = ''
  if (dialect === 'SQL') {
    if (/no such table:\s*c\d+\b/i.test(clean)) hint = "IN expects a list or a table, not a text column. Use c2 IN ('first', 'second') for a list, c2 = 'text' for equality, or instr(c2, 'text') > 0 for a substring. Substitute your column alias."
    else if (/no such column/i.test(clean)) hint = 'Check Columns and query help: c1, c2, … require FROM t; quoted names require FROM data. Use single quotes for text values.'
    else if (/not authorized|prohibited|readonly|read-only/i.test(clean)) hint = 'Only read-only queries and allowed functions are available. Check the supported syntax in query help.'
    else if (/syntax error|incomplete input/i.test(clean)) hint = 'Check commas, parentheses and quotes near the reported position. Use one SELECT statement.'
    else if (/interrupted|budget|limit/i.test(clean)) hint = 'Select fewer columns or rows, add LIMIT, or simplify the query.'
  } else {
    if (/undefined|not defined/i.test(clean)) hint = 'Check the filter spelling and argument count. CLI flags, environment, input/inputs and module imports are unavailable.'
    else if (/Invalid jq filter/i.test(clean)) hint = 'Check brackets, pipes and double-quoted strings at the reported position. Enter a filter without jq or shell quoting.'
    else if (/evaluation failed/i.test(clean)) hint = 'Inspect the input with type or keys. Use .[] for an array, or filter each JSONL record directly; check the Slurp setting.'
  }
  return `${dialect} query failed: ${clean}${hint ? '\n' + hint : ''}`
}
