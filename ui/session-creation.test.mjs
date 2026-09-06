import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

function functionSource(name, nextName) {
  const start = source.indexOf(`${name}(`)
  const end = source.indexOf(`\n${nextName}(`, start)
  assert.ok(start >= 0, `${name} must exist`)
  assert.ok(end > start, `${nextName} must follow ${name}`)
  return source.slice(start, end)
}

test('new sessions activate the authoritative thread/start result before closing the dialog', () => {
  const create = functionSource('async function createThread', 'async function forkThread')
  const started = create.indexOf("await rpc('thread/start', params)")
  const activate = create.indexOf('activateStartedThread(backend, startedThread)')
  const close = create.indexOf('closeNewThreadDialog()')

  assert.ok(started >= 0 && started < activate)
  assert.ok(activate < close)
  assert.doesNotMatch(create, /await loadThreads\(\)/u)
  assert.doesNotMatch(create, /selectThread\(result\.thread\.id/u)
  assert.match(create, /createdThreadId[\s\S]*if \(createdThreadId\) showError\(error\)/u)
})

test('started-session activation is cached, prepared, and selected without a resume', () => {
  const activate = functionSource('function activateStartedThread', 'async function interruptTurn')

  assert.ok(activate.indexOf('rememberStartedThread(') < activate.indexOf('cacheThreadModel('))
  assert.ok(activate.indexOf('cacheThreadModel(') < activate.indexOf('sessionDispatch.markPrepared('))
  assert.ok(activate.indexOf('sessionDispatch.markPrepared(') < activate.indexOf('selectThread('))
  assert.doesNotMatch(activate, /(?:rpc|dispatchBackendRpc)\([^\n]*thread\/resume/u)
  assert.match(activate, /reportSessionLifecycle\('selected'/u)
  assert.match(activate, /reportSessionLifecycle\('ready'/u)
})

test('all catalog installation paths retain unconfirmed started sessions', () => {
  const install = functionSource('function installBackendCatalog', 'async function refreshRouterCatalogs')
  const active = functionSource('function setActiveThreads', 'function sidebarThreadsForBackend')
  const remember = functionSource('function rememberStartedThread', 'function forgetStartedThread')
  const reconcile = functionSource('function reconcileCatalogWithStartedThreads', 'function scheduleStartedThreadCatalogConfirmation')

  assert.match(install, /reconcileCatalogWithStartedThreads\(backend, threads\)/u)
  assert.match(active, /reconcileCatalogWithStartedThreads\(backend, threads\)/u)
  assert.match(remember, /catalogRequestGenerations\.set\(backend/u)
  assert.match(remember, /mergeThreadIntoCatalog\(backend, thread\)/u)
  assert.match(reconcile, /currentCatalog\.find\(\(thread\) => thread\.id === entry\.thread\.id\)/u)
  assert.match(reconcile, /entry\.thread = \{ \.\.\.entry\.thread, \.\.\.current, turns: undefined \}/u)
  assert.match(reconcile, /reportSessionLifecycle\('catalog-retained'/u)
  assert.match(reconcile, /forgetStartedThread\(backend, threadId, 'catalog-confirmed'\)/u)
})

test('forks use the same authoritative activation path', () => {
  const fork = functionSource('async function forkThread', 'async function forkSelectedThread')
  assert.match(fork, /activateStartedThread\(sourceBackend, result\.thread/u)
  assert.doesNotMatch(fork, /await loadThreads\(\)/u)
  assert.doesNotMatch(fork, /selectThread\(result\.thread\.id/u)
})
