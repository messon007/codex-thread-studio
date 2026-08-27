import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('settings use a shared categorized pane layout', () => {
  assert.match(html, /id="settings-navigation"/u)
  for (const pane of ['general', 'typography', 'comments']) {
    assert.match(html, new RegExp(`data-settings-pane="${pane}"`, 'u'))
    assert.match(html, new RegExp(`data-settings-pane-content="${pane}"`, 'u'))
  }
  assert.doesNotMatch(html, /data-settings-pane="appearance"/u)
  assert.match(html, /data-settings-pane-content="general"[\s\S]*id="theme-select"[\s\S]*id="content-width"[\s\S]*id="high-contrast"/u)
  assert.match(app, /function activateSettingsPane/u)
  assert.match(app, /activeSettingsPane = button\.dataset\.settingsPane/u)
  assert.match(styles, /\.settings-layout \{[^}]*grid-template-columns: 218px minmax\(0, 1fr\)/u)
  assert.match(styles, /\.settings-navigation-item\.active \{[^}]*background: var\(--brand-soft\)/u)
  assert.match(styles, /\.settings-section \{[^}]*border: 0;[^}]*background: transparent;/u)
  assert.match(styles, /\.typography-profile \{[^}]*border-bottom: 1px solid var\(--border\);/u)
})

test('user messages have a quiet content background', () => {
  assert.match(
    styles,
    /\.message\.user \.message-content \{[^}]*margin-inline: -8px;[^}]*padding: 6px 8px;[^}]*border-radius: 9px;[^}]*background: color-mix\(in srgb, var\(--brand-soft\) 44%, transparent\);/u,
  )
})

test('session More action uses the shared toolbar SVG geometry', () => {
  assert.match(html, /id="thread-more-button"[^>]*><svg viewBox="0 0 24 24"/u)
  assert.doesNotMatch(html, /id="thread-more-button"[^>]*>•••</u)
  assert.match(styles, /#thread-more-button svg \{[^}]*width: 17px;[^}]*height: 17px;/u)
})
