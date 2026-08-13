import assert from 'node:assert/strict'
import test from 'node:test'
import { createTableCommentProvider, tableCommentSource } from './table-comment-provider.mjs'

test('table comments keep sheet and cell range anchors', async () => {
  const source = tableCommentSource({ root: '/work', filePath: 'data.xlsx', documentHash: 'hash', sheet: 'Revenue', range: 'B7' })
  const provider = createTableCommentProvider()
  assert.equal(provider.describe({ source }), 'data.xlsx · Revenue!B7')
  assert.match(provider.promptAnchor({ source }), /Revenue!B7/)
  let reopened
  await provider.reopen({ source }, { openTableSource: (anchor) => { reopened = anchor } })
  assert.equal(reopened.range, 'B7')
})
