import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { resolveModelDisplay } from './model-display.mjs'

test('model display resolves one model and effort label without backend assumptions', () => {
  const models = [{ model: 'gpt-default', isDefault: true, defaultReasoningEffort: 'medium' }]
  assert.deepEqual(resolveModelDisplay({ models }), {
    model: 'gpt-default',
    effort: 'medium',
    label: 'gpt-default/medium',
  })
  assert.deepEqual(resolveModelDisplay({
    overrideModel: 'gpt-selected',
    overrideEffort: 'high',
    sessionModel: 'gpt-session',
    models,
  }), {
    model: 'gpt-selected',
    effort: 'high',
    label: 'gpt-selected/high',
  })
})

test('composer renders the model as status without a second model switch entry', () => {
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  assert.match(app, /resolveModelDisplay\(/u)
  assert.doesNotMatch(app, /composer-hint/u)
  assert.match(html, /<div id="composer-model" class="composer-model"/u)
  assert.doesNotMatch(html, /<button id="composer-model"/u)
  assert.doesNotMatch(html, /id="composer-hint"/u)
})
