import assert from 'node:assert/strict'
import test from 'node:test'
import { formatEnvironmentLines, parseEnvironmentLines, parseHosts } from './environment-profile.mjs'

test('environment editor parses values without splitting embedded equals signs', () => {
  assert.deepEqual(parseEnvironmentLines('MODE=dev\nURL=https://example.test?a=b'), { MODE: 'dev', URL: 'https://example.test?a=b' })
  assert.equal(formatEnvironmentLines({ MODE: 'dev' }), 'MODE=dev')
  assert.throws(() => parseEnvironmentLines('BAD-NAME=value'), /Invalid/)
})

test('allowed hosts are normalized and deduplicated', () => {
  assert.deepEqual(parseHosts('API.Example.com, api.example.com\n*.openai.com'), ['api.example.com', '*.openai.com'])
})
