import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createDocumentWorkspaceState } from './document-workspace-controller.mjs'

test('Document Workspace state factories isolate search and outline state', () => {
  const first = createDocumentWorkspaceState()
  const second = createDocumentWorkspaceState()

  first.artifactSearchMatches.push({ id: 'match-1' })
  first.artifactOutlineCollapsed.add('section-1')

  assert.deepEqual(second.artifactSearchMatches, [])
  assert.deepEqual([...second.artifactOutlineCollapsed], [])
  assert.equal(second.artifactView, 'preview')
  assert.equal(second.artifact, null)
})

test('Document outline keeps active-row scrolling inside its own list', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const scroll = source.slice(source.indexOf('function scrollActiveOutlineItemIntoView()'), source.indexOf('function ensureArtifactOutlineResizeObserver()'))

  assert.match(scroll, /list\.scrollTop/u)
  assert.doesNotMatch(scroll, /scrollIntoView/u)
})
