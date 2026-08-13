import assert from 'node:assert/strict'
import test from 'node:test'

import { parseTabularArtifact } from './table-reader.mjs'

test('CSV parsing does not create a phantom row for a trailing newline', async () => {
  const workbook = await parseTabularArtifact({ path: 'metrics.csv', text: 'name,score\nalpha,82\n' })
  assert.deepEqual(workbook.sheets[0].rows, [['name', 'score'], ['alpha', '82']])
})

test('CSV parsing keeps quoted commas and escaped quotes', async () => {
  const workbook = await parseTabularArtifact({ path: 'notes.csv', text: 'name,note\nalpha,"hello, ""world"""' })
  assert.deepEqual(workbook.sheets[0].rows[1], ['alpha', 'hello, "world"'])
})
