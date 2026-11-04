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
    await writeFile(path, extension === 'md' ? '# Selection fixture\n\nA selectable paragraph with 中文 and a stable source anchor.\n\n| Name | Value |\n|---|---|\n' + Array.from({length:120}, (_, i) => `| ${i === 0 ? 'first' : 'row-' + i} | ${i === 0 ? 2 : i === 1 ? 3 : 0} |\n`).join('') + '\n' + 'A trailing paragraph for document scrolling.\n\n'.repeat(80) : '{"name":"中文","value":2}\n{"name":"other","value":3}\n')
    const child = spawn(new URL('target/debug/examples/reader-service', root).pathname, [path])
    const info = await new Promise((resolve, reject) => {
      createInterface({ input: child.stdout }).once('line', line => resolve(JSON.parse(line)))
      child.once('error', reject)
    })
    const server = createServer(async (request, response) => {
      try {
        if (request.url === '/ui/markdown-table-layout.mjs') { response.setHeader('content-type', 'text/javascript'); response.end(await readFile(new URL('crates/marknoria/web/table-layout.mjs', root))); return }
        if (request.url.startsWith('/ui/')) {
          const path = new URL('.' + request.url, root)
          if (!path.pathname.startsWith(new URL('ui/', root).pathname)) throw new Error('Invalid asset')
          response.setHeader('content-type', request.url.endsWith('.css') ? 'text/css' : 'text/javascript'); response.end(await readFile(path)); return
        }
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
        const table = reader.locator('.table-content').first()
        const handle = table.getByRole('separator').first()
        await handle.waitFor({state:'attached'})
        const original = Number(await handle.getAttribute('aria-valuenow'))
        const bounds = await handle.boundingBox()
        await page.mouse.move(bounds.x + 4, bounds.y + 20)
        await page.mouse.down(); await page.mouse.move(bounds.x + 84, bounds.y + 20, {steps:8}); await page.mouse.up()
        assert.ok(Number(await handle.getAttribute('aria-valuenow')) >= original + 70)
        await handle.focus(); await page.keyboard.press('ArrowRight')
        const resized = Number(await handle.getAttribute('aria-valuenow'))
        assert.ok(resized >= original + 86)
        await reader.getByRole('button', {name:'Next page',exact:true}).click()
        await reader.locator('article[data-ready="true"][data-row-start="50"]').waitFor()
        assert.equal(Number(await handle.getAttribute('aria-valuenow')), resized)
        await reader.getByRole('button', {name:'Previous page',exact:true}).click()
        await reader.locator('article[data-ready="true"][data-row-start="0"]').waitFor()
        await reader.locator('.column-picker > summary').first().click()
        await reader.locator('.column-choices input[value="1"]').first().uncheck()
        await reader.getByRole('button', {name:'Apply',exact:true}).click()
        await reader.locator('article[data-selected-columns="[0]"][data-ready="true"]').waitFor()
        assert.equal(Number(await handle.getAttribute('aria-valuenow')),resized)
        await reader.locator('.column-picker > summary').first().click()
        await reader.locator('.column-choices input[value="1"]').first().check()
        await reader.getByRole('button', {name:'Apply',exact:true}).click()
        await reader.locator('article[data-selected-columns="[0,1]"][data-ready="true"]').waitFor()
        assert.equal(Number(await handle.getAttribute('aria-valuenow')),resized)
        // Actual wheel input verifies browser scroll chaining, including at both edges.
        await table.evaluate(el => {el.scrollTop=80;document.querySelector('#viewport').scrollTop=0})
        const wheelBounds = await table.boundingBox()
        await page.mouse.move(wheelBounds.x+120,wheelBounds.y+100)
        await page.mouse.wheel(0,100); await page.waitForTimeout(350)
        assert.ok(await table.evaluate(el=>el.scrollTop>80))
        assert.equal(await reader.locator('#viewport').evaluate(el=>el.scrollTop),0)
        await table.evaluate(el=>{el.scrollTop=el.scrollHeight})
        await page.mouse.wheel(0,250); await page.waitForTimeout(350)
        assert.ok(await reader.locator('#viewport').evaluate(el=>el.scrollTop>100))
        await reader.locator('#viewport').evaluate(el=>{el.scrollTop=180})
        await table.evaluate(el=>{el.scrollTop=0})
        const topBounds = await table.boundingBox()
        await page.mouse.move(topBounds.x+120,Math.max(50,topBounds.y+180))
        await page.mouse.wheel(0,-200); await page.waitForTimeout(350)
        assert.ok(await reader.locator('#viewport').evaluate(el=>el.scrollTop<180))
        await reader.locator('#viewport').evaluate(el=>{el.scrollTop=0})
        await reader.getByRole('button', { name: 'SQL', exact: true }).click()
        await reader.getByRole('textbox', { name: 'SQL query', exact: true }).fill('SELECT sum(CAST("Value" AS INTEGER)) AS total FROM data')
        await reader.getByRole('button', { name: 'Run query', exact: true }).click()
        await reader.getByRole('cell', {name:'5',exact:true}).waitFor()
      } else {
        await reader.getByRole('button', { name: 'jq', exact: true }).click()
        await reader.getByRole('textbox', { name: 'jq filter', exact: true }).fill('select(.value == 2) | .name')
        await reader.getByRole('button', { name: 'Run filter', exact: true }).click()
        await reader.locator('.jq-results').filter({ hasText: '中文' }).waitFor()
      }
      if (extension === 'md') {
        await page.goto(`http://127.0.0.1:${server.address().port}/ui/styles.css`)
        const controllerSource = await readFile(new URL('ui/document-workspace-controller.mjs',root),'utf8')
        const ids = [...new Set([...controllerSource.matchAll(/\$\('#([\w-]+)'\)/g)].map(match=>match[1]))]
        await page.setContent(`<link rel="stylesheet" href="/ui/styles.css"><div id="fixture">${ids.filter(id=>!['artifact-reader-shell','artifact-content'].includes(id)).map(id=>`<div id="${id}" hidden></div>`).join('')}</div><div id="artifact-reader-shell" class="artifact-reader-shell" style="position:relative;width:700px;height:550px"><article id="artifact-content" style="overflow:auto;height:550px"></article></div>`)
        await page.evaluate(async()=>{
          const {createDocumentWorkspaceController,createDocumentWorkspaceState}=await import('/ui/document-workspace-controller.mjs')
          window.state=createDocumentWorkspaceState();state.activeRightWorkspace='document'
          state.artifact={kind:'text',root:'/tmp',path:'fixture.md',content:'table',size:5,lineCount:1}
          const table='<div class="markdown-table-wrap"><table><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody>'+Array.from({length:60},(_,i)=>`<tr data-source-line="${i+4}"><td>row ${i}</td><td>cell ${i}</td></tr>`).join('')+'</tbody></table></div>'
          window.controller=createDocumentWorkspaceController({state,gatewayFetch:()=>{throw Error('Unexpected request')},randomId:()=>'',reportError:error=>{throw error},view:{disconnectMarkdownImageObserver(){},applyRightRailWidth(){},hydrateMarkdownImages(){},renderMarkdownDocument:()=>({html:table+'<div style="height:3000px"></div>'+table,outline:[]})}})
          await controller.render()
        })
        const small = page.locator('.markdown-resizable-table').first()
        await small.getByRole('separator').first().waitFor({state:'attached'})
        await small.getByRole('separator').first().focus(); await page.keyboard.press('ArrowRight')
        const width=await small.getByRole('separator').first().getAttribute('aria-valuenow')
        await page.locator('#artifact-content').evaluate(el=>{el.scrollTop=el.scrollHeight})
        await page.waitForFunction(()=>!document.querySelector('.markdown-resizable-table').querySelector('.table-column-resize'))
        await page.locator('#artifact-content').evaluate(el=>{el.scrollTop=0})
        await small.getByRole('separator').first().waitFor({state:'attached'})
        assert.equal(await small.getByRole('separator').first().getAttribute('aria-valuenow'),width)
        assert.equal(await small.locator('tr[data-source-line]').count(),60)
        await page.evaluate(()=>{state.artifact=null;controller.render()})
        assert.equal(await page.locator('.table-column-resize,.table-scrollbar').count(),0)
        console.log('small Markdown: resizing, virtual mount persistence and cleanup passed')
      }
      assert.deepEqual(errors, [])
      console.log(`${extension}: browser selection/query UI passed`)
    } finally { await page.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); child.kill() }
  }
} finally { await browser.close(); await rm(temporary, { recursive: true, force: true }) }
