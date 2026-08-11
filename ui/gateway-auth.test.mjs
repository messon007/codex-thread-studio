import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('all Studio transports use the injected gateway credential', () => {
  assert.match(source, /headers\.set\('Authorization', `Bearer \$\{gatewayToken\(\)\}`\)/u)
  assert.match(source, /codex-thread-studio\.auth\.\$\{gatewayToken\(\)\}/u)
  assert.match(source, /gatewayEventSource\('\/opencode\/global\/event'\)/u)
  assert.doesNotMatch(source, /new EventSource\(/u)
})
