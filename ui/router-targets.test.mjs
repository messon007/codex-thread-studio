import test from 'node:test'
import assert from 'node:assert/strict'
import { rankRouterTargets, routerMention } from './router-targets.mjs'

const candidates = [
  { key: 'codex:a', title: 'Studio design', cwd: '/studio', responsibility: 'Interface design', openingMessage: '' },
  { key: 'opencode:a', title: '\u8bfb\u4e66\u5b66\u4e60', cwd: '/books', responsibility: '\u6309\u7ae0\u8282\u5b66\u4e60\u4e66\u7c4d', openingMessage: '' },
]

test('Router mentions preserve the question and do not mistake email addresses for targets', () => {
  assert.deepEqual(routerMention('Explain this @stu', 17), { start: 13, end: 17, query: 'stu' })
  assert.equal(routerMention('me@example.com', 14), null)
  assert.deepEqual(routerMention('@', 1), { start: 0, end: 1, query: '' })
})

test('Router suggestions rank question context and preserve backend-qualified identity', () => {
  assert.equal(rankRouterTargets(candidates, '', '\u5b66\u4e60\u4e0b\u4e00\u7ae0\u8282')[0].key, 'opencode:a')
  assert.deepEqual(rankRouterTargets(candidates, 'std').map(item => item.key), ['codex:a'])
  assert.deepEqual(rankRouterTargets(candidates, 'books').map(item => item.key), ['opencode:a'])
  assert.equal(rankRouterTargets(candidates, 'missing').length, 0)
  assert.deepEqual(rankRouterTargets(candidates, '').map(item => item.key), ['codex:a', 'opencode:a'])
})
