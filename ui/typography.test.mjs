import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

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

test('derives compact activity typography from reading-content settings', () => {
  assert.match(app, /--activity-font-weight', Math\.max\(400, state\.typography\.contentFontWeight - 100\)/u)
  assert.match(styles, /\.activity-stage \{[^}]*font-family: var\(--content-font-family\);[^}]*font-size: calc\(var\(--content-font-size\) - 1px\);[^}]*font-weight: var\(--activity-font-weight\);/u)
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
  assert.doesNotMatch(styles, /--workspace-font-/u)
})
