import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { Marked } from './vendor/marked.esm.js'
import { diagramKind, hasUnsafePlantumlDirective, plantumlLines, plantumlMarkdownExtension, markdownParserForSource } from './diagram-source.mjs'
import { extractMarkdownOutline } from './document-outline.mjs'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const rust = readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8')
const vendorScript = readFileSync(new URL('../scripts/vendor-markdown.mjs', import.meta.url), 'utf8')

test('detects fenced diagram languages and bare PlantUML blocks', () => {
  assert.equal(diagramKind('mermaid', 'A-->B'), 'mermaid')
  assert.equal(diagramKind('plantuml', 'Alice -> Bob'), 'plantuml')
  assert.equal(diagramKind('puml', 'Alice -> Bob'), 'plantuml')
  assert.equal(diagramKind('uml', 'Alice -> Bob'), 'plantuml')
  assert.equal(diagramKind('d2', 'a -> b'), 'd2')
  assert.equal(diagramKind('code', '@startuml\nA -> B\n@enduml'), 'plantuml')
  assert.equal(diagramKind('javascript', '@startuml\nA -> B\n@enduml'), null)
})

test('parses raw PlantUML without consuming adjacent Markdown', () => {
  const marked = new Marked({ extensions: [plantumlMarkdownExtension()] })
  const output = marked.parse('# Heading\n\n@startuml\nAlice -> Bob: hello\n@enduml\n\nAfter')
  assert.match(output, /<h1>Heading<\/h1>/u)
  assert.match(output, /<code class="language-plantuml">@startuml/u)
  assert.match(output, /Alice -&gt; Bob/u)
  assert.match(output, /<p>After<\/p>/u)
  assert.doesNotMatch(output, /<p>lo/u)
  const fenced = marked.parse('```text\n@startuml\nAlice -> Bob\n@enduml\n```')
  assert.match(fenced, /<code class="language-text">/u)
  assert.doesNotMatch(fenced, /language-plantuml/u)
})

test('ordinary Markdown uses a reusable parser with no PlantUML scanner', () => {
  const source = '## Section\n\nParagraph **content**\n\u7b2c\u4e8c\u884c\u3002\n\n'.repeat(30_000)
  const parser = markdownParserForSource(source)
  assert.equal(parser.defaults.extensions, null)
  assert.equal(markdownParserForSource('Another document'), parser)
  assert.equal(markdownParserForSource(''), parser)
  assert.equal(markdownParserForSource('@startumlExtra'), parser)
  assert.equal(parser.defaults.async, false)
  assert.equal(parser.defaults.breaks, true)
  assert.equal(parser.defaults.gfm, true)
})

test('content-aware parsing preserves configured Markdown and original source tokens', () => {
  const reference = new Marked({ async: false, breaks: true, gfm: true, extensions: [plantumlMarkdownExtension()] })
  for (const source of [
    '# \u4e2d\u6587 😀\n\nFirst\nSecond\n\n## Next\n',
    '| left | right |\n| :--- | ---: |\n| a | b |\n\n- [x] ~~done~~\n\nhttps://example.com\n',
    '[link][ref]\n\n[ref]: ../README.md:1 "title"\n',
    '```mermaid\ngraph TD; A-->B\n```\n\n```d2\na -> b\n```\n',
    '<details><summary>Title</summary>\n\n**body**\n\n</details>\n',
    '# Top\n\n@startuml\nAlice -> Bob\n@enduml\n\n## Bottom\n',
  ]) {
    const parser = markdownParserForSource(source)
    const tokens = parser.lexer(source)
    assert.deepEqual(tokens, reference.lexer(source))
    assert.equal(parser.parse(source), reference.parse(source))
    assert.equal(parser.parser(tokens), reference.parse(source))
    assert.deepEqual(extractMarkdownOutline(source, tokens), extractMarkdownOutline(source, reference.lexer(source)))
  }
})

test('PlantUML routing preserves nested blocks, fences, case, CRLF and incomplete streaming input', () => {
  const plain = markdownParserForSource('Ordinary Markdown')
  const extended = markdownParserForSource('@startuml\nAlice -> Bob\n@enduml')
  const reference = new Marked({ async: false, breaks: true, gfm: true, extensions: [plantumlMarkdownExtension()] })
  assert.notEqual(extended, plain)
  for (const source of [
    '> @startuml\n> Alice -> Bob\n> @enduml\n',
    '- @startuml\n  Alice -> Bob\n  @enduml\n',
    '@STARTUML\r\nAlice -> Bob\r\n@ENDUML\r\n',
    '@startuml\nAlice -> Bob\n@enduml\n\n@startuml\nBob -> Alice\n@enduml\n',
    '@startuml\nAlice -> Bob',
    '```text\n@startuml\nAlice -> Bob\n@enduml\n```\n',
    '```plantuml\nAlice -> Bob\n```\n',
  ]) {
    const parser = markdownParserForSource(source)
    assert.equal(parser.parse(source), reference.parse(source), source)
    assert.equal(parser.parser(parser.lexer(source)), reference.parse(source), source)
  }
  assert.equal(markdownParserForSource('No diagram on the next parse'), plain)
})

test('chat keeps its parser while document workers retain the same sanitization', () => {
  assert.doesNotMatch(app, /marked\.use\(/u)
  assert.match(app, /await markdownDocumentClient\.parse\(source\)/u)
  assert.match(app, /renderMarkdownHtml\('', \{ dirtyHtml: parsed\.html, documentImages: true \}\)/u)
  assert.match(app, /const activeParser = parser \|\| markdownParserForSource\(source\)/u)
  assert.match(app, /tokens \? activeParser\.parser\(tokens\) : activeParser\.parse\(source\)/u)
  assert.match(app, /DOMPurify\.sanitize\(dirty/u)
  assert.match(app, /FORBID_TAGS: \['button', 'form', 'iframe', 'object', 'embed', 'script', 'style'\]/u)
  assert.match(app, /FORBID_ATTR: \['style'\]/u)
  const document = app.slice(app.indexOf('async function renderMarkdownDocument('), app.indexOf('function readMarkdownRenderCache('))
  assert.doesNotMatch(document, /\.lexer\(/u)
})

test('wraps fenced PlantUML and rejects external includes', () => {
  assert.deepEqual(plantumlLines('Alice -> Bob'), ['@startuml', 'Alice -> Bob', '@enduml'])
  assert.deepEqual(plantumlLines('@startuml\nAlice -> Bob\n@enduml'), ['@startuml', 'Alice -> Bob', '@enduml'])
  for (const source of ['!include https://example.com/a.puml', '!includeurl https://example.com/a.puml', '!theme plain', '!import local.puml', '%load_json("https://example.com/a.json")']) {
    assert.equal(hasUnsafePlantumlDirective(source), true, source)
  }
  assert.equal(hasUnsafePlantumlDirective('!define ACCENT #f00\nAlice -> Bob'), false)
})

test('vendors lazy browser renderers and permits D2 blob workers without CDN access', () => {
  const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'))
  const lock = JSON.parse(readFileSync(new URL('../package-lock.json', import.meta.url), 'utf8'))
  for (const name of ['@plantuml/core', '@d2lang/d2']) {
    assert.equal(lock.packages[`node_modules/${name}`].version, manifest.dependencies[name])
  }
  assert.match(vendorScript, /@plantuml\/core\/plantuml\.js/u)
  assert.match(vendorScript, /@d2lang\/d2\/dist\/browser\/index\.js/u)
  assert.match(app, /script\.src = '\/vendor\/plantuml-viz-global\.js'/u)
  assert.match(app, /await loadPlantumlViz\(\)/u)
  assert.match(rust, /route\("\/vendor\/plantuml\.js", get\(plantuml_js\)\)/u)
  assert.match(rust, /route\("\/vendor\/d2\.js", get\(d2_js\)\)/u)
  assert.match(rust, /worker-src 'self' blob:/u)
  assert.match(rust, /script-src 'self' 'wasm-unsafe-eval'/u)
  assert.match(app, /hasUnsafePlantumlDirective\(source\)/u)
  assert.match(app, /DOMPurify\.sanitize\(svg/u)
})
