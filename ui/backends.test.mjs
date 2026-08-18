import test from 'node:test'
import assert from 'node:assert/strict'

import {
  BACKEND_IDS,
  backendDescriptor,
  defaultTurnOptions,
  emptyBackendCatalogs,
  installBackendRegistry,
  isCodexBackend,
  isSupportedBackend,
} from './backends.mjs'

test.afterEach(() => installBackendRegistry())

test('installs configured Codex-compatible instances without hard-coding EPT', () => {
  installBackendRegistry([
    { id: 'codex', name: 'Codex', tag: 'CX', kind: 'codex' },
    { id: 'work-codex', name: 'Work Codex', tag: 'WK', kind: 'codex', socketPath: '/ws/codex/work-codex' },
    { id: 'opencode', name: 'OpenCode', tag: 'OC', kind: 'opencode' },
  ])
  assert.deepEqual(BACKEND_IDS, ['codex', 'work-codex', 'opencode'])
  assert.equal(isCodexBackend('work-codex'), true)
  assert.equal(isSupportedBackend('private-codex'), false)
  assert.equal(backendDescriptor('work-codex').socketPath, '/ws/codex/work-codex')
  assert.equal(backendDescriptor('work-codex').tag, 'WK')
})

test('configured backend catalogs never alias built-in Codex sessions', () => {
  installBackendRegistry([
    { id: 'codex', name: 'Codex', tag: 'CX', kind: 'codex' },
    { id: 'company-codex', name: 'Company Codex', tag: 'CO', kind: 'codex' },
    { id: 'opencode', name: 'OpenCode', tag: 'OC', kind: 'opencode' },
  ])
  const catalogs = emptyBackendCatalogs()
  catalogs.codex.push({ id: 'same-id', model: 'gpt-5.6-sol' })
  catalogs['company-codex'].push({ id: 'same-id', model: 'company-model' })
  assert.notEqual(catalogs.codex, catalogs['company-codex'])
  assert.equal(catalogs.codex[0].model, 'gpt-5.6-sol')
  assert.equal(catalogs['company-codex'][0].model, 'company-model')
})

test('invalid registry entries cannot replace required builtins', () => {
  installBackendRegistry([{ id: 'Bad Id', kind: 'codex' }])
  assert.deepEqual(BACKEND_IDS, ['codex', 'opencode'])
  assert.equal(isCodexBackend('codex'), true)
})

test('only built-in Codex starts new session turn options at high effort', () => {
  installBackendRegistry([
    { id: 'codex', name: 'Codex', tag: 'CX', kind: 'codex' },
    { id: 'work-codex', name: 'Work Codex', tag: 'WK', kind: 'codex' },
    { id: 'opencode', name: 'OpenCode', tag: 'OC', kind: 'opencode' },
  ])
  assert.deepEqual(defaultTurnOptions('codex'), { effort: 'high' })
  assert.deepEqual(defaultTurnOptions('work-codex'), {})
  assert.deepEqual(defaultTurnOptions('opencode'), {})
})
