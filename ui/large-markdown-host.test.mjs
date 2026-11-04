import assert from 'node:assert/strict'
import test from 'node:test'
import { createLargeMarkdownHost } from './large-markdown-host.mjs'

test('host reuses paged reading controls without rebuilding an outline or rendering document text', async context => {
  const previous = globalThis.MutationObserver
  let changed
  let disconnected = false
  globalThis.MutationObserver = class {
    constructor(callback) { changed = callback }
    observe() {}
    disconnect() { disconnected = true }
  }
  context.after(() => { globalThis.MutationObserver = previous })
  let outlineOpen = false
  let mode = 'preview'
  let dirty = false
  let saveSucceeds = true
  let left = false
  const navigated = []
  const reader = {
    index: { toc: [{ id: 0 }], sourceLines: 100000 },
    source: { visible: false, async show() { this.visible = true; changed() }, go: line => navigated.push(line) },
    jump: (...location) => navigated.push(location),
  }
  const buttons = {
    '#contents-toggle': { getAttribute: () => String(outlineOpen), click() { outlineOpen = !outlineOpen; changed() } },
    '#source-mode': {},
    '#preview-mode': { click() { reader.source.visible = false; changed() } },
  }
  const bridge = {
    get state() { return { ready: true, readingReady: true, editable: true, view: mode, dirty, busy: false, outlineOpen, hasOutline: true, lineCount: 100000, documentId: 'current-document', status: saveSucceeds ? '' : 'The file changed outside Studio' } },
    async setView(view) { mode = view; reader.source.visible = view === 'source'; changed() },
    navigate(match) { if (mode === 'preview') return reader.jump(match.page, null, match.tableRow); return reader.source.go(match.line) },
    async save() { if (saveSucceeds) dirty = false; return saveSucceeds },
    leave() { left = true },
  }
  const snapshots = []
  const frame = { contentWindow: { readerExperiment: reader, marknoriaHost: bridge }, contentDocument: { querySelector: selector => buttons[selector] } }
  const host = createLargeMarkdownHost(frame, state => snapshots.push(state))
  context.after(() => host.dispose())
  assert.equal(snapshots[0].lineCount, 100000)
  await host.setView('source')
  assert.equal(snapshots.at(-1).view, 'source')
  host.navigateMatch({ line: 4242, page: 3 })
  assert.equal(navigated.at(-1), 4242)
  await host.setView('preview')
  host.setOutline(true)
  assert.equal(snapshots.at(-1).outlineOpen, true)
  host.navigateMatch({ line: 4242, page: 3, tableRow: 50 })
  assert.deepEqual(navigated.at(-1), [3, null, 50])
  await host.setView('edit')
  dirty = true
  assert.equal(host.dirty, true)
  host.navigateMatch({ line: 50 })
  assert.equal(navigated.at(-1), 50)
  saveSucceeds = false
  await assert.rejects(host.save(), /changed outside/)
  assert.equal(host.dirty, true)
  saveSucceeds = true
  assert.equal(await host.save(), true)
  assert.equal(snapshots.at(-1).dirty, false)
  host.dispose()
  assert.equal(disconnected, true)
  assert.equal(left, true)
})

test('iframe selections publish translated screen coordinates and detach listeners', context => {
  const previous = globalThis.MutationObserver
  globalThis.MutationObserver = class { observe() {} disconnect() {} }
  context.after(() => { globalThis.MutationObserver = previous })
  const listeners = new Map(), selections = []
  const article = { dataset: { page: '0' } }
  const element = { nodeType: 1, closest(selector) {
    if (selector === '#viewport, #source-view') return this
    if (selector === '[data-page]') return article
    return null
  } }
  const range = { startContainer: element, endContainer: element,
    getBoundingClientRect: () => ({ left: 5, right: 45, top: 10, bottom: 30, width: 40, height: 20 }) }
  const selection = { rangeCount: 1, isCollapsed: false, toString: () => 'Selected \u4e2d\u6587', getRangeAt: () => range }
  const frame = { addEventListener() {}, removeEventListener() {}, getBoundingClientRect: () => ({ left: 100, top: 200 }),
    contentDocument: { querySelector: () => null, addEventListener: (type, callback) => listeners.set(type, callback), removeEventListener: type => listeners.delete(type) },
    contentWindow: { marknoriaHost: { state: { ready: true }, leave() {} }, getSelection: () => selection,
      readerExperiment: { index: { pages: [{ line: 10 }, { line: 25 }], sourceLines: 100, revision: 'revision' } } } }
  const host = createLargeMarkdownHost(frame, () => {}, value => selections.push(value))
  context.after(() => host.dispose())
  listeners.get('mouseup')()
  assert.equal(selections[0].quote, 'Selected \u4e2d\u6587')
  assert.equal(selections[0].startLine, 10)
  assert.equal(selections[0].endLine, 25)
  assert.equal(selections[0].rect.left, 105)
  assert.equal(selections[0].rect.top, 210)
  host.dispose()
  assert.equal(listeners.size, 0)
})
