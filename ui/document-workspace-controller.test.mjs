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
  const scroll = source.slice(source.indexOf('function scrollActiveOutlineItemIntoView()'), source.indexOf('function scheduleTextArtifactOutline('))

  assert.match(scroll, /list\.scrollTop/u)
  assert.doesNotMatch(scroll, /scrollIntoView/u)
})

test('Document outline is transient and remains open after chapter navigation', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const navigate = source.slice(source.indexOf('async function navigateArtifactOutlineItem('), source.indexOf('function setArtifactOutlineActive('))

  assert.doesNotMatch(source, /ResizeObserver/u)
  assert.doesNotMatch(source, /classList\.contains\('compact'\)/u)
  assert.doesNotMatch(source, /resize: updateArtifactOutlineLayout/u)
  assert.doesNotMatch(navigate, /setArtifactOutlineOpen/u)
  assert.match(source, /function resetArtifactOutline\(\)[\s\S]*state\.artifactOutlineOpen = false/u)
})
