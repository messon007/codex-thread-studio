import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  addLoadedThread,
  updateLoadedCatalogTimestamp,
} from './thread-workset.mjs'

test('loaded workset keeps equal IDs from different backends and is idempotent', () => {
  const loaded = new Set()
  assert.equal(addLoadedThread(loaded, 'codex', 'same-id'), true)
  assert.equal(addLoadedThread(loaded, 'opencode', 'same-id'), true)
  assert.equal(addLoadedThread(loaded, 'codex', 'same-id'), false)
  assert.deepEqual([...loaded], ['codex:same-id', 'opencode:same-id'])
})

test('only loaded sessions receive a dynamic catalog timestamp', () => {
  const catalogs = {
    codex: [{ id: 'loaded', updatedAt: 10 }, { id: 'cold', updatedAt: 20 }],
    opencode: [],
  }
  const loaded = new Set(['codex:loaded'])
  assert.equal(updateLoadedCatalogTimestamp(catalogs, loaded, 'codex', 'cold', 30), false)
  assert.equal(updateLoadedCatalogTimestamp(catalogs, loaded, 'codex', 'loaded', 40), true)
  assert.deepEqual(catalogs.codex.map(({ updatedAt }) => updatedAt), [40, 20])
})

test('Studio starts with an empty runtime workset and adds a session after loading history', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /state\.attentionThreads = new Set\(\)/)
  assert.doesNotMatch(source, /attentionThreads: \[\.\.\.state\.attentionThreads\]/)
  const resume = source.slice(
    source.indexOf('async function resumeThreadUncached('),
    source.indexOf('async function refreshSelectedThread('),
  )
  assert.ok(resume.indexOf('cacheThreadModel(') > resume.indexOf("rpc('thread/resume'"))
  assert.match(source, /function cacheThreadModel[\s\S]*markThreadLoaded\(backend, id\)/)
})

test('completed replies update timestamps without changing loaded membership', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /function updateCodexReplyTime[\s\S]*message\.method !== 'turn\/completed'[\s\S]*isCodexBackend\(state\.backend\)[\s\S]*updateLoadedThreadTimestamp\(backend/)
  assert.match(source, /function updateOpenCodeReplyTime[\s\S]*payload\.type !== 'session\.idle'[\s\S]*updateLoadedThreadTimestamp\('opencode'/)
})
