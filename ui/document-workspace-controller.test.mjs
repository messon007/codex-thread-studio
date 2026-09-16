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

test('line links keep rendered Markdown and HTML in Preview', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const start = source.indexOf('function jumpArtifactToLine(')
  const end = source.indexOf('\n  return {', start)
  const jump = source.slice(start, end)

  assert.match(jump, /state\.artifactView === 'preview'/u)
  assert.match(jump, /artifactPreviewLocation\(file, Number\(line\)\)/u)
  assert.match(jump, /navigateArtifactPreviewLocation\(file, previewLocation, Number\(line\), column\)/u)
  assert.ok(jump.indexOf("state.artifactView === 'preview'") < jump.indexOf("setArtifactView('source')"))
})

test('manual Source to Preview uses the same nearest-heading mapping', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const start = source.indexOf('function setArtifactView(')
  const end = source.indexOf('\nfunction toggleArtifactSearch(', start)
  const switchView = source.slice(start, end)

  assert.match(switchView, /artifactSourceLineAtViewport\(\)/u)
  assert.match(switchView, /artifactPreviewLocation\(file, sourceLine\)/u)
  assert.match(switchView, /if \(sourceLine != null && !previewLocation\) return/u)
  assert.match(switchView, /navigateArtifactPreviewLocation\(file, previewLocation, sourceLine\)/u)
})

test('late document rendering cannot reopen an inactive right workspace', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const start = source.indexOf('function renderArtifact()')
  const end = source.indexOf('\nfunction disposeArtifactEditor()', start)
  const render = source.slice(start, end)
  const inactiveGuard = render.indexOf("if (state.activeRightWorkspace !== 'document')")
  const showRail = render.indexOf("rail.classList.remove('hidden')")

  assert.ok(inactiveGuard >= 0)
  assert.ok(inactiveGuard < showRail)
  assert.match(render.slice(inactiveGuard, showRail), /rail\.classList\.add\('hidden'\)[\s\S]*return/u)
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
