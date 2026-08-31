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
})

test('session state mutations use bounded per-session endpoints', () => {
  assert.match(app, /'\/studio\/session-state\/annotations'/u)
  assert.match(app, /'\/studio\/session-state\/opening-message'/u)
  assert.match(app, /'\/studio\/session-state\/session',[^\n]*'DELETE'/u)
  assert.match(app, /const payload = JSON\.stringify\(body\)/u)
})

test('debounced comment persistence stays bound to the session that changed', () => {
  assert.match(reviewNotes, /const key = selectedStateKey\(\)[\s\S]*setTimeout\(\(\) => persistAnnotationState\(key\), 300\)/u)
  assert.doesNotMatch(reviewNotes, /setTimeout\(persistAnnotationState/u)
})
