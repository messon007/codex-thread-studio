import test from 'node:test'
import assert from 'node:assert/strict'
import { normalizeTranslationPreferences, normalizeContinueBehavior, normalizeTypography, normalizeAdditional, normalizeOpeningMessages } from './preference-normalization.mjs'
test('preferences retain backend defaults and reject unsafe model names', () => {
  const result = normalizeTranslationPreferences({ models: { codex: ' chosen ', opencode: 'bad\u0000model' }, engine: 'ollama' })
  assert.equal(result.models.codex, 'chosen')
  assert.equal(result.models.opencode, undefined)
  assert.equal(result.ollamaModel, 'gemma3:4b')
  assert.equal(normalizeContinueBehavior('bad'), 'sessionModelDraft')
  assert.equal(normalizeContinueBehavior('quickSend'), 'quickSend')
})
test('typography follows configured family/weight and bounds sizes', () => {
  const defaults = { uiFontFamily: 'sans', uiFontSize: 14, uiFontWeight: 400, contentFontSize: 14, codeFontFamily: 'mono', codeFontSize: 13, codeFontWeight: 400, highContrast: false }
  const result = normalizeTypography({ uiFontFamily: 'custom', uiFontWeight: 600, uiFontSize: 100, codeFontSize: -1 }, defaults)
  assert.equal(result.contentFontFamily, 'custom')
  assert.equal(result.contentFontWeight, 600)
  assert.equal(result.uiFontSize, 20)
  assert.equal(result.codeFontSize, 11)
})
test('persisted text normalization preserves Unicode limits and session ownership', () => {
  assert.deepEqual(normalizeAdditional({ x: 'note' }), { 'codex:x': 'note' })
  const messages = normalizeOpeningMessages({ x: { text: '\u4e2d'.repeat(10000), capturedAt: 'now' } })
  assert.ok(new TextEncoder().encode(messages['codex:x'].text).length <= 16 * 1024)
  assert.equal(messages['codex:x'].text.endsWith('\ufffd'), false)
  assert.deepEqual(normalizeOpeningMessages(null), {})
})
