import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const directory = await mkdtemp(join(await realpath('target'), 'markdown-resources-'))
const workspace = join(directory, 'workspace'); await mkdir(workspace)
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=', 'base64')
await writeFile(join(workspace, '中文 pic.png'), png)
await writeFile(join(directory, 'private.png'), png)
await writeFile(join(workspace, 'sibling.md'), '# Sibling\n\nOnly an explicit grant may open this.\n')
await writeFile(join(workspace, 'shape.svg'), '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8"><rect width="12" height="8" fill="green"/></svg>')
for (let i = 0; i < 5; i++) await writeFile(join(workspace, `extra-${i}.png`), png)
const source = '\ufeff# Resources 中文😀\r\n\r\n![local](中文%20pic.png)\r\n\r\n![inline](data:image/png;base64,' + png.toString('base64') + ')\r\n\r\n[sibling](sibling.md) [jump](#section-two) [external](https://example.invalid/reference)\r\n\r\n![outside](../private.png) ![remote](https://example.invalid/never-fetch.png)\r\n\r\n## Section two\r\n\r\n| Picture | Meaning |\r\n|---|---|\r\n| ![table](shape.svg) | SVG |\r\n\r\n' + Array.from({length:5},(_,i)=>`![extra ${i}](extra-${i}.png)\r\n\r\n`).join('')
const sourcePath = join(workspace, 'source.md'); await writeFile(sourcePath, source)
const corpusDirectory=join(directory,'corpus');await mkdir(corpusDirectory)
const corpus=JSON.parse((await promisify(execFile)(resolve(process.env.MARKNORIA_CORPUS_BINARY || 'target/debug/examples/reader-corpus'),[corpusDirectory])).stdout)
const children = [], caches = [], urls = []
const parent = createServer(async (request, response) => {
  if (request.url === '/contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript')
    response.end(`import {markdownReferencesContract} from '/markdown-references-contract.mjs';import {rawResourcesContract} from '/raw-resources-contract.mjs';export const run=async()=>({basic:await markdownReferencesContract(${JSON.stringify(urls.slice(0,2))}),raw:await rawResourcesContract(${JSON.stringify(urls.slice(3,6))})})`)
  } else if (request.url === '/markdown-references-contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript'); response.end(await readFile('tests/browser/contracts/markdown-references.mjs'))
  } else if (request.url === '/raw-resources-contract.mjs') {
    response.setHeader('Content-Type','text/javascript');response.end(await readFile('tests/browser/contracts/raw-resources.mjs'))
  } else if (/^\/[a-z-]+\.(mjs|css)$/.test(request.url) || request.url === '/vendor/purify.mjs') {
    try { response.setHeader('Content-Type', request.url.endsWith('.css') ? 'text/css' : 'text/javascript'); response.end(await readFile('web' + request.url)) }
    catch { response.writeHead(404); response.end() }
  } else {
    response.setHeader('Content-Type', 'text/html'); response.end('<!doctype html><html><body></body></html>')
  }
})
parent.listen(0, '127.0.0.1'); await once(parent, 'listening')
const parentOrigin = `http://127.0.0.1:${parent.address().port}`
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
let browser
try {
  for (let i = 0; i < 7; i++) {
    const cache = join(directory, 'cache-' + i); await mkdir(cache); caches.push(cache)
    const standalone=i===2||i===6, root=i>=3?corpusDirectory:workspace
    const input=i===3||i===6?corpus.rawMarkdown:i===4?corpus.sourceImage:i===5?corpus.resolutionMarkdown:sourcePath
    const binary = resolve(standalone ? process.env.MARKNORIA_SERVER_BINARY || 'target/debug/marknoria-server' : process.env.MARKNORIA_EMBEDDED_BINARY || 'target/debug/examples/embedded-host')
    const args = standalone ? [input, '--root', root] : [input, `--parent=${parentOrigin}`, ...([1,3,5].includes(i) ? [`--reference-root=${root}`] : [])]
    const child = spawn(binary, args, { env: { ...process.env, READER_CACHE_DIR: cache }, stdio: ['pipe', 'pipe', 'pipe'] })
    children.push(child)
    let stdout = '', stderr = ''
    child.stdout.on('data', bytes => stdout += bytes); child.stderr.on('data', bytes => stderr += bytes)
    const deadline = Date.now() + 30000
    while (!stdout.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
    assert.match(stdout, /http:\/\/127\.0\.0\.1:/, stderr); urls.push(stdout.trim().split('\n')[0])
  }
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage({ viewport: { width: 1400, height: 1000 } }), errors = [], external = []
  page.on('pageerror', error => errors.push(String(error)))
  page.on('request', request => { if (!request.url().startsWith('http://127.0.0.1:') && !request.url().startsWith('blob:') && !request.url().startsWith('data:')) external.push(request.url()) })
  await page.goto(parentOrigin)
  const {basic:contract,raw:rawContract} = await page.evaluate(async () => (await import('/contract.mjs')).run())
  assert.equal(contract.remountCleanup, true)
  assert.equal(rawContract.sourceResolution,true)
  await page.goto(urls[6])
  await page.locator('[data-image-reference="9"] [data-asset-state="ready"] img').waitFor()
  await page.locator('[data-image-reference="10"] [data-asset-state="ready"] img').waitFor()
  await page.locator('[data-link-reference="0"]').click()
  await page.locator('[data-document-anchor="custom-heading"]').waitFor()
  assert.equal(await page.evaluate(()=>window.bad),undefined)
  await page.locator('[data-link-reference="8"]').click()
  assert.equal(await page.evaluate(()=>window.bad),undefined)
  // Test actual virtual reader DOM as well as the ordinary component above.
  await page.goto(urls[2])
  await page.locator('[data-image-reference="0"] [data-asset-state="ready"] img').waitFor()
  await page.locator('[data-image-reference="1"] [data-asset-state="ready"] img').waitFor()
  await page.locator('[data-link-reference="3"]').click()
  await page.locator('#heading-1').waitFor()
  await page.locator('[data-image-reference="7"] [data-asset-state="ready"] img').waitFor()
  assert.equal(await page.locator('[data-image-reference="7"] img').evaluate(image => image.naturalWidth), 12)
  const uri = new URL(urls[2]), headers = { 'x-reader-token': uri.hash.slice(1) }
  const documentResponse = await fetch(new URL('/api/document', uri), { headers }).then(response => response.json())
  assert.equal(documentResponse.state, 'ready')
  const index = documentResponse.index
  const referenceUrl = new URL(`/api/reference?id=0&revision=${index.revision}`, uri)
  assert.equal((await fetch(referenceUrl)).status, 401)
  assert.equal((await fetch(referenceUrl, { headers: { ...headers, Origin: 'https://untrusted.invalid' } })).status, 403)
  assert.equal((await fetch(new URL('/api/reference?id=0&revision=' + '0'.repeat(64), uri), { headers })).status, 409)
  for (const id of [5, 6]) assert.equal((await fetch(new URL(`/api/reference-image?id=${id}&revision=${index.revision}`, uri), { headers })).status, 403)
  assert.deepEqual(external, [])
  await page.locator('[data-link-reference="2"]').click()
  await page.getByRole('heading', { name: 'Sibling', exact: true }).waitFor()
  assert.deepEqual(errors, [])
  let webkit
  if (process.argv.includes('--webkit')) {
    webkit = JSON.parse((await promisify(execFile)('/usr/bin/python3', ['tests/browser/contract-webkit.py', parentOrigin], { timeout: 190000 })).stdout)
    assert.equal(webkit.basic.remountCleanup, true)
    assert.equal(webkit.raw.sourceResolution,true)
  }
  // Force actual cache eviction, then reject frames tied to the old snapshot.
  const embedded = new URL(urls[1]), scopedHeaders = { 'x-reader-token': embedded.hash.slice(1) }
  const owned = await fetch(new URL('/api/document', embedded), {headers:scopedHeaders}).then(response=>response.json())
  const readyResource = async id => {
    const uri = new URL(`/api/reference-image?id=${id}&revision=${owned.index.revision}`, embedded)
    const deadline = Date.now()+30000
    while (Date.now()<deadline) {
      const response = await fetch(uri,{headers:scopedHeaders})
      if (response.status === 429) { await pause(30); continue }
      assert.ok(response.ok,await response.clone().text())
      const value = await response.json()
      if (value.state === 'ready') return value
      assert.equal(value.state,'indexing'); await pause(30)
    }
    throw Error('Image snapshot did not become ready')
  }
  const previous = await readyResource(0)
  for (let id = 8; id < 13; id++) await readyResource(id)
  await writeFile(join(workspace,'中文 pic.png'),Buffer.concat([png,Buffer.from('changed source snapshot')]))
  const next = await readyResource(0)
  assert.notEqual(previous.imageRevision,next.imageRevision)
  assert.equal((await fetch(new URL(`/api/reference-image?id=0&revision=${owned.index.revision}&imageRevision=${previous.imageRevision}&frame=0`,embedded),{headers:scopedHeaders})).status,409)
  console.log(JSON.stringify({ ok: true, browserVersion: browser.version(), contract, rawContract, standalone: true, snapshotEviction:true, webkit, artifacts: directory }))
} finally {
  await browser?.close()
  for (const [i, child] of children.entries()) if (child.exitCode == null && child.signalCode == null) {
    const exited = once(child, 'exit')
    if (i === 2 || i===6) child.kill('SIGINT'); else child.stdin.end('\n')
    await exited
    assert.equal(child.exitCode, 0)
  }
  parent.closeAllConnections(); parent.close()
  for (const cache of caches) assert.deepEqual(await readdir(cache), [])
  assert.equal(await readFile(sourcePath, 'utf8'), source)
}
