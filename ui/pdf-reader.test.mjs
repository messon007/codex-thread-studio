import assert from 'node:assert/strict'
import test from 'node:test'

import { extractPdfOutline } from './document-outline.mjs'

test('PDF outline resolves named and explicit destinations to pages', async () => {
  const first = { num: 10, gen: 0 }
  const second = { num: 20, gen: 0 }
  const document = {
    getOutline: async () => [
      { title: 'Start', dest: [first], items: [{ title: 'Details', dest: 'details', items: [] }] },
    ],
    getDestination: async (name) => name === 'details' ? [second] : null,
    getPageIndex: async (reference) => reference === first ? 0 : 4,
  }
  assert.deepEqual(await extractPdfOutline(document), [
    { id: 'pdf-1', label: 'Start', depth: 0, target: { kind: 'pdf', page: 1, destination: [first] } },
    { id: 'pdf-2', label: 'Details', depth: 1, target: { kind: 'pdf', page: 5, destination: [second] } },
  ])
})
