import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const config = readFileSync(new URL('./mermaid-config.mjs', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
const packageJson = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'))
const vendorScript = readFileSync(new URL('../scripts/vendor-markdown.mjs', import.meta.url), 'utf8')
const rust = readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8')

test('pins and embeds Mermaid before the application module', () => {
  const lock = JSON.parse(readFileSync(new URL('../package-lock.json', import.meta.url), 'utf8'))
  assert.match(packageJson.dependencies.mermaid, /^\d+\.\d+\.\d+$/u)
  assert.equal(lock.packages['node_modules/mermaid'].version, packageJson.dependencies.mermaid)
  assert.match(vendorScript, /mermaid\/dist\/mermaid\.min\.js/u)
  assert.match(vendorScript, /mermaid-MIT\.txt/u)
  assert.ok(html.indexOf('/vendor/mermaid.min.js') < html.indexOf('/app.js'))
  assert.match(rust, /route\("\/vendor\/mermaid\.min\.js", get\(mermaid_js\)\)/u)
})

test('renders only fenced Mermaid blocks with strict bounded post-sanitization', () => {
  assert.match(app, /normalizedLanguage === 'mermaid'/u)
  assert.match(app, /MAX_MERMAID_SOURCE_CHARS = 100_000/u)
  assert.match(config, /securityLevel: 'strict'/u)
  assert.match(config, /htmlLabels: false/u)
  assert.match(config, /startOnLoad: false/u)
  assert.match(config, /suppressErrorRendering: true/u)
  assert.match(app, /DOMPurify\.sanitize\(result\.svg/u)
  assert.match(app, /MutationObserver/u)
  assert.match(app, /showMermaidError/u)
})

test('keeps multilingual labels in sanitizable SVG text instead of foreignObject HTML', () => {
  const examples = [
    'Sense[PDC / LBMS\\n\u91C7\u96C6\u7535\u6C60\u4E0E\u56DE\u8DEF\u72B6\u6001]',
    'NoRequest --> ConstantCurrent: \u5145\u7535 MOS \u4ECE\u65AD\u5F00\u6062\u590D\u4E3A\u95ED\u5408',
  ]
  assert.ok(examples.every((source) => /[\p{Script=Han}]/u.test(source)))
  assert.match(config, /htmlLabels: false/u)
  assert.doesNotMatch(app, /ADD_TAGS:\s*\[[^\]]*foreignObject/u)
})

test('renders fenced plain text without a title or copy action', () => {
  assert.match(app, /normalizedLanguage === 'text'/u)
  assert.match(app, /normalizedLanguage === 'plaintext'/u)
  assert.match(app, /normalizedLanguage === 'txt'/u)
  assert.match(app, /markdown-plain-text/u)
  assert.match(styles, /markdown-plain-text/u)
})

test('provides responsive diagram and source-fallback styles', () => {
  assert.match(styles, /\.markdown-mermaid-canvas/u)
  assert.match(styles, /\.markdown-mermaid-canvas svg/u)
  assert.match(styles, /\.markdown-mermaid\.show-source \.markdown-mermaid-source/u)
  assert.match(styles, /data-mermaid-state="error"/u)
})
