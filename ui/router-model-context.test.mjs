import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

test('Composer model context follows the selected Router target and leaves Router settings intact', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function composerModelContext(')
  const end = source.indexOf('\nfunction ', start + 1)
  const state = { backend: 'codex', selectedId: 'router', routerRuntime: { selectedTarget: '' }, turnOptions: {
    'codex:router': { model: 'router-model' }, 'ept-codex:worker': { model: 'worker-model', effort: 'high' },
  } }
  const context = new Function('state', 'isRouterThread', 'sessionRefFromKey', 'selectedStateKey', 'defaultTurnOptions', 'threadForRef', 'backendDescriptor', `${source.slice(start, end)};return composerModelContext`)(
    state, () => true, key => { const [backend, id] = key.split(':'); return { backend, id } },
    (id, backend) => `${backend}:${id}`, () => ({}), ref => ({ model: `${ref.id}-native` }), backend => ({ name: backend, tag: backend }),
  )
  assert.equal(context().options.model, 'router-model')
  state.routerRuntime.selectedTarget = 'ept-codex:worker'
  assert.equal(context().options.model, 'worker-model')
  assert.equal(context().descriptor.tag, 'ept-codex')
  context().options.model = 'new-worker-model'
  assert.equal(state.turnOptions['ept-codex:worker'].model, 'new-worker-model')
  assert.equal(state.turnOptions['codex:router'].model, 'router-model')
  state.routerRuntime.selectedTarget = ''
  assert.equal(context().options.model, 'router-model')
})
