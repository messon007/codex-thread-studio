import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('project environment is a project-scoped thread action instead of global settings content', () => {
  const threadMenu = html.match(/<div id="thread-more-menu"[\s\S]*?<\/div>/u)?.[0] || ''
  const settings = html.match(/<dialog id="settings-dialog"[\s\S]*?<\/dialog>/u)?.[0] || ''

  assert.match(threadMenu, /id="project-environment-action"/u)
  assert.match(threadMenu, /id="project-environment-indicator"/u)
  assert.doesNotMatch(settings, /id="environment-settings"|id="environment-variables"/u)
  assert.match(app, /renderProjectEnvironmentEntry\(\)/u)
})

test('dedicated environment dialog establishes project identity and separates primary from advanced controls', () => {
  const dialog = html.match(/<dialog id="environment-dialog"[\s\S]*?<\/dialog>/u)?.[0] || ''

  assert.match(dialog, /id="environment-project-name"/u)
  assert.match(dialog, /id="environment-project-root"/u)
  assert.match(dialog, /id="environment-profile-status"/u)
  assert.match(dialog, /class="environment-primary-grid"/u)
  assert.match(dialog, /<details id="environment-advanced"/u)
  assert.equal((dialog.match(/name="environment-network-policy"/gu) || []).length, 2)
  assert.match(dialog, /后续 Agent Turn 与新启动的 Terminal/u)
})

test('secret values remain write-only while saved names use reversible removal chips', () => {
  assert.match(html, /id="environment-secrets"/u)
  assert.doesNotMatch(html, /id="environment-remove-secrets"/u)
  assert.match(app, /environmentSecretRemovals = new Set\(\)/u)
  assert.match(app, /toggleEnvironmentSecretRemoval/u)
  assert.match(app, /aria-pressed="\$\{environmentSecretRemovals\.has\(name\)\}"/u)
})

test('environment dialog uses a spacious two-column desktop layout with a narrow-window fallback', () => {
  assert.match(styles, /\.environment-dialog \{[^}]*width: min\(820px, calc\(100vw - 42px\)\)/u)
  assert.match(styles, /\.environment-primary-grid \{[^}]*grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/u)
  assert.match(styles, /@media \(max-width: 720px\) \{[\s\S]*\.environment-primary-grid, \.environment-advanced-grid \{ grid-template-columns: 1fr;/u)
})
