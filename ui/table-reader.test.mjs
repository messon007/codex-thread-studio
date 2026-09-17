import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { clampTableColumnWidth, parseTabularArtifact, resizeTableColumnWidths } from './table-reader.mjs'
import { defaultTableSql, tableSqlSource } from './table-data.mjs'

const source = readFileSync(new URL('./table-reader.mjs', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
const workspaceSource = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')

test('CSV parsing does not create a phantom row for a trailing newline', async () => {
  const workbook = await parseTabularArtifact({ path: 'metrics.csv', text: 'name,score\nalpha,82\n' })
  assert.deepEqual(workbook.sheets[0].rows, [['name', 'score'], ['alpha', '82']])
})

test('CSV parsing keeps quoted commas and escaped quotes', async () => {
  const workbook = await parseTabularArtifact({ path: 'notes.csv', text: 'name,note\nalpha,"hello, ""world"""' })
  assert.deepEqual(workbook.sheets[0].rows[1], ['alpha', 'hello, "world"'])
})

test('CSV parsing detects tab separators despite commas inside quoted fields', async () => {
  const workbook = await parseTabularArtifact({
    path: 'annotations.csv',
    text: '\uFEFFdocument\tquestion\r\nspec.md\t"Why, now?"\r\nother.md\t"How, exactly?"\r\n',
  })
  assert.deepEqual(workbook.sheets[0].rows, [
    ['document', 'question'],
    ['spec.md', 'Why, now?'],
    ['other.md', 'How, exactly?'],
  ])
})

test('CSV parsing detects consistent semicolon separators', async () => {
  const workbook = await parseTabularArtifact({ path: 'metrics.csv', text: 'name;score\nalpha;82\nbeta;91\n' })
  assert.deepEqual(workbook.sheets[0].rows, [['name', 'score'], ['alpha', '82'], ['beta', '91']])
})

test('table column resizing is bounded and changes only the requested column', () => {
  assert.equal(clampTableColumnWidth(12), 82)
  assert.equal(clampTableColumnWidth(2_000), 720)
  assert.deepEqual(resizeTableColumnWidths([120, 180, 240], 1, 75), [120, 255, 240])
  assert.deepEqual(resizeTableColumnWidths([120, 180, 240], 0, -100), [82, 180, 240])
})

test('SQL source uses the first row as unique column names', () => {
  assert.deepEqual(tableSqlSource([
    ['', 'Name', 'name', 'Score'],
    ['1', 'alpha', 'alias', '82'],
  ]), {
    columns: ['column_1', 'Name', 'name_2', 'Score'],
    rows: [['1', 'alpha', 'alias', '82']],
  })
})

test('default SQL lists every source column for direct editing', () => {
  assert.equal(defaultTableSql(['document', 'component_name', 'say "yes"']), [
    'SELECT',
    '  "document",',
    '  "component_name",',
    '  "say ""yes"""',
    'FROM data',
    'LIMIT 1000',
  ].join('\n'))
})

test('table reader resizes through column elements and exposes complete selected-cell copy', () => {
  assert.match(source, /<col data-table-column-width=/u)
  assert.match(source, /data-table-column-resizer=/u)
  assert.match(source, /window\.addEventListener\('mousemove', resizeColumn\)/u)
  assert.match(source, /navigator\.clipboard\.writeText\(selectedCell\.textContent \|\| ''\)/u)
  assert.match(styles, /\.table-grid\.columns-resized \{ table-layout: fixed; \}/u)
  assert.match(styles, /body\.resizing-table-column/u)
  assert.doesNotMatch(source, /table-cell-tooltip|scrollWidth/u)
})

test('delimited table reader exposes SQL querying and width-aware wrapping', () => {
  assert.match(source, /data-table-view="sql"/u)
  assert.match(source, /data-table-wrap/u)
  assert.match(source, /data-table-chart[^\n]+data-table-copy[^\n]+data-table-wrap/u)
  assert.match(styles, /\.table-toolbar \[data-table-chart\] \{ margin-left: auto; \}/u)
  assert.match(source, /table-query-composer[^\n]+data-table-sql[^\n]+data-table-run[^\n]+translate\('Query'\)/u)
  assert.match(source, /input\.style\.height = `\$\{input\.scrollHeight\}px`/u)
  assert.match(source, /event\.ctrlKey \|\| event\.metaKey/u)
  assert.doesNotMatch(source, /table-query-schema/u)
  assert.match(styles, /\.table-grid\.wrapped td \{[^}]*white-space: pre-wrap;[^}]*overflow-wrap: anywhere;/u)
  assert.match(styles, /\.table-query-view/u)
  assert.match(styles, /\.table-query-composer:focus-within/u)
  assert.match(styles, /\.table-query-composer textarea \{[^}]*overflow: hidden;[^}]*resize: none;/u)
  assert.match(workspaceSource, /gatewayFetch\('\/studio\/table\/query'/u)
})
