// Optional: STUDIO_PLAYWRIGHT_MODULE=/path/to/playwright-core/index.mjs node scripts/smoke-reader-browser.mjs
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawn } from 'node:child_process'
import { createServer } from 'node:http'
import { createInterface } from 'node:readline'
import assert from 'node:assert/strict'
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const root = new URL('../', import.meta.url)
const temporary = await mkdtemp(join(tmpdir(), 'studio-reader-browser-'))
const browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH, headless: true, args: ['--no-sandbox'] })
try {
  for (const extension of ['md', 'jsonl']) {
    const path = join(temporary, `data.${extension}`)
    await writeFile(path, extension === 'md' ? '# Selection fixture\n\nA selectable paragraph with 中文 and a stable source anchor.\n\n| Name | Value |\n|---|---|\n| first | 2 |\n| second | 3 |\n' : '{"name":"中文","value":2}\n{"name":"other","value":3}\n')
    const child = spawn(new URL('target/debug/examples/reader-service', root).pathname, [path])
    const info = await new Promise((resolve, reject) => {
      createInterface({ input: child.stdout }).once('line', line => resolve(JSON.parse(line)))
      child.once('error', reject)
    })
    const server = createServer(async (request, response) => {
      try {
        if (request.url === '/host.mjs') { response.setHeader('content-type', 'text/javascript'); response.end(await readFile(new URL('ui/large-markdown-host.mjs', root))); return }
        if (request.url === '/test') {
          response.setHeader('content-type', 'text/html')
          response.end(`<style>iframe{width:900px;height:700px}</style><iframe src="/?embedded=1&diagrams=browser#${info.token}"></iframe><div id="selection"></div><script type="module">import {createLargeMarkdownHost} from '/host.mjs'; window.host=createLargeMarkdownHost(document.querySelector('iframe'),()=>{},selection=>{window.captured=selection;document.querySelector('#selection').textContent='Comment Translate'})</script>`); return
        }
        const chunks = []; for await (const chunk of request) chunks.push(chunk)
        const headers = { ...request.headers }; delete headers.host; delete headers.origin; delete headers.connection
        const upstream = await fetch(`http://${info.address}${request.url}`, { method: request.method, headers, ...(chunks.length ? { body: Buffer.concat(chunks) } : {}) })
        response.statusCode = upstream.status
        for (const key of ['content-type', 'content-security-policy']) if (upstream.headers.has(key)) response.setHeader(key, key === 'content-security-policy' ? upstream.headers.get(key).replace("frame-ancestors 'none'", "frame-ancestors 'self'").replace("script-src 'self'", "script-src 'self' 'wasm-unsafe-eval'") : upstream.headers.get(key))
        let bytes = Buffer.from(await upstream.arrayBuffer())
        if (request.url.split('?')[0] === '/') bytes = Buffer.from(bytes.toString().replace('</head>', `<style>${await readFile(new URL('ui/large-markdown-host.css', root), 'utf8')}</style></head>`))
        response.end(bytes)
      } catch (error) { response.statusCode = 500; response.end(String(error)) }
    })
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
    const page = await browser.newPage({ viewport: { width: 1100, height: 900 } }), errors = []
    page.setDefaultTimeout(15000)
    console.log(extension, 'reader started')
    page.on('pageerror', error => { errors.push(error.message); console.error('Reader error:', error.message) })
    try {
      await page.goto(`http://127.0.0.1:${server.address().port}/test`)
      const reader = page.frameLocator('iframe')
      await reader.locator('#source-mode').waitFor({ state: 'attached' })
      await page.waitForFunction(() => document.querySelector('iframe').contentWindow?.readerExperiment?.index)
      if (extension === 'md') {
        await reader.locator('.page p').first().waitFor()
        await reader.locator('.page p').first().evaluate(element => {
          const range = document.createRange(); range.selectNodeContents(element)
          const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(range)
          element.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }))
        })
        await page.waitForFunction(() => window.captured?.quote.includes('selectable'))
        assert.equal(await page.locator('#selection').textContent(), 'Comment Translate')
        assert.ok((await page.evaluate(() => window.captured.rect.top)) > 0)
        await reader.getByRole('button', { name: 'SQL', exact: true }).click()
        await reader.getByRole('textbox', { name: 'SQL query', exact: true }).fill('SELECT sum(CAST("Value" AS INTEGER)) AS total FROM data')
        await reader.getByRole('button', { name: 'Run query', exact: true }).click()
        await reader.locator('.table-query-result td').filter({ hasText: '5' }).waitFor()
      } else {
        await reader.getByRole('button', { name: 'jq', exact: true }).click()
        await reader.getByRole('textbox', { name: 'jq filter', exact: true }).fill('select(.value == 2) | .name')
        await reader.getByRole('button', { name: 'Run filter', exact: true }).click()
        await reader.locator('.jq-results').filter({ hasText: '中文' }).waitFor()
      }
      assert.deepEqual(errors, [])
      console.log(`${extension}: browser selection/query UI passed`)
    } finally { await page.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); child.kill() }
  }
} finally { await browser.close(); await rm(temporary, { recursive: true, force: true }) }
