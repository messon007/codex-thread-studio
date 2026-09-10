import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const source = readFileSync(new URL('./thread-router-controller.mjs', import.meta.url), 'utf8')
const start = source.indexOf('  let targetNavigationSequence =')
const end = source.indexOf('\n  function removeSession', start)
function fixture(select) {
  const state = { backend: 'codex', selectedId: 'router' }, frames = [], revealed = []
  const open = new Function('state', 'selectThread', 'requestAnimationFrame', 'view', `${source.slice(start, end)};return openTarget`)(
    state, (id, options) => select(state, id, options), cb => frames.push(cb), { revealTargetTurn: id => revealed.push(id) },
  )
  return { state, frames, revealed, open, link: { dataset: { routerTarget: 'rico', routerBackend: 'codex', routerTurn: 'old-turn' } } }
}
test('Open response reveals the target through scoped Turn navigation', async () => {
  const f = fixture((state, id, { backend }) => Object.assign(state, { selectedId: id, backend }))
  await f.open(f.link)
  f.frames.shift()()
  assert.deepEqual(f.revealed, ['old-turn'])
  assert.doesNotMatch(source.slice(start, end), /scrollIntoView|document\.querySelector/)
})
test('Open response does not scroll when selection was cancelled or superseded', async () => {
  const f = fixture(() => {})
  await f.open(f.link)
  f.frames.shift()()
  assert.deepEqual(f.revealed, [])
  f.state.selectedId = 'rico'
  await f.open(f.link)
  f.state.backend = 'opencode'
  f.frames.shift()()
  assert.deepEqual(f.revealed, [])
})
test('Only the last Open response navigation may reveal a Turn', async () => {
  const f = fixture((state, id) => { state.selectedId = id })
  await f.open(f.link)
  await f.open({ dataset: { ...f.link.dataset, routerTurn: 'new-turn' } })
  f.frames.forEach(cb => cb())
  assert.deepEqual(f.revealed, ['new-turn'])
})
