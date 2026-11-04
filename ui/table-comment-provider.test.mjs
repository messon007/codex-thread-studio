import assert from 'node:assert/strict'
import test from 'node:test'
import { createTableCommentProvider, tableCommentSource } from './table-comment-provider.mjs'
import { CommentSourceRegistry, normalizeCommentDrafts } from './comment-core.mjs'

test('table comments keep sheet and cell range anchors', async () => {
  const source = tableCommentSource({ root: '/work', filePath: 'data.xlsx', documentHash: 'hash', sheet: 'Revenue', range: 'B7' })
  const provider = createTableCommentProvider()
  assert.equal(provider.describe({ source }), 'data.xlsx · Revenue!B7')
  assert.match(provider.promptAnchor({ source }), /Revenue!B7/)
  let reopened
  await provider.reopen({ source }, { openTableSource: (anchor) => { reopened = anchor } })
  assert.equal(reopened.range, 'B7')
})

test('native result metadata survives web normalization, source reopening and prompt construction', async () => {
  const sql = "SELECT 'a  b' AS value\r\nFROM data"
  const source = tableCommentSource({ root: '/work', filePath: 'data.csv', documentHash: 'hash', sheet: 'Data', range: 'A1', header: 'value', sql, delimiter: ';' })
  const provider = createTableCommentProvider()
  const registry = new CommentSourceRegistry().register(provider)
  const [draft] = normalizeCommentDrafts({ 'codex:t': [{ id: 'native', excerpt: 'a  b', source }] }, { registry })['codex:t']
  assert.equal(draft.source.anchor.sql, sql)
  assert.equal(draft.source.anchor.delimiter, ';')
  assert.equal(draft.source.anchor.header, 'value')
  assert.match(provider.promptAnchor(draft), /SQL result cell/)
  const instruction = provider.promptInstruction(draft, { translate: (value) => value })
  assert.ok(instruction.includes(JSON.stringify(sql)))
  let reopened
  await provider.reopen(draft, { openTableSource: (anchor) => { reopened = anchor } })
  assert.equal(reopened.sql, sql)
  assert.equal(reopened.delimiter, ';')
})
