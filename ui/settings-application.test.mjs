import assert from 'node:assert/strict'
import test from 'node:test'
import { createSettingsApplication } from './settings-application.mjs'
import { normalizeTypography } from './preference-normalization.mjs'

function fixture(saved = {}, stored = {}) {
  const state = { hostPlatform: 'linux' }, calls = [], elements = new Map()
  const defaults = { uiFontFamily: 'sans-serif', uiFontSize: 13, uiFontWeight: 400, contentFontFamily: 'serif', contentFontSize: 15, contentFontWeight: 500, codeFontFamily: 'monospace', codeFontSize: 13, codeFontWeight: 400, highContrast: false }
  const $ = id => {
    if (!elements.has(id)) elements.set(id, { value: '', checked: false, textContent: '', classList: { add() {}, remove() {} }, close: () => calls.push('close') })
    return elements.get(id)
  }
  const services = {
    $, gatewayFetch: async path => { calls.push(path); return new Response(JSON.stringify(path === '/studio/preferences' ? saved : stored)) },
    normalizeRightRailWidthRatio: () => 0.4, typographyDefaults: defaults,
    normalizeTypography: value => normalizeTypography(value, defaults), migrateDefaultFontFamilies: v => v || {},
    isSupportedBackend: b => ['codex', 'ept-codex', 'opencode'].includes(b), emptyBackendSelections: () => ({ codex: null, 'ept-codex': null, opencode: null }),
    normalizeAnnotationDrafts: v => v || {}, resolveLanguage: () => 'en-US', normalizeLocalizedTemplates: v => v || {}, defaultAnnotationPrompt: () => '{{annotations}}',
    markPreferencesReady: () => calls.push('ready'), persistOpeningMessageState: () => calls.push('opening'), applySidebarState: () => calls.push('sidebar'),
    getLocale: () => 'en-US', setLanguage() {}, syncEmbeddedBrowserTranslations() {}, t: s => s,
    applyAppearance: () => calls.push('appearance'), persistPreferences: async () => calls.push('persist'), sessionResources: { invalidate: () => calls.push('resources') }, renderLocalizedUI() {}, toast() {},
    annotationPromptDefaults: { 'en-US': '{{annotations}}' }, populateSettingsForm() {},
  }
  return { state, services, calls, $, controller: () => createSettingsApplication(state, services) }
}
test('loads settings and SQLite state separately and always selects the Router', async () => {
  const f = fixture({ selectedThread: 'old', selectedBackend: 'opencode', router: { controllerBackend: 'codex', controllers: { codex: 'router' }, fallbacks: [] }, sharedDocumentDirectories: ['/tmp', '/tmp'] }, { messageQueues: { 'codex:a': [{ id: 'q', text: 'queued', input: [], createdAt: 1 }] } })
  await f.controller().loadPreferences()
  assert.equal(f.state.selectedId, 'router')
  assert.equal(f.state.backend, 'codex')
  assert.deepEqual(f.state.sharedDocumentDirectories, ['/tmp'])
  assert.ok(f.state.pausedMessageQueues.has('codex:a'))
  assert.deepEqual(f.calls.slice(0, 2), ['/studio/preferences', '/studio/session-state'])
  assert.equal(f.calls.includes('persist'), false)
})
test('malformed top-level settings fall back to safe defaults', async () => {
  const f = fixture(null, [])
  await f.controller().loadPreferences()
  assert.equal(f.state.theme, 'light')
  assert.equal(f.state.queueDepth, 1)
  assert.deepEqual(f.state.messageQueues, {})
})
test('invalid templates and relative shared directories do not persist changes', async () => {
  const f = fixture(), c = f.controller()
  await c.loadPreferences()
  await c.saveSettings({ preventDefault() {} })
  assert.match(f.$('#settings-error').textContent, /must contain/)
  f.$('#annotation-template').value = '{{annotations}}'
  f.$('#shared-document-directories').value = 'relative/path'
  await c.saveSettings({ preventDefault() {} })
  assert.match(f.$('#settings-error').textContent, /absolute paths/)
  assert.equal(f.calls.includes('persist'), false)
})
test('save failure keeps the settings dialog open and reset does not write storage', async () => {
  const f = fixture()
  f.services.persistPreferences = async () => { throw Error('disk full') }
  const c = f.controller(); await c.loadPreferences()
  f.$('#annotation-template').value = '{{annotations}}'
  await c.saveSettings({ preventDefault() {} })
  assert.equal(f.calls.includes('close'), false)
  assert.equal(f.calls.includes('resources'), false)
  assert.equal(f.$('#settings-error').textContent, 'Unable to save settings')
  c.resetSettings()
  assert.equal(f.state.continueBehavior, 'sessionModelDraft')
  assert.equal(f.calls.includes('persist'), false)
})
