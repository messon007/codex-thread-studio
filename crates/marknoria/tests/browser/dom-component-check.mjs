// Local ordinary-DOM host; only the explicit public module map is served.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import { readFile } from 'node:fs/promises'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
export async function runDomContract({ files, contract, entry, moduleUrl }) {
  const server = createServer(async (request, response) => {
    if (request.url === '/') {
      response.setHeader('Content-Type', 'text/html')
      response.end('<!doctype html><html><body></body></html>')
      return
    }
    if (request.url === '/contract.mjs') {
      response.setHeader('Content-Type', 'text/javascript')
      response.end(
        `import {${entry}} from '${contract}';export const run=()=>${entry}('${moduleUrl}')`
      )
      return
    }
    const file = files.get(request.url)
    if (!file) {
      response.writeHead(404)
      response.end()
      return
    }
    response.setHeader('Content-Type', file.endsWith('.css') ? 'text/css' : 'text/javascript')
    response.end(await readFile(file))
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  let browser
  try {
    const url = `http://127.0.0.1:${server.address().port}/`
    const compact = process.argv.includes('--compact')
    if (process.argv.includes('--webkit')) {
      const result = JSON.parse(
        (
          await promisify(execFile)(
            '/usr/bin/python3',
            ['tests/browser/contract-webkit.py', url, ...(compact ? ['480', '480'] : [])],
            {
              timeout: 190000
            }
          )
        ).stdout
      )
      return { ok: true, ...result }
    }
    const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
    browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
    const page = await browser.newPage(compact ? { viewport: { width: 480, height: 480 } } : {}),
      errors = []
    page.on('pageerror', (e) => errors.push(String(e)))
    await page.goto(url)
    const result = await page.evaluate(async () => (await import('/contract.mjs')).run())
    assert.deepEqual(errors, [])
    return { ok: true, ...result, browserVersion: browser.version() }
  } finally {
    await browser?.close()
    server.closeAllConnections()
    server.close()
  }
}
