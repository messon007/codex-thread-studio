import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { SLASH_COMMANDS } from './composer-tools.mjs'
import { effectiveServiceTier, modelSupportsFast, resolveFastMode, selectedCatalogModel } from './service-tier.mjs'

const models = [
  { id: 'standard', model: 'standard', isDefault: true, serviceTiers: [] },
  { id: 'fast-model', model: 'fast-model', serviceTiers: [{ id: 'fast', name: 'Fast', description: 'Priority processing' }] },
  { id: 'legacy-fast', model: 'legacy-fast', additionalSpeedTiers: ['fast'] },
]

test('Fast availability follows the selected model catalog entry', () => {
  assert.equal(selectedCatalogModel(models, '', '')?.model, 'standard')
  assert.equal(selectedCatalogModel(models, 'fast-model', '')?.model, 'fast-model')
  assert.equal(modelSupportsFast(models[0]), false)
  assert.equal(modelSupportsFast(models[1]), true)
  assert.equal(modelSupportsFast(models[2]), true)
})

test('session override wins over the thread service tier', () => {
  assert.equal(effectiveServiceTier('', 'fast'), 'fast')
  assert.equal(effectiveServiceTier('default', 'fast'), 'default')
  assert.deepEqual(resolveFastMode({
    models,
    overrideModel: 'fast-model',
    overrideServiceTier: 'fast',
    sessionServiceTier: 'default',
  }), {
    model: 'fast-model',
    selectedModel: models[1],
    supported: true,
    enabled: true,
    serviceTier: 'fast',
  })
})

test('Composer exposes the Fast command and visible active-state badge', () => {
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  assert.equal(SLASH_COMMANDS.find((command) => command.name === 'fast')?.action, 'fast')
  assert.match(app, /fast: toggleFastMode/u)
  assert.match(app, /serviceTier/u)
  assert.match(html, /id="composer-model-fast"/u)
})
