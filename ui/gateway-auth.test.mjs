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

test('authentication failure stops startup before binding controls or connecting backends', () => {
  const init = source.slice(source.indexOf('async function init()'), source.indexOf('async function loadBackendRegistry()'))
  assert.match(init, /if \(!window\.__CODEX_THREAD_STUDIO_GATEWAY__\)[\s\S]*showRemoteAuthenticationRequired\(\)[\s\S]*return/u)
  assert.ok(init.indexOf('await loadBackendRegistry() === false') < init.indexOf('bindUI()'))
  assert.match(source, /response\.status === 401 && window\.__CODEX_THREAD_STUDIO_GATEWAY__\?\.remote[\s\S]*showRemoteAuthenticationRequired\(\)[\s\S]*return false/u)
})
