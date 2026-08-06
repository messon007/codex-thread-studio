import test from 'node:test'
import assert from 'node:assert/strict'

import {
  MERMAID_PREFERENCES_DEFAULTS,
  mermaidInitializeConfig,
  normalizeMermaidPreferences,
} from './mermaid-config.mjs'

test('Mermaid preferences use documented safe defaults', () => {
  assert.deepEqual(normalizeMermaidPreferences(null), MERMAID_PREFERENCES_DEFAULTS)
  assert.deepEqual(normalizeMermaidPreferences({ style: 'unknown', fontSize: 99 }), MERMAID_PREFERENCES_DEFAULTS)
})

test('Mermaid configuration maps appearance controls without relaxing safety', () => {
  const config = mermaidInitializeConfig({
    style: 'handDrawn', density: 'compact', curve: 'step', layout: 'elk', fontSize: 16,
  }, { dark: true, fontFamily: 'Inter' })

  assert.equal(config.theme, 'dark')
  assert.equal(config.look, 'handDrawn')
  assert.equal(config.layout, 'elk')
  assert.equal(config.fontSize, 16)
  assert.equal(config.fontFamily, 'Inter')
  assert.equal(config.flowchart.curve, 'step')
  assert.equal(config.flowchart.nodeSpacing, 24)
  assert.equal(config.securityLevel, 'strict')
  assert.equal(config.htmlLabels, false)
})

test('auto Mermaid style follows the Studio theme', () => {
  assert.equal(mermaidInitializeConfig({}, { dark: false }).theme, 'neo')
  assert.equal(mermaidInitializeConfig({}, { dark: true }).theme, 'neo-dark')
})
