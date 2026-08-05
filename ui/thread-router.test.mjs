import test from 'node:test'
import assert from 'node:assert/strict'
import {
  finalAgentText,
  normalizeThreadRouter,
  parseRouterDecision,
  routerCandidates,
  routerDecisionForTurn,
  routerDecisionSchema,
} from './thread-router.mjs'

test('normalizes router configuration and excludes the router itself', () => {
  assert.deepEqual(normalizeThreadRouter({
    threadId: 'router',
    responsibilities: {
      router: { description: 'do not retain' },
      learn: { description: 'books and learning', fallback: 'learning' },
      misc: { description: 'anything else', fallback: 'unknown' },
    },
  }), {
    threadId: 'router',
    responsibilities: {
      learn: { description: 'books and learning', fallback: 'fallback' },
      misc: { description: 'anything else', fallback: 'none' },
    },
  })
})

test('builds candidates from native thread metadata', () => {
  const candidates = routerCandidates({
    threadId: 'router',
    responsibilities: { learn: { description: 'Learning', fallback: 'fallback' } },
  }, [
    { id: 'router', name: 'Router' },
    { id: 'learn', name: 'Books', cwd: '/work/books' },
    { id: 'archived', archived: true },
  ], { 'codex:learn': { text: 'Read this book' } })
  assert.deepEqual(candidates, [{
    id: 'learn', title: 'Books', cwd: '/work/books', responsibility: 'Learning', fallback: 'fallback', openingMessage: 'Read this book',
  }])
})

test('validates structured routing decisions against the catalog', () => {
  const value = JSON.stringify({
    action: 'dispatch', targetThreadId: 'learn', forwardedPrompt: 'Explain chapter 2', reason: 'Book request', message: 'Sent',
  })
  assert.equal(parseRouterDecision(value, ['learn']).targetThreadId, 'learn')
  assert.throws(() => parseRouterDecision(value, ['other']), /outside the configured catalog/)
  assert.deepEqual(parseRouterDecision(JSON.stringify({
    action: 'clarify', targetThreadId: 'ignored', forwardedPrompt: 'ignored', reason: 'Ambiguous', message: 'Which project?',
  }), ['learn']), {
    action: 'clarify', targetThreadId: '', forwardedPrompt: '', reason: 'Ambiguous', message: 'Which project?',
  })
})

test('extracts a persisted router decision from an App Server turn', () => {
  const turn = { items: [{ type: 'agentMessage', text: '```json\n{"action":"dispatch","targetThreadId":"a","forwardedPrompt":"do it","reason":"match","message":"sent"}\n```' }] }
  assert.match(finalAgentText(turn), /targetThreadId/)
  assert.equal(routerDecisionForTurn(turn, ['a']).targetThreadId, 'a')
  assert.equal(routerDecisionSchema().additionalProperties, false)
})
