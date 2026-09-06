import test from 'node:test'
import assert from 'node:assert/strict'
import { preferencesSnapshot } from './preferences-snapshot.mjs'
import { environmentSavePayload } from './environment-profile.mjs'

const environment = { root: '/project', configured: false, secrets: '', removeSecrets: [], variables: '', allowedHosts: '', cacheVariables: '', networkPolicy: 'restricted' }

test('preferences snapshot keeps the exact persisted field allowlist and excludes session data', () => {
  const fields = ['language', 'theme', 'contentWidth', 'hiddenSessionDirectories', 'sharedDocumentDirectories', 'sessionDirectoryIgnore', 'sidebarCollapsed', 'rightRailWidthRatio', 'typography', 'mermaid', 'markdown', 'translation', 'desktopNotifications', 'queueDepth', 'continueBehavior', 'browser', 'annotationPromptTemplates']
  const state = Object.fromEntries(fields.map(field => [field, { sentinel: field }]))
  Object.assign(state, { wsl: {}, router: { controllers: {}, fallbacks: [] }, annotationDrafts: { secret: true }, messageQueues: {}, turnOptions: {}, selectedId: 'do-not-save' })
  const result = preferencesSnapshot(state)
  assert.deepEqual(Object.keys(result).sort(), [...fields, 'wslDistribution', 'wslUser', 'wslCodexBinary', 'wslOpencodeBinary', 'router'].sort())
  for (const field of fields) assert.equal(result[field], state[field])
  assert.equal(result.wslDistribution, null)
  assert.equal(result.wslUser, null)
  assert.equal(result.wslCodexBinary, 'codex')
  assert.equal(result.wslOpencodeBinary, 'opencode')
  assert.equal(result.router, null)
  state.router.controllers.codex = 'router'
  state.wsl = { distribution: 'Ubuntu', user: 'user', codexBinary: '/bin/codex', opencodeBinary: '/bin/opencode' }
  const configured = preferencesSnapshot(state)
  assert.equal(configured.router, state.router)
  assert.equal(configured.wslDistribution, 'Ubuntu')
  assert.equal(configured.wslCodexBinary, '/bin/codex')
})

test('empty environment is skipped only when unconfigured and restricted', () => {
  assert.equal(environmentSavePayload(environment), null)
  assert.notEqual(environmentSavePayload({ ...environment, configured: true }), null)
  assert.equal(environmentSavePayload({ ...environment, networkPolicy: 'enabled' }).networkPolicy, 'enabled')
})

test('environment payload validates fields and preserves explicit secret removals and empty variables', () => {
  const input = { ...environment, secrets: 'TOKEN=value', removeSecrets: ['OLD_TOKEN'], variables: 'EMPTY=\nA=B', allowedHosts: 'EXAMPLE.COM,example.com', cacheVariables: 'CACHE=/tmp/cache' }
  assert.deepEqual(environmentSavePayload(input), {
    root: '/project', variables: { EMPTY: '', A: 'B' }, secrets: { TOKEN: 'value' }, removeSecrets: ['OLD_TOKEN'],
    networkPolicy: 'restricted', allowedHosts: ['example.com'], cacheVariables: { CACHE: '/tmp/cache' },
  })
  assert.throws(() => environmentSavePayload({ ...input, removeSecrets: ['invalid-name'] }), /Invalid secret name/)
  assert.throws(() => environmentSavePayload({ ...input, secrets: 'TOKEN=' }), /requires a value/)
  assert.throws(() => environmentSavePayload({ ...input, cacheVariables: 'CACHE=' }), /requires a value/)
})
