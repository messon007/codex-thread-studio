import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const reviewNotes = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')

test('dynamic session state loads beside preferences but is not written back to settings', () => {
  const loadStart = app.indexOf('async function loadPreferences()')
  const snapshotStart = app.indexOf('function preferencesSnapshot()', loadStart)
  const persistStart = app.indexOf('function persistPreferences()', snapshotStart)
  const load = app.slice(loadStart, snapshotStart)
  const snapshot = app.slice(snapshotStart, persistStart)

  assert.match(load, /Promise\.all\(\[/u)
  assert.match(load, /loadJson\('\/studio\/preferences'/u)
  assert.match(load, /loadJson\('\/studio\/session-state'/u)
  assert.doesNotMatch(snapshot, /annotationDrafts|annotationAdditional|openingMessages/u)
  assert.match(load, /state\.turnOptions = normalizeStoredTurnOptions\(storedSessionState\.turnOptions\)/u)
  assert.doesNotMatch(snapshot, /turnOptions/u)
})

test('startup selection comes from Thread Router instead of persisted session state', () => {
  const loadStart = app.indexOf('async function loadPreferences()')
  const snapshotStart = app.indexOf('function preferencesSnapshot()', loadStart)
  const persistStart = app.indexOf('function persistPreferences()', snapshotStart)
  const load = app.slice(loadStart, snapshotStart)
  const snapshot = app.slice(snapshotStart, persistStart)

  assert.match(load, /state\.router = normalizeThreadRouter\(saved\.router\)/u)
  assert.match(load, /state\.backend = isSupportedBackend\(state\.router\.controllerBackend\)/u)
  assert.match(load, /const routerId = state\.router\.controllers\[state\.backend\]/u)
  assert.doesNotMatch(load, /saved\.(?:selectedBackend|selectedThreads)/u)
  assert.doesNotMatch(snapshot, /\bselected(?:Backend|Threads)\s*:/u)
  assert.match(app, /state\.startupRouterSelectionPending[\s\S]{0,240}threadRouter\.ensureManagedSession\(backend\)/u)
})

test('session state mutations use bounded per-session endpoints', () => {
  assert.match(app, /'\/studio\/session-state\/annotations'/u)
  assert.match(app, /'\/studio\/session-state\/opening-message'/u)
  assert.match(app, /'\/studio\/session-state\/pin'/u)
  assert.match(app, /'\/studio\/session-state\/turn-options'/u)
  assert.match(app, /'\/studio\/session-state\/session',[^\n]*'DELETE'/u)
  assert.match(app, /sessionStateWriter\.write\(path, body, method\)/u)
  assert.match(app, /if \(!preferencesReady\) return Promise\.resolve\(\)/u)
})

test('session model and effort choices persist the concrete backend default', () => {
  assert.match(app, /persistSessionTurnOptions\(key\)/u)
  assert.match(app, /data-model-default/u)
  assert.match(app, /backendDefaultId[\s\S]{0,1200}state\.turnOptions\[key\] = options[\s\S]{0,120}persistSessionTurnOptions\(key\)/u)
  assert.doesNotMatch(app, /if \(useDefault\) \{[\s\S]{0,160}delete state\.turnOptions\[key\]/u)
  assert.match(app, /sessionModelPreferencePayload\(key, options\)/u)
  assert.match(app, /async function createThread[\s\S]*state\.turnOptions\[key\] = \{ \.\.\.defaultTurnOptions\(backend\), model \}[\s\S]*persistSessionTurnOptions\(key\)/u)
  assert.match(app, /async function forkThread[\s\S]*state\.turnOptions\[forkKey\] = sourceOptions[\s\S]*persistSessionTurnOptions\(forkKey\)/u)
})

test('pinning is bounded and archive clears the persisted pin', () => {
  assert.match(app, /state\.pinnedSessions\.size >= 10/u)
  assert.match(app, /state\.pinnedSessions = new Set\(\[\.\.\.state\.pinnedSessions, key\]\)/u)
  assert.match(app, /async function archiveSelectedThread\([\s\S]*persistSessionPin\(key, false\)/u)
  assert.match(app, /partitionPinnedCatalogEntries\([\s\S]{0,120}state\.pinnedSessions,[\s\S]{0,120}order: state\.filter === 'all' \? 'pin' : 'activity'/u)
})

test('debounced comment persistence stays bound to the session that changed', () => {
  assert.match(reviewNotes, /const key = selectedStateKey\(\)[\s\S]*setTimeout\(\(\) => persistAnnotationState\(key\), 300\)/u)
  assert.doesNotMatch(reviewNotes, /setTimeout\(persistAnnotationState/u)
})
