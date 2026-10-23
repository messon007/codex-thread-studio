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
