import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { SessionWorkspaceMemory } from './selection-coordinator.mjs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const source = app.slice(app.indexOf('const sessionWorkspaceMemory ='), app.indexOf('function resetStreamingPatches()'))
function fixture() {
  let key = 'codex:a', workspace = null, cancelled = false
  const nodes = new Map(), calls = []
  const $ = selector => {
    if (!nodes.has(selector)) {
      const classes = new Set(['hidden'])
      nodes.set(selector, { classList: { contains: x => classes.has(x), add: x => classes.add(x), remove: x => classes.delete(x) }, scrollTop: 0, scrollLeft: 0 })
    }
    return nodes.get(selector)
  }
  const state = { artifact: null, artifactOutlineCollapsed: new Set(), artifactView: 'preview' }
  const hide = () => { for (const node of nodes.values()) node.classList.add('hidden'); workspace = null }
  const tools = { snapshot: () => workspace, open: async tool => { workspace = { tool, scroll: [] }; calls.push(tool) }, openForSession: async (thread, backend, tool) => { calls.push({ thread, backend, tool }) } }
  const documentWorkspace = {
    close: () => { if (cancelled) return false; state.artifact = null; return true },
    open: async (file, options) => { calls.push({ file, options }); state.artifact = file; $('#artifact-rail').classList.remove('hidden') },
  }
  const api = new Function('SessionWorkspaceMemory', 'selectedStateKey', '$', 'state', 'workspaceTools', 'documentWorkspace', 'sessionResources', 'activateRightWorkspace', 'threadForRef', 'reviewNotes', 'openGlobalBrowser', 'sessionMap', 'showError', `${source}; return {leaveSessionWorkspace, restoreSessionWorkspace}`)(
    SessionWorkspaceMemory, () => key, $, state, tools, documentWorkspace,
    { isOpen: () => false, close() {}, open() {} }, hide, ref => ref, {}, async () => {}, { render() {} }, error => { throw error },
  )
  return { ...api, state, $, calls, select: value => { key = value }, cancel: value => { cancelled = value }, workspace: value => { workspace = value } }
}

test('session switch restores document descriptor, reading position and backend-isolated tools', async () => {
  const f = fixture()
  f.state.artifact = { root: '/project', path: 'book.pdf', page: 7, bytes: new Uint8Array(100) }
  f.$('#artifact-rail').classList.remove('hidden')
  f.$('#artifact-content').scrollTop = 130
  assert.equal(f.leaveSessionWorkspace(), true)
  f.select('opencode:a')
  assert.equal(f.restoreSessionWorkspace(), false)
  f.workspace({ tool: 'terminal', source: null, scroll: [] })
  f.leaveSessionWorkspace()
  f.select('codex:a')
  assert.equal(f.restoreSessionWorkspace(), true)
  assert.equal(f.calls[0].options.viewState.page, 7)
  assert.equal(f.calls[0].options.viewState.scrollTop, 130)
  assert.equal('bytes' in f.calls[0].file, false)
  assert.equal(f.restoreSessionWorkspace(), false)
  f.leaveSessionWorkspace()
  f.select('opencode:a')
  f.restoreSessionWorkspace()
  assert.equal(f.calls.at(-1), 'terminal')
})

test('cancelled dirty document close leaves visible workspace untouched', () => {
  const f = fixture()
  const file = { root: '/project', path: 'draft.md', dirty: true }
  f.state.artifact = file
  f.$('#artifact-rail').classList.remove('hidden')
  f.cancel(true)
  assert.equal(f.leaveSessionWorkspace(), false)
  assert.equal(f.state.artifact, file)
  assert.equal(f.$('#artifact-rail').classList.contains('hidden'), false)
})

test('restored Files panel keeps its scroll and routed source session', async () => {
  const f = fixture()
  f.workspace({ tool: 'files', source: { backend: 'ept-codex', id: 'worker' }, scroll: [] })
  f.leaveSessionWorkspace()
  f.select('codex:b')
  f.leaveSessionWorkspace()
  f.select('codex:a')
  f.restoreSessionWorkspace()
  assert.deepEqual(f.calls[0], { thread: { backend: 'ept-codex', id: 'worker' }, backend: 'ept-codex', tool: 'files' })
})
