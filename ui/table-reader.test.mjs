import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { clampTableColumnWidth, parseTabularArtifact, resizeTableColumnWidths } from './table-reader.mjs'

const source = readFileSync(new URL('./table-reader.mjs', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('CSV parsing does not create a phantom row for a trailing newline', async () => {
  const workbook = await parseTabularArtifact({ path: 'metrics.csv', text: 'name,score\nalpha,82\n' })
  assert.deepEqual(workbook.sheets[0].rows, [['name', 'score'], ['alpha', '82']])
})

test('CSV parsing keeps quoted commas and escaped quotes', async () => {
  const workbook = await parseTabularArtifact({ path: 'notes.csv', text: 'name,note\nalpha,"hello, ""world"""' })
  assert.deepEqual(workbook.sheets[0].rows[1], ['alpha', 'hello, "world"'])
})

test('table column resizing is bounded and changes only the requested column', () => {
  assert.equal(clampTableColumnWidth(12), 82)
  assert.equal(clampTableColumnWidth(2_000), 720)
  assert.deepEqual(resizeTableColumnWidths([120, 180, 240], 1, 75), [120, 255, 240])
  assert.deepEqual(resizeTableColumnWidths([120, 180, 240], 0, -100), [82, 180, 240])
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
