import { markdownParserForSource } from '/diagram-source.mjs'
import { extractMarkdownOutline, normalizeDocumentOutline } from '/document-outline.mjs'
import { renderMarkdownHtml } from '/actual-render.mjs'

let wasm

export async function initialize() {
  const start = performance.now()
  wasm = await import('/wasm/studio_markdown_wasm_bench.js')
  const imported = performance.now()
  await wasm.default()
  return { importMs: imported - start, instantiateMs: performance.now() - imported, totalMs: performance.now() - start }
}

function outline(headings) {
  const occurrences = new Map()
  return normalizeDocumentOutline(headings.map((heading, index) => {
    const label = heading.label.replace(/\s+/gu, ' ').trim().slice(0, 512) || 'Untitled section'
    const slug = label.normalize('NFKC').toLocaleLowerCase().replace(/<[^>]*>/gu, '').replace(/[^\p{Letter}\p{Number}]+/gu, '-').replace(/^-+|-+$/gu, '').slice(0, 96) || `section-${index + 1}`
    const occurrence = (occurrences.get(slug) || 0) + 1
    occurrences.set(slug, occurrence)
    return { id: occurrence === 1 ? slug : `${slug}-${occurrence}`, label, depth: heading.depth, target: { kind: 'text-heading', line: heading.line, offset: heading.offset } }
  }))
}

function documentResult(engine, source) {
  if (engine === 'marked') {
    const parser = markdownParserForSource(source)
    const tokens = parser.lexer(source)
    return { html: parser.parser(tokens), outline: extractMarkdownOutline(source, tokens) }
  }
  if (engine === 'wasm-json') {
    const result = JSON.parse(wasm.render_document_json(source))
    return { html: result.html, outline: outline(result.headings) }
  }
  const document = wasm.parse_document(source)
  try {
    return { html: document.take_html(), outline: outline(JSON.parse(document.headings_json())) }
  } finally {
    document.free()
  }
}

export function signature(html) {
  const target = document.querySelector('main')
  target.innerHTML = renderMarkdownHtml('', { parser: { parse: () => html }, documentImages: true })
  const all = selector => [...target.querySelectorAll(selector)]
  const text = element => element.innerText.replace(/\s+/gu, ' ').trim()
  const result = {
    text: text(target),
    headings: all('h1,h2,h3,h4,h5,h6').map(element => [element.tagName, text(element)]),
    links: all('a').map(element => [element.getAttribute('href'), text(element)]),
    cells: all('th,td').map(element => [element.tagName, text(element), element.getAttribute('align')]),
    code: all('pre code').map(element => [element.className, element.textContent.trimEnd()]),
    checkboxes: all('input').map(element => [element.type, element.checked]),
    formatting: all('strong,em,del').map(element => [element.tagName, text(element)]),
    lists: all('ol,ul,li,blockquote').map(element => [element.tagName, element.getAttribute('start')]),
    breaks: all('br').length,
    unsafe: all('script,iframe,object,embed,style,form').length + all('*').filter(element => [...element.attributes].some(attribute => /^on/i.test(attribute.name) || /javascript:/i.test(attribute.value))).length,
  }
  target.replaceChildren()
  return result
}

export function compatibility(source, nativeJson) {
  const marked = documentResult('marked', source)
  const candidate = documentResult('wasm-parts', source)
  const expected = signature(marked.html)
  const actual = signature(candidate.html)
  const differences = Object.keys(expected).filter(key => JSON.stringify(expected[key]) !== JSON.stringify(actual[key]))
  const raw = wasm.render_document_json(source)
  const shape = item => item && [item.id, item.label, item.depth, item.parentId, item.target?.kind, item.target?.offset, item.target?.line]
  const outlineDifferences = marked.outline.flatMap((item, index) => JSON.stringify(shape(item)) === JSON.stringify(shape(candidate.outline[index])) ? [] : [{ marked: shape(item), wasm: shape(candidate.outline[index]) }]).slice(0, 3)
  const outlineMatches = marked.outline.length === candidate.outline.length && outlineDifferences.length === 0
  return { differences, outlineMatches, outlineDifferences, unsafe: actual.unsafe, nativeMatchesWasm: raw === nativeJson, markedHeadings: marked.outline.length, wasmHeadings: candidate.outline.length }
}

export async function measure(source, samples) {
  const engines = ['marked', 'wasm-json', 'wasm-parts']
  const results = Object.fromEntries(engines.map(engine => [engine, { html: [], document: [] }]))
  for (let iteration = 0; iteration < samples + 3; iteration++) {
    for (let index = 0; index < engines.length; index++) {
      const engine = engines[(index + iteration) % engines.length]
      let start = performance.now()
      const html = engine === 'marked' ? markdownParserForSource(source).parse(source) : wasm.render_html(source)
      const htmlMs = performance.now() - start
      start = performance.now()
      const result = documentResult(engine, source)
      const documentMs = performance.now() - start
      if (!html.length || !result.html.length) throw Error('empty output')
      if (iteration >= 3) {
        results[engine].html.push(htmlMs)
        results[engine].document.push(documentMs)
      }
    }
  }
  const timerLag = {}
  for (const engine of ['marked', 'wasm-parts']) {
    const start = performance.now()
    const timer = new Promise(resolve => setTimeout(() => resolve(performance.now() - start), 0))
    documentResult(engine, source)
    timerLag[engine] = await timer
  }
  return { results, timerLag, wasmLinearMemoryBytes: wasm.wasm_memory().buffer.byteLength }
}

export async function pipeline(source, iterations) {
  const target = document.querySelector('main')
  target.classList.add('artifact-content')
  target.style.height = '850px'
  const results = { marked: [], wasm: [] }
  for (let iteration = 0; iteration < iterations + 1; iteration++) {
    for (const engine of iteration % 2 ? ['wasm', 'marked'] : ['marked', 'wasm']) {
      const start = performance.now()
      const result = documentResult(engine === 'wasm' ? 'wasm-parts' : engine, source)
      const parsed = performance.now()
      const html = renderMarkdownHtml('', { parser: { parse: () => result.html }, documentImages: true })
      const sanitized = performance.now()
      target.innerHTML = html
      void target.offsetHeight
      const mounted = performance.now()
      await new Promise(requestAnimationFrame)
      await new Promise(requestAnimationFrame)
      if (iteration) results[engine].push({ parseMs: parsed - start, sanitizeAndPostprocessMs: sanitized - parsed, mountAndLayoutMs: mounted - sanitized, throughSecondFrameMs: performance.now() - start, elements: target.querySelectorAll('*').length, headings: result.outline.length })
      target.replaceChildren()
    }
  }
  return results
}

export function memoryProbe(engine, source) {
  const bytes = () => wasm ? wasm.wasm_memory().buffer.byteLength : 0
  const before = bytes()
  documentResult(engine, source)
  const first = bytes()
  for (let index = 0; index < 8; index++) documentResult(engine, source)
  return { before, first, repeated: bytes() }
}

export function firstParse(source, engine = 'wasm-parts') {
  const start = performance.now()
  const result = documentResult(engine, source)
  return { documentMs: performance.now() - start, headings: result.outline.length, htmlUtf16Length: result.html.length }
}
