import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { marked } from '../ui/vendor/marked.esm.js'
import { plantumlMarkdownExtension } from '../ui/diagram-source.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outputDir = resolve(process.argv[2] || resolve(root, 'target/markdown-parser-results'))
const fixtures = JSON.parse(await readFile(resolve(outputDir, 'compatibility.json'), 'utf8'))
const results = JSON.parse(await readFile(resolve(outputDir, 'results.json'), 'utf8'))
marked.setOptions({ async: false, breaks: true, gfm: true })
marked.use({ extensions: [plantumlMarkdownExtension()] })
for (const entry of results.cases.filter(entry => ['readme', 'acceptance-review'].includes(entry.name))) {
  const path = resolve(outputDir, `${entry.name}.md`)
  const source = await readFile(path, 'utf8')
  const outputs = { marked: marked.parse(source) }
  for (const engine of ['pulldown', 'comrak']) {
    const result = spawnSync(resolve(root, 'target/markdown-parser-bench/bin/default'), [engine, path, 'render'], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
    if (result.error) throw result.error
    assert.equal(result.status, 0, result.stderr)
    outputs[engine] = result.stdout
  }
  fixtures.push({ name: entry.name, source, outputs })
}
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const browser = await chromium.launch({ headless: true, executablePath: process.env.STUDIO_CHROMIUM_PATH })
try {
  const page = await browser.newPage()
  await page.route('http://studio.test/**', async route => {
    const path = new URL(route.request().url()).pathname
    const allowed = ['/vendor/marked.esm.js', '/vendor/purify.es.mjs', '/diagram-source.mjs']
    if (path === '/') return route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Parser benchmark</title>' })
    if (!allowed.includes(path)) return route.fulfill({ status: 404, body: '' })
    await route.fulfill({ contentType: 'text/javascript', body: await readFile(resolve(root, 'ui', path.slice(1))) })
  })
  await page.goto('http://studio.test/')
  const compatibility = await page.evaluate(async fixtures => {
    const { default: DOMPurify } = await import('/vendor/purify.es.mjs')
    const signature = html => {
      const template = document.createElement('template')
      template.innerHTML = DOMPurify.sanitize(html, {
        USE_PROFILES: { html: true },
        FORBID_TAGS: ['button', 'form', 'iframe', 'object', 'embed', 'script', 'style'],
        FORBID_ATTR: ['style'],
      })
      const content = document.createElement('div')
      content.append(template.content)
      document.body.replaceChildren(content)
      const all = selector => [...content.querySelectorAll(selector)]
      const text = element => element.innerText.replace(/\s+/gu, ' ').trim()
      const result = {
        text: text(content),
        headings: all('h1,h2,h3,h4,h5,h6').map(element => [element.tagName, text(element)]),
        links: all('a').map(element => [element.getAttribute('href'), text(element), element.getAttribute('title')]),
        images: all('img').map(element => [element.getAttribute('src'), element.getAttribute('alt'), element.getAttribute('title')]),
        cells: all('th,td').map(element => [element.tagName, text(element), element.getAttribute('align')]),
        checkboxes: all('input').map(element => [element.getAttribute('type'), element.checked]),
        code: all('pre code').map(element => [element.className, element.textContent.trimEnd()]),
        breaks: all('br').length,
        strike: all('del').map(text),
        formatting: all('strong,em,del').map(element => [element.tagName, text(element)]),
        lists: all('ol,ul,li,blockquote').map(element => [element.tagName, element.getAttribute('start')]),
        details: all('details,summary').map(element => [element.tagName, text(element)]),
        unsafe: all('script,iframe,object,embed,style,button,form').length + all('*').filter(element => [...element.attributes].some(attribute => /^on/i.test(attribute.name) || /javascript:/i.test(attribute.value))).length,
      }
      document.body.replaceChildren()
      return result
    }
    return fixtures.map(fixture => {
      const signatures = Object.fromEntries(Object.entries(fixture.outputs).map(([engine, html]) => [engine, signature(html)]))
      const differences = Object.fromEntries(['pulldown', 'comrak'].map(engine => [engine, Object.keys(signatures.marked).filter(key => JSON.stringify(signatures.marked[key]) !== JSON.stringify(signatures[engine][key]))]))
      return { name: fixture.name, differences, signatures }
    })
  }, fixtures)
  for (const fixture of compatibility) for (const signature of Object.values(fixture.signatures)) assert.equal(signature.unsafe, 0, `${fixture.name}: sanitizer failed`)
  const timings = []
  for (const entry of results.cases.filter(entry => ['readme', 'acceptance-review', 'mixed-2mib'].includes(entry.name))) {
    await page.reload()
    const source = await readFile(resolve(outputDir, `${entry.name}.md`), 'utf8')
    const measured = await page.evaluate(async source => {
      const { Marked } = await import('/vendor/marked.esm.js')
      const { plantumlMarkdownExtension } = await import('/diagram-source.mjs')
      const engines = { marked: new Marked({ async: false, breaks: true, gfm: true }), core: new Marked({ async: false, breaks: true, gfm: true }) }
      engines.marked.use({ extensions: [plantumlMarkdownExtension()] })
      const samples = { marked: [], core: [] }
      const outputBytes = {}
      for (let index = 0; index < 20; index++) {
        for (const name of index % 2 ? ['core', 'marked'] : ['marked', 'core']) {
          const start = performance.now()
          const html = engines[name].parse(source)
          const elapsed = performance.now() - start
          outputBytes[name] = html.length
          if (index >= 5) samples[name].push(elapsed)
        }
      }
      return { samples, outputUtf16Lengths: outputBytes }
    }, source)
    timings.push({ name: entry.name, ...measured })
  }
  const report = { browser: browser.version(), compatibility, timings }
  await writeFile(resolve(outputDir, 'browser.json'), JSON.stringify(report, null, 2))
  console.log(JSON.stringify({ browser: report.browser, compatibility: compatibility.map(({ name, differences }) => ({ name, differences })), timings }, null, 2))
} finally {
  await browser.close()
}
