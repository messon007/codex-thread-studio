import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('uses Noto CJK Medium as the default UI and workspace typography', () => {
  const expected = '"Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif'
  assert.match(app, new RegExp(`const defaultUiFontFamily = '${expected}'`, 'u'))
  assert.match(app, /uiFontWeight: 500/u)
  assert.match(styles, /--ui-font-family: "Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif;/u)
  assert.match(styles, /--workspace-font-family: "Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif;/u)
  assert.match(styles, /--ui-font-weight: 500;/u)
})

test('migrates only known former default font stacks', () => {
  assert.match(app, /legacyDefaultUiFontFamilies = new Set/u)
  assert.match(app, /Ubuntu, "Noto Sans SC"/u)
  assert.match(app, /Inter, "Noto Sans CJK SC"/u)
  assert.match(app, /legacyDefaultUiFontFamilies\.has\(typography\.uiFontFamily\)/u)
  assert.match(app, /legacyDefaultUiFontFamilies\.has\(typography\.workspaceFontFamily\)/u)
})
