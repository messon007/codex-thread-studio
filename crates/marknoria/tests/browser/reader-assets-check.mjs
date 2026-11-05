// A public ordinary host imports exact shared components without a file grant.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { projectionViewContract } from './contracts/projection-view.mjs'
import { presentationContract } from './contracts/presentation.mjs'
import { tableViewContract } from './contracts/table-view.mjs'
import { jqQueryContract } from './contracts/jq-query.mjs'
import { notesContract } from './contracts/notes.mjs'
const child = spawn(resolve(process.argv[2] || 'target/debug/examples/reader-assets-host'), [], {
  stdio: ['pipe', 'pipe', 'pipe']
})
let output = '',
  errors = '',
  browser,
  assertions = 0
child.stdout.on('data', (bytes) => (output += bytes))
child.stderr.on('data', (bytes) => (errors += bytes))
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
try {
  const deadline = Date.now() + 20000
  while (!output.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
  assert.match(output, /http:\/\/127\.0\.0\.1:/, errors)
  const url = new URL(output.trim()),
    base = new URL('reader-assets/', url)
  for (const path of [
    'api/document',
    'api/source',
    'api/edit',
    'api/original',
    'api/epub',
    'api/table-query',
    'api/jq-query',
    'api/files',
    'api/open',
    'src/core/mod.rs',
    '../Cargo.toml'
  ]) {
    assert.equal((await fetch(new URL(path, base))).status, 404, path)
    assertions++
  }
  for (const name of [
    'projection-view.mjs',
    'projection-html.mjs',
    'document-prose.css',
    'host-client.mjs',
    'pdf-reader.mjs',
    'epub-reader.mjs',
    'notes.mjs',
    'table-query.mjs',
    'jq-query.mjs'
  ]) {
    const response = await fetch(new URL(name, base))
    assert.equal(response.status, 200)
    assert.equal(await response.text(), await readFile('web/' + name, 'utf8'))
    assertions++
  }
  const pdf = await fetch(new URL('vendor/pdf/pdf.worker.mjs', base))
  assert.equal(pdf.status, 200)
  assert.match(pdf.headers.get('content-type'), /javascript/)
  assert.ok((await pdf.arrayBuffer()).byteLength > 1000)
  assertions++
  assert.equal((await fetch(new URL('vendor/pdf/unknown-file', base))).status, 404)
  assertions++
  assert.equal(
    await (await fetch(new URL('engine.mjs', base))).text(),
    await readFile('assets/diagrams/engine.mjs', 'utf8')
  )
  assertions++
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage({ viewport: { width: 1400, height: 1000 } }),
    failures = []
  page.on('pageerror', (error) => failures.push(String(error)))
  await page.goto(url.href)
  const prefix = '/reader-assets/'
  const projection = await page.evaluate(projectionViewContract, prefix + 'projection-view.mjs')
  const presentation = await page.evaluate(presentationContract, prefix + 'presentation.mjs')
  const table = await page.evaluate(tableViewContract, prefix + 'table-view.mjs')
  const jq = await page.evaluate(jqQueryContract, prefix + 'jq-query.mjs')
  const notes = await page.evaluate(notesContract, prefix + 'notes.mjs')
  await page.evaluate(async () => {
    await import('/reader-assets/pdf-reader.mjs')
    await import('/reader-assets/epub-reader.mjs')
  })
  const diagram = await page.evaluate(async () => {
    const { createDiagramRenderer } = await import('/reader-assets/engine.mjs')
    const engine = createDiagramRenderer()
    try {
      return await engine.render('mermaid', 'flowchart LR\n  A-->B', 'sdk-asset-diagram')
    } finally {
      engine.dispose()
    }
  })
  assert.ok(diagram.svg.includes('<svg') && diagram.elements > 0)
  assertions++
  assert.deepEqual(failures, [])
  console.log(
    JSON.stringify({
      ok: true,
      noDocumentGrant: true,
      nestedAssets: true,
      assertions:
        assertions +
        projection.assertions +
        presentation.assertions +
        table.assertions +
        jq.assertions +
        notes.assertions,
      browserVersion: browser.version()
    })
  )
} finally {
  await browser?.close()
  if (child.exitCode == null && child.signalCode == null) {
    const exited = once(child, 'exit')
    child.stdin.end('\n')
    await exited
    assert.equal(child.exitCode, 0, errors)
  }
}
