import assert from 'node:assert/strict'
import test from 'node:test'
import { createPdfCommentProvider, pdfCommentSource } from './pdf-comment-provider.mjs'

test('PDF comments keep page and normalized region anchors', async () => {
  const source = pdfCommentSource({ root: '/work', filePath: 'paper.pdf', documentHash: 'hash', page: 3, rects: [{ x: .1, y: .2, width: .3, height: .1 }] })
  const provider = createPdfCommentProvider()
  assert.equal(provider.describe({ source }), 'paper.pdf · page 3')
  assert.match(provider.promptAnchor({ source }), /page 3/)
  let reopened
  await provider.reopen({ source }, { openPdfSource: (anchor) => { reopened = anchor } })
  assert.equal(reopened.rects[0].x, .1)
})
