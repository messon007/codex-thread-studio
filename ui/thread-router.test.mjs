import test from 'node:test'
import assert from 'node:assert/strict'
import {
  catalogsWithSingleRouter,
  DEFAULT_FALLBACK_CONDITION,
  finalAgentText,
  isRouterSession,
  managedRouterThread,
  migrateLegacyResponsibilities,
  normalizeThreadRouter,
  parseRouterDecision,
  parseSessionRefKey,
  recoverManagedRouterCatalog,
  routerApplicationContext,
  routerCandidates,
  routerControllerRef,
  routerDecisionForTurn,
  routerDecisionSchema,
  routerDeveloperInstructions,
  sessionRefKey,
  shouldCreateManagedRouter,
} from './thread-router.mjs'

test('migrates the Codex-only Router configuration to backend-neutral session keys', () => {
  assert.deepEqual(normalizeThreadRouter({
    threadId: 'router',
    responsibilities: {
      router: { description: 'do not retain' },
      learn: { description: 'books and learning', fallback: 'learning' },
      'opencode:misc': { description: 'anything else', fallback: 'unknown' },
    },
  }), {
    controllerBackend: 'codex',
    controllers: { codex: 'router' },
    fallbacks: [{ sessionKey: 'codex:learn', condition: DEFAULT_FALLBACK_CONDITION }],
  })
  assert.deepEqual(migrateLegacyResponsibilities({}, {
    threadId: 'router',
    responsibilities: {
      router: { description: 'do not retain' },
      learn: { description: 'books and learning', fallback: 'learning' },
      'opencode:misc': { description: 'anything else' },
    },
  }), {
    'codex:learn': { responsibility: 'books and learning' },
    'opencode:misc': { responsibility: 'anything else' },
  })
})

test('keeps at most three unique fallback targets with explicit conditions', () => {
  assert.deepEqual(normalizeThreadRouter({
    fallbacks: [
      { sessionKey: 'codex:a', condition: 'A condition' },
      { sessionKey: 'codex:a', condition: 'duplicate' },
      { sessionKey: 'opencode:b', condition: '' },
      { sessionKey: 'codex:c', condition: 'C condition' },
      { sessionKey: 'codex:d', condition: 'ignored fourth unique target' },
    ],
  }).fallbacks, [
    { sessionKey: 'codex:a', condition: 'A condition' },
    { sessionKey: 'opencode:b', condition: DEFAULT_FALLBACK_CONDITION },
    { sessionKey: 'codex:c', condition: 'C condition' },
  ])
})

test('measures responsibility and fallback limits in Unicode characters', () => {
  const chinese = '会'.repeat(4097)
  assert.equal(normalizeThreadRouter({
    fallbacks: [{ sessionKey: 'codex:a', condition: chinese }],
  }).fallbacks[0].condition, '会'.repeat(4096))
  assert.equal(migrateLegacyResponsibilities({}, {
    responsibilities: { 'codex:a': { description: chinese } },
  })['codex:a'].responsibility, '会'.repeat(4096))
})

test('uses stable backend-qualified session references', () => {
  assert.equal(sessionRefKey('codex', 'same'), 'codex:same')
  assert.deepEqual(parseSessionRefKey('opencode:same'), { backend: 'opencode', id: 'same', key: 'opencode:same' })
  const config = normalizeThreadRouter({ controllerBackend: 'opencode', controllers: { codex: 'cx', opencode: 'oc' } })
  assert.deepEqual(routerControllerRef(config), { backend: 'opencode', id: 'oc', key: 'opencode:oc' })
  assert.equal(isRouterSession(config, 'codex', 'cx'), true)
})

test('shows exactly the active Router controller while retaining both native histories', () => {
  const catalogs = {
    codex: [{ id: 'router-cx', name: 'Thread Router' }, { id: 'work-cx' }],
    opencode: [{ id: 'router-oc', name: 'Thread Router' }, { id: 'work-oc' }],
  }
  const controllers = { codex: 'router-cx', opencode: 'router-oc' }
  assert.deepEqual(catalogsWithSingleRouter({ controllerBackend: 'codex', controllers }, catalogs), {
    codex: catalogs.codex,
    opencode: [{ id: 'work-oc' }],
  })
  assert.deepEqual(catalogsWithSingleRouter({ controllerBackend: 'opencode', controllers }, catalogs), {
    codex: [{ id: 'work-cx' }],
    opencode: catalogs.opencode,
  })
})

test('recovers a managed router omitted from the bounded catalog', () => {
  const catalog = [{ id: 'regular', cwd: '/work/project' }]
  const recovered = { id: 'router', name: 'Thread Router', cwd: '/studio/router' }
  assert.equal(managedRouterThread(catalog, 'router', '/studio/router'), null)
  assert.deepEqual(recoverManagedRouterCatalog(catalog, 'router', '/studio/router', recovered), [recovered, ...catalog])
  assert.equal(recoverManagedRouterCatalog(catalog, 'router', '/studio/router', { ...recovered, cwd: '/wrong' }), catalog)
})

test('creates a managed router only when no Router identity is configured', () => {
  assert.equal(shouldCreateManagedRouter(null), true)
  assert.equal(shouldCreateManagedRouter(''), true)
  assert.equal(shouldCreateManagedRouter('router-id'), false)
})

test('builds candidates across backends and excludes every Router controller', () => {
  const candidates = routerCandidates({
    controllerBackend: 'codex',
    controllers: { codex: 'router-cx', opencode: 'router-oc' },
    fallbacks: [{ sessionKey: 'codex:learn', condition: 'No regular learning session matches.' }],
  }, {
    codex: [{ id: 'router-cx' }, { id: 'learn', name: 'Books', cwd: '/work/books' }],
    opencode: [{ id: 'router-oc' }, { id: 'learn', name: 'OC Books', cwd: '/work/oc-books' }],
  }, {
    'codex:learn': { text: 'Read this book', responsibility: 'Learning' },
    'opencode:learn': { responsibility: 'OpenCode learning' },
  })
  assert.deepEqual(candidates, [
    { key: 'codex:learn', backend: 'codex', id: 'learn', title: 'Books', cwd: '/work/books', responsibility: 'Learning', fallback: 'fallback', fallbackCondition: 'No regular learning session matches.', openingMessage: 'Read this book' },
    { key: 'opencode:learn', backend: 'opencode', id: 'learn', title: 'OC Books', cwd: '/work/oc-books', responsibility: 'OpenCode learning', fallback: 'none', fallbackCondition: '', openingMessage: '' },
  ])
})

test('validates structured routing decisions against backend-qualified keys', () => {
  const value = JSON.stringify({
    action: 'dispatch', targetSessionKey: 'opencode:learn', forwardedPrompt: 'Explain chapter 2', reason: 'Book request', message: 'Sent',
  })
  assert.equal(parseRouterDecision(value, ['codex:learn', 'opencode:learn']).targetSessionKey, 'opencode:learn')
  assert.throws(() => parseRouterDecision(value, ['codex:learn']), /outside the configured catalog/)
  const legacyClarify = JSON.stringify({
    action: 'clarify', targetSessionKey: 'ignored', forwardedPrompt: 'ignored', reason: 'Ambiguous', message: 'Which project?',
  })
  assert.throws(() => parseRouterDecision(legacyClarify, ['codex:learn']), /unsupported action/)
  assert.deepEqual(parseRouterDecision(legacyClarify, ['codex:learn'], { allowLegacyClarify: true }), {
    action: 'clarify', targetSessionKey: '', forwardedPrompt: '', reason: 'Ambiguous', message: 'Which project?',
  })
})

test('reads legacy persisted decisions only when the session id is unambiguous', () => {
  const turn = { items: [{ type: 'agentMessage', text: '```json\n{"action":"dispatch","targetThreadId":"a","forwardedPrompt":"do it","reason":"match","message":"sent"}\n```' }] }
  assert.match(finalAgentText(turn), /targetThreadId/)
  assert.equal(routerDecisionForTurn(turn, ['codex:a'])?.targetSessionKey, 'codex:a')
  assert.equal(routerDecisionForTurn(turn, ['codex:a', 'opencode:a']), null)
  assert.equal(routerDecisionSchema().properties.targetSessionKey.type, 'string')
  assert.deepEqual(routerDecisionSchema(['codex:learn', 'opencode:learn']).properties.targetSessionKey.enum, ['codex:learn', 'opencode:learn'])
  assert.deepEqual(routerDecisionSchema(['codex:learn']).properties.action.enum, ['dispatch'])
})

test('requires the controller to copy a catalog session key verbatim', () => {
  const instructions = routerDeveloperInstructions([{
    key: 'codex:signal-design', backend: 'codex', id: 'native-id', title: 'Signal design', cwd: '/work', responsibility: 'Formal signal design', fallback: 'none', openingMessage: '',
  }])
  assert.match(instructions, /copied byte-for-byte/)
  assert.match(instructions, /Never ask the user to clarify/)
  assert.match(instructions, /"sessionKey": "codex:signal-design"/)
  assert.deepEqual(routerApplicationContext([{
    key: 'codex:signal-design', backend: 'codex', id: 'native-id', title: 'Signal design', cwd: '/work', responsibility: 'Formal signal design', fallback: 'none', openingMessage: '',
  }]), {
    'codex-thread-studio/thread-router': {
      kind: 'application',
      value: instructions,
    },
  })
})
