import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('typography standard covers profile-owned emphasis and native toolbar synchronization', () => {
  assert.match(styles, /--ui-font-xxs: max\(10px, calc\(var\(--ui-font-size\) - 4px\)\)/u)
  assert.match(styles, /--ui-font-xs: max\(11px, calc\(var\(--ui-font-size\) - 3px\)\)/u)
  assert.match(styles, /strong, b, h1, h2, h3, h4, h5, h6 \{ font-weight: var\(--ui-font-emphasis\);/u)
  assert.match(app, /--content-font-emphasis', Math\.min\(700, state\.typography\.contentFontWeight \+ 100\)/u)
  assert.match(app, /set-browser-typography\?profile=/u)
  const toolbar = readFileSync(new URL('./embedded-browser.html', import.meta.url), 'utf8')
  assert.match(toolbar, /root\.setProperty\('--ui-font-family', typography\.fontFamily\)/u)
  for (const platform of ['embedded_browser.rs', 'embedded_browser_windows.rs']) {
    const source = readFileSync(new URL(`../src-tauri/src/${platform}`, import.meta.url), 'utf8')
    assert.match(source, /SetTypography/u)
    assert.match(source, /"typography": (?:workspace|w)\.typography/u)
  }
  assert.match(styles, /\.artifact-editor-gutter \{[^}]*font: var\(--code-font-weight\) var\(--code-font-size\)\/1\.72/u)
})

test('readable compact controls and hints follow the configured interface profile', () => {
  const rules = styles.slice(styles.indexOf('/* Readable interface labels:'))
  for (const selector of ['.studio-entry-copy small', '.selection-popover button', '.segmented-control button', '.check-field strong', '.translation-speak-button']) {
    assert.ok(rules.includes(selector))
  }
  assert.match(rules, /font-size: var\(--ui-font-compact\);\s*font-weight: var\(--ui-font-weight\);/u)
  assert.match(rules, /\.artifact-footer,[\s\S]*?font-size: var\(--ui-font-label\);\s*font-weight: var\(--ui-font-weight\);\s*color: var\(--muted\);/u)
})

test('defines independent interface, reading-content, and code typography settings', () => {
  const expected = '"Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif'
  assert.match(app, new RegExp(`const defaultUiFontFamily = '${expected}'`, 'u'))
  assert.match(app, /uiFontSize: 14/u)
  assert.match(app, /uiFontWeight: 500/u)
  assert.match(app, /contentFontSize: 15/u)
  assert.match(app, /contentFontWeight: 500/u)
  assert.match(styles, /--ui-font-family: "Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif;/u)
  assert.match(styles, /--content-font-family: "Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif;/u)
  assert.match(styles, /--ui-font-weight: 500;/u)
  for (const id of ['ui-font-family', 'ui-font-size', 'ui-font-weight', 'content-font-family', 'content-font-size', 'content-font-weight', 'code-font-family', 'code-font-size', 'code-font-weight']) {
    assert.match(html, new RegExp(`id="${id}"`, 'u'))
  }
  assert.doesNotMatch(html, /id="workspace-font-/u)
})

test('applies reading-content typography to both message roles', () => {
  assert.match(
    styles,
    /\.message\.user \{[^}]*font-family: var\(--content-font-family\);[^}]*font-size: var\(--content-font-size\);[^}]*font-weight: var\(--content-font-weight\);/u,
  )
  assert.match(styles, /\.message\.agent \{[^}]*font-family: var\(--content-font-family\);[^}]*font-size: var\(--content-font-size\);[^}]*font-weight: var\(--content-font-weight\);/u)
  assert.match(styles, /\.composer textarea \{[^}]*font-family: var\(--content-font-family\);[^}]*font-size: var\(--content-font-size\);[^}]*font-weight: var\(--content-font-weight\);/u)
  assert.match(styles, /\.markdown-body \{[^}]*font-family: var\(--content-font-family\);[^}]*font-size: var\(--content-font-size\);[^}]*font-weight: var\(--content-font-weight\);/u)
})

test('keeps compact activity typography at the configured reading-content weight', () => {
  assert.doesNotMatch(app, /--activity-font-weight/u)
  assert.match(styles, /--content-font-compact: max\(11px, calc\(var\(--content-font-size\) - 2px\)\);/u)
  assert.match(styles, /--code-font-compact: max\(10px, calc\(var\(--code-font-size\) - 2px\)\);/u)
  assert.match(styles, /\.activity-title \{[^}]*font-size: var\(--ui-font-label\);[^}]*font-weight: var\(--ui-font-emphasis\);/u)
  assert.match(styles, /\.activity-stage \{[^}]*font-family: var\(--content-font-family\);[^}]*font-size: calc\(var\(--content-font-size\) - 1px\);[^}]*font-weight: var\(--content-font-weight\);/u)
  assert.match(styles, /\.progress-entry, \.reasoning-entry \{[^}]*font-weight: var\(--content-font-weight\);/u)
  assert.match(styles, /\.activity-entry header strong \{[^}]*font-weight: var\(--content-font-weight\);/u)
  assert.match(styles, /\.activity-raw-item > summary > strong \{[^}]*font: var\(--content-font-weight\) var\(--content-font-compact\)\/1\.45 var\(--content-font-family\);/u)
  assert.match(styles, /\.activity-raw-body pre \{[^}]*font: var\(--content-font-weight\) var\(--content-font-compact\)\/1\.55 var\(--content-font-family\);/u)
  assert.match(styles, /\.activity-raw-body pre\.activity-raw-code, \.activity-raw-body pre\.activity-raw-code code \{[^}]*font: var\(--code-font-weight\) var\(--code-font-size\)\/1\.55 var\(--code-font-family\);/u)
  assert.match(app, /entry\.kind === 'command'[\s\S]*class="activity-raw-code"/u)
  assert.match(app, /entry\.kind === 'plan'[\s\S]*class="activity-raw-text"/u)
})

test('migrates only known former default font stacks', () => {
  assert.match(app, /legacyDefaultUiFontFamilies = new Set/u)
  assert.match(app, /Ubuntu, "Noto Sans SC"/u)
  assert.match(app, /Inter, "Noto Sans CJK SC"/u)
  assert.match(app, /legacyDefaultUiFontFamilies\.has\(typography\.uiFontFamily\)/u)
  assert.match(app, /legacyDefaultUiFontFamilies\.has\(typography\.contentFontFamily\)/u)
})

test('uses interface typography for application chrome and workspace tools', () => {
  assert.match(styles, /body \{[^}]*font-family: var\(--ui-font-family\);[^}]*font-weight: var\(--ui-font-weight\);[^}]*font-size: var\(--ui-font-size\);/u)
  assert.match(styles, /\.workspace-tools-rail \{[^}]*font-family: var\(--ui-font-family\);[^}]*font-size: var\(--ui-font-size\);[^}]*font-weight: var\(--ui-font-weight\);/u)
  assert.match(styles, /\.studio-entry-copy strong \{[^}]*font-size: var\(--ui-font-secondary\);[^}]*font-weight: var\(--ui-font-emphasis\);/u)
  assert.match(styles, /\.studio-entry-copy small \{[^}]*font-size: var\(--ui-font-sm\);/u)
  assert.match(styles, /\.action-menu button \{[^}]*font-size: var\(--ui-font-compact\);[^}]*font-weight: var\(--ui-font-weight\);/u)
  assert.match(styles, /\.studio-menu-count \{[^}]*font-size: var\(--ui-font-xxs\);[^}]*font-weight: var\(--ui-font-emphasis\);/u)
  assert.match(styles, /\.browser-menu-status small \{[^}]*font-size: var\(--ui-font-label\);/u)
  assert.match(app, /--ui-font-emphasis', Math\.min\(700, state\.typography\.uiFontWeight \+ 100\)/u)
  assert.doesNotMatch(styles, /--workspace-font-/u)
})

test('uses reading typography for diagrams and configured code typography for tools', () => {
  const editor = readFileSync(new URL('./workspace-editor.mjs', import.meta.url), 'utf8')
  const workspaceTools = readFileSync(new URL('./workspace-tools.mjs', import.meta.url), 'utf8')
  assert.match(app, /fontFamily: state\.typography\.contentFontFamily/u)
  assert.match(editor, /fontSize: 'var\(--ui-font-label\)'/u)
  assert.match(editor, /font: 'var\(--ui-font-weight\) var\(--ui-font-label\) var\(--ui-font-family\)'/u)
  assert.match(workspaceTools, /fontWeight: rootStyle\.getPropertyValue\('--code-font-weight'\)\.trim\(\) \|\| '500'/u)
})
