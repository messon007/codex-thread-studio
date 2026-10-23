import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createDocumentWorkspaceController, createDocumentWorkspaceState } from './document-workspace-controller.mjs'

function documentWorkspaceFixture(context, gatewayFetch, overrides = {}) {
  const elements = new Map()
  const element = (selector) => {
    if (!elements.has(selector)) {
      const classes = new Set()
      elements.set(selector, {
        dataset: {},
        classList: {
          add: (...values) => values.forEach((value) => classes.add(value)),
          remove: (...values) => values.forEach((value) => classes.delete(value)),
          contains: (value) => classes.has(value),
          toggle(value, force = !classes.has(value)) {
            if (force) classes.add(value)
            else classes.delete(value)
            return force
          },
        },
        setAttribute() {},
        replaceChildren() {},
        querySelectorAll: () => [],
      })
    }
    return elements.get(selector)
  }
  const previousDocument = Object.getOwnPropertyDescriptor(globalThis, 'document')
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { querySelector: element } })
  let cleanup = () => {}
  context.after(() => {
    cleanup()
    if (previousDocument) Object.defineProperty(globalThis, 'document', previousDocument)
    else delete globalThis.document
  })
  const state = createDocumentWorkspaceState()
  let threadKey = 'thread-1'
  let requestId = 0
  let markdownRenders = 0
  const errors = []
  const controller = createDocumentWorkspaceController({
    state,
    gatewayFetch,
    randomId: () => `request-${++requestId}`,
    reportError: error => errors.push(error),
    view: {
      selectedThread: () => ({ cwd: '/project' }),
      selectedStateKey: () => threadKey,
      activateRightWorkspace: (tool) => { state.activeRightWorkspace = tool },
      hideComposerMenu() {},
      closeActionMenus() {},
      disposeMarkdownImageAssets() {},
      disconnectMarkdownImageObserver() {},
      applyRightRailWidth() {},
      hideSelection() {},
      renderMarkdownDocument() { markdownRenders += 1; throw new Error('Unexpected Markdown rendering') },
      hydrateMarkdownImages() {},
      ...overrides,
    },
  })
  cleanup = () => controller.close({ restoreMap: false })
  return { state, controller, element, errors, markdownRenders: () => markdownRenders, selectThread: (key) => { threadKey = key } }
}

function assertDocumentError(fixture, message, markdownRenders = 0) {
  assert.equal(fixture.state.artifact.loading, false)
  assert.equal(fixture.state.artifact.error, message)
  assert.equal(fixture.element('#artifact-loading').classList.contains('hidden'), true)
  assert.equal(fixture.element('#artifact-error').classList.contains('hidden'), false)
  assert.equal(fixture.element('#artifact-error-message').textContent, message)
  assert.equal(fixture.markdownRenders(), markdownRenders)
}

test('large Markdown uses one retained frame without whole-document rendering and closes its lease', async (context) => {
  const closed = []
  const result = { root: '/project', path: 'large.md', size: 32 * 1024 * 1024, readOnly: true, largeMarkdown: { id: 'reader-1', url: '/markdown-reader/reader-1/?embedded=1#token' } }
  const fixture = documentWorkspaceFixture(context, async (url, options) => {
    if (url.endsWith('/close')) { closed.push(JSON.parse(options.body).id); return new Response('{}') }
    return new Response(JSON.stringify(result))
  })
  const frames = []
  document.createElement = () => {
    const frame = { setAttribute() {}, addEventListener() {}, remove() { this.removed = true } }
    frames.push(frame)
    return frame
  }
  await fixture.controller.open({ path: 'large.md' })
  assert.equal(fixture.state.artifact.kind, 'large-markdown')
  assert.equal(fixture.state.artifact.content, undefined)
  assert.equal(fixture.markdownRenders(), 0)
  assert.equal(fixture.element('#artifact-view-switch').classList.contains('hidden'), false)
  assert.equal(fixture.element('#artifact-edit').classList.contains('hidden'), true)
  assert.equal(frames.length, 1)
  assert.equal(frames[0].src, result.largeMarkdown.url)
  fixture.controller.render()
  fixture.controller.setView('edit')
  assert.equal(frames.length, 1)
  fixture.controller.close({ restoreMap: false })
  assert.deepEqual(closed, ['reader-1'])
  assert.equal(frames[0].removed, true)
})

test('late large Markdown metadata releases its service after the document closes', async (context) => {
  let resolveResponse
  const closed = []
  const fixture = documentWorkspaceFixture(context, async (url, options) => {
    if (url.endsWith('/close')) { closed.push(JSON.parse(options.body).id); return new Response('{}') }
    return new Promise(resolve => { resolveResponse = resolve })
  })
  const pending = fixture.controller.open({ path: 'large.md' })
  fixture.controller.close({ restoreMap: false })
  resolveResponse(new Response(JSON.stringify({ largeMarkdown: { id: 'stale-reader' } })))
  await pending
  assert.equal(fixture.state.artifact, null)
  assert.deepEqual(closed, ['stale-reader'])
})

test('document network failures clear Loading and display the error', async (context) => {
  const failure = new Error('Network connection closed')
  const fixture = documentWorkspaceFixture(context, async () => { throw failure })

  await assert.rejects(fixture.controller.open({ path: 'large.md' }), failure)
  assertDocumentError(fixture, failure.message)
})

test('document response decoding failures clear Loading and display the error', async (context) => {
  const failure = new SyntaxError('Invalid JSON response')
  const fixture = documentWorkspaceFixture(context, async () => ({ ok: true, json: async () => { throw failure } }))

  await assert.rejects(fixture.controller.open({ path: 'large.md' }), failure)
  assertDocumentError(fixture, failure.message)
})

test('binary document response failures clear Loading and display the error', async (context) => {
  const failure = new Error('Document download interrupted')
  const fixture = documentWorkspaceFixture(context, async () => ({ ok: true, arrayBuffer: async () => { throw failure } }))

  await assert.rejects(fixture.controller.open({ path: 'large.pdf' }), failure)
  assertDocumentError(fixture, failure.message)
})

test('Markdown rendering failures display an error without retrying the renderer', async (context) => {
  const result = { root: '/project', path: 'large.md', content: '# Document', size: 10, lineCount: 1 }
  const fixture = documentWorkspaceFixture(context, async () => new Response(JSON.stringify(result)))

  await fixture.controller.open({ path: 'large.md' })
  assertDocumentError(fixture, 'Unexpected Markdown rendering', 1)
  assert.equal(fixture.errors.length, 1)
})

test('oversized documents display the server rejection without Markdown rendering', async (context) => {
  const message = 'file exceeds the 5 MiB review limit'
  const fixture = documentWorkspaceFixture(context, async () => new Response(JSON.stringify({ error: { message } }), { status: 413 }))

  await assert.rejects(fixture.controller.open({ path: 'large.md' }), { message })
  assertDocumentError(fixture, message)
})

test('document opening waits for asynchronous Markdown parsing while keeping controls available', async (context) => {
  let resolveRendering
  let cancelled = 0
  const result = { root: '/project', path: 'large.md', content: '# Document', size: 10, lineCount: 1 }
  const fixture = documentWorkspaceFixture(context, async () => new Response(JSON.stringify(result)), {
    renderMarkdownDocument: () => new Promise(resolve => { resolveRendering = resolve }),
    cancelMarkdownDocument: () => { cancelled += 1 },
  })
  const pending = fixture.controller.open({ path: 'large.md' })
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(fixture.element('#artifact-loading').classList.contains('hidden'), false)
  assert.equal(fixture.element('#artifact-view-switch').classList.contains('hidden'), false)
  fixture.controller.close({ restoreMap: false })
  resolveRendering({ html: '<h1>Document</h1>', outline: [] })
  await pending
  assert.equal(cancelled, 2)
  assert.equal(fixture.state.artifact, null)
  assert.equal(fixture.element('#artifact-rail').classList.contains('hidden'), true)
  assert.equal(fixture.element('#artifact-content').innerHTML, undefined)
})

test('late Markdown results cannot replace a newer document', async (context) => {
  let resolveFirst
  const fixture = documentWorkspaceFixture(context, async (url, options) => {
    const { path } = JSON.parse(options.body)
    return new Response(JSON.stringify({ root: '/project', path, content: path, size: 10, lineCount: 1 }))
  }, {
    renderMarkdownDocument: source => source === 'first.md'
      ? new Promise(resolve => { resolveFirst = resolve })
      : Promise.resolve({ html: '<p>Second</p>', outline: [] }),
  })
  const first = fixture.controller.open({ path: 'first.md' })
  await new Promise(resolve => setImmediate(resolve))
  await fixture.controller.open({ path: 'second.md' })
  const current = fixture.state.artifact
  resolveFirst({ html: '<p>First</p>', outline: [] })
  await first
  assert.equal(fixture.state.artifact, current)
  assert.equal(fixture.element('#artifact-content').innerHTML, '<p>Second</p>')
})

test('Source and line navigation reuse worker outlines instead of lexing whole documents', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  assert.doesNotMatch(source, /extractMarkdownOutline/u)
  assert.match(source, /file\.markdownDocument\.result\.outline/u)
  assert.match(source, /await renderArtifact\(\)/u)
})

test('late document failures cannot overwrite a newer request', async (context) => {
  let rejectFirst
  const pending = new Promise((resolve, reject) => { rejectFirst = reject })
  let requests = 0
  const fixture = documentWorkspaceFixture(context, () => ++requests === 1 ? pending : Promise.reject(new Error('Second request failed')))
  const first = fixture.controller.open({ path: 'first.md' })
  await assert.rejects(fixture.controller.open({ path: 'second.md' }), { message: 'Second request failed' })
  const current = fixture.state.artifact
  rejectFirst(new Error('Stale failure'))
  await first

  assert.equal(fixture.state.artifact, current)
  assertDocumentError(fixture, 'Second request failed')
})

test('late document failures do not reopen closed documents', async (context) => {
  let rejectRequest
  const fixture = documentWorkspaceFixture(context, () => new Promise((resolve, reject) => { rejectRequest = reject }))
  const pending = fixture.controller.open({ path: 'large.md' })
  fixture.controller.close({ restoreMap: false })
  rejectRequest(new Error('Stale failure'))
  await pending

  assert.equal(fixture.state.artifact, null)
  assert.equal(fixture.element('#artifact-rail').classList.contains('hidden'), true)
})

test('late document failures cannot change another session', async (context) => {
  let rejectRequest
  const fixture = documentWorkspaceFixture(context, () => new Promise((resolve, reject) => { rejectRequest = reject }))
  const pending = fixture.controller.open({ path: 'large.md' })
  const current = fixture.state.artifact
  fixture.selectThread('thread-2')
  rejectRequest(new Error('Stale failure'))
  await pending

  assert.equal(fixture.state.artifact, current)
  assert.equal(current.error, undefined)
})

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
