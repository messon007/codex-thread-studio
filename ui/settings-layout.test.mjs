import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('settings use a shared categorized pane layout', () => {
  assert.match(html, /id="settings-navigation"/u)
  for (const pane of ['general', 'typography', 'translation', 'comments']) {
    assert.match(html, new RegExp(`data-settings-pane="${pane}"`, 'u'))
    assert.match(html, new RegExp(`data-settings-pane-content="${pane}"`, 'u'))
  }
  assert.doesNotMatch(html, /data-settings-pane="appearance"/u)
  assert.match(html, /data-settings-pane-content="general"[\s\S]*id="theme-select"[\s\S]*id="content-width"[\s\S]*id="high-contrast"/u)
  assert.match(html, /id="shared-document-directories"[^>]*placeholder="One absolute directory per line"/u)
  assert.match(html, /Add multiple directories on separate lines \(no commas\)\.[^<]*its subdirectories/u)
  assert.match(app, /sharedDocumentDirectories: state\.sharedDocumentDirectories/u)
  assert.match(app, /Shared document directories must use absolute paths\./u)
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

test('right workspace headers use one stable SVG close geometry', () => {
  assert.match(html, /id="session-map-more"[^>]*><svg/u)
  assert.doesNotMatch(html, /id="session-map-more"[^>]*>•••</u)
  for (const id of ['close-session-map', 'close-resources', 'close-workspace-tools', 'close-annotation-rail', 'close-favorites']) {
    assert.match(html, new RegExp(`id="${id}"[^>]*right-workspace-close[^>]*>\\s*<svg`, 'u'))
    assert.doesNotMatch(html, new RegExp(`id="${id}"[^>]*>×`, 'u'))
  }
  assert.match(styles, /\.icon-button\.right-workspace-close svg \{[^}]*width: 16px;[^}]*height: 16px;[^}]*stroke-width: 1\.65;/u)
})

test('Session Map sync status lives with footer metadata instead of header actions', () => {
  const header = html.match(/<header class="session-map-header">[\s\S]*?<\/header>/u)?.[0] || ''
  const footer = html.match(/<footer class="session-map-footer">[\s\S]*?<\/footer>/u)?.[0] || ''
  assert.doesNotMatch(header, /session-map-sync-state/u)
  assert.match(footer, /session-map-footer-status[\s\S]*session-map-sync-state[\s\S]*session-map-revision/u)
})
