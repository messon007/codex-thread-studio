import { projectionViewContract } from './contracts/projection-view.mjs'
import { jqQueryContract } from './contracts/jq-query.mjs'
import { tableControlsContract } from './contracts/table-controls.mjs'
import { translationContract } from './contracts/translation.mjs'
import { notesContract } from './contracts/notes.mjs'
import { tableQueryContract } from './contracts/table-query.mjs'
import { tableViewContract } from './contracts/table-view.mjs'
import { tablePlotContract } from './contracts/table-plot.mjs'
import { tableCellsContract } from './contracts/table-cells.mjs'
import { diagramLifecycleContract } from './contracts/diagram-lifecycle.mjs'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve, basename } from 'node:path'
import { tableFixture } from './table-fixture.mjs'
import { tableComponentContract } from './contracts/table-layout.mjs'

const base = await realpath('target'); await mkdir(join(base, 'web-table-checks'), { recursive: true })
const dir = await mkdtemp(join(base, 'web-table-checks/run-')), file = join(dir, 'tables.md'), cache = join(dir, 'cache')
await mkdir(cache); await writeFile(file, tableFixture)
const server = spawn(resolve(process.argv[2] || 'target/release/marknoria-server'), [file], { env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR:join(dir,'config') }, stdio: ['ignore', 'pipe', 'pipe'] })
let output = '', errors = ''; server.stdout.on('data', b => { output += b }); server.stderr.on('data', b => { errors += b })
let browser
try {
  const deadline = Date.now() + 15000
  while (!output.includes('http://') && Date.now() < deadline) await new Promise(r => setTimeout(r, 30))
  assert.match(output, /http:\/\/127\.0\.0\.1:/, errors)
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const baseUrl = new URL(output.trim().split('\n')[0])
  const checkedAssets = ['document-prose.css','projection-view.mjs','projection-view.css','projection-html.mjs','notes.mjs','translation.mjs','notes.css','reader.mjs','public-reader.mjs','component-styles.mjs','epub-reader.mjs','epub-reader.css','table-controls.mjs','table-view.mjs','table-query.mjs','table-layout.mjs','table-cells.mjs','table-plot.mjs','presentation.mjs','reader-ui.mjs','pdf-reader.mjs','jq-query.mjs','jq-query.css','index.html','styles.css']
  for(const asset of checkedAssets)assert.equal(await (await fetch(new URL(asset==='index.html'?'./':asset,baseUrl))).text(),await readFile(join('web',asset),'utf8'),`Release asset differs from checkout: ${asset}`)
  const page = await browser.newPage({ viewport: { width: 1200, height: 800 } })
  const pageErrors = []; page.on('pageerror', e => pageErrors.push(String(e)))
  // Explicit local-asset override for developing UI before a Rust asset rebuild.
  if (process.env.MARKNORIA_TABLE_DEV_ASSETS === '1') {
    await page.route(/\/(?:table-layout|tables|reader)\.mjs$|\/styles\.css$/, async route => {
      const path = join('web', basename(new URL(route.request().url()).pathname))
      await route.fulfill({ body: await readFile(path), contentType: path.endsWith('.css') ? 'text/css' : 'text/javascript' })
    })
  }
  await page.exposeFunction('marknoriaTableInput', async v => {
    await page.mouse.move(v.x, v.y)
    if (v.type === 'button') await (v.amount ? page.mouse.down() : page.mouse.up())
    else if (v.type === 'double') await page.mouse.dblclick(v.x, v.y)
    else if (v.type === 'axis') {
      if (v.shifted) await page.keyboard.down('Shift')
      try { const amount = v.amount * (v.smooth ? 1 : 10); await page.mouse.wheel(v.axis ? amount : 0, v.axis ? 0 : amount) }
      finally { if (v.shifted) await page.keyboard.up('Shift') }
    }
    await page.waitForTimeout(100)
  })
  await page.goto(output.trim().split('\n')[0])
  await page.waitForFunction(() => window.readerExperiment?.metrics.firstContentMs != null)
  await page.evaluate(await readFile('tests/desktop/table-check.js', 'utf8'))
  const report = await page.evaluate(() => window.runTableChecks())
  const component = await page.evaluate(tableComponentContract, '/table-layout.mjs')
  const cells = await page.evaluate(tableCellsContract, '/table-cells.mjs')
  const toolbar=await page.evaluate(tableControlsContract,'/table-controls.mjs')
  const translation=await page.evaluate(translationContract,'/translation.mjs')
  const view = await page.evaluate(tableViewContract, '/table-view.mjs')
  const projectionComponent = await page.evaluate(projectionViewContract, '/projection-view.mjs')
  const queryComponent = await page.evaluate(tableQueryContract, '/table-query.mjs')
  const jqComponent = await page.evaluate(jqQueryContract, '/jq-query.mjs')
  const notesComponent = await page.evaluate(notesContract, '/notes.mjs')
  const plots = await page.evaluate(tablePlotContract, '/table-plot.mjs')
  const diagrams = await page.evaluate(diagramLifecycleContract, '/diagrams.mjs')
  await page.screenshot({ path: join(dir, 'tables.png') })
  // Window changes preserve fixed columns; widths stay isolated to each reader.
  const widths = () => page.locator('article[data-table="0"] thead th').evaluateAll(cols => cols.map(c => c.getBoundingClientRect().width))
  const before = await widths()
  await page.setViewportSize({ width: 640, height: 650 }); await page.waitForTimeout(250)
  assert.deepEqual(await widths(), before)
  const other = await browser.newPage()
  await other.goto(output.trim().split('\n')[0]); await other.waitForFunction(() => document.querySelector('article[data-table="0"][data-ready="true"]'))
  assert.ok(await other.locator('article[data-table="0"] thead th').first().evaluate(c => c.getBoundingClientRect().width) <= 100)
  assert.deepEqual(pageErrors, [])
  const url = new URL(output.trim().split('\n')[0]), token = url.hash.slice(1)
  for (const side of ['left', 'right']) {
    const response = await fetch(new URL('/api/panes', url), { method: 'POST', headers: { 'x-reader-token': token, 'Content-Type': 'application/json' }, body: JSON.stringify({ side, path: 'tables.md' }) })
    assert.equal(response.status, 200)
  }
  const compare = await browser.newPage({ viewport: { width: 1500, height: 900 } })
  compare.on('pageerror', e => pageErrors.push(String(e)))
  await compare.goto(new URL(`./compare.html#${token}`, url).href)
  await compare.waitForFunction(() => window.comparisonReader?.metadata)
  for (const side of ['left', 'right']) await compare.evaluate(side => comparisonReader.setMode(side, 'preview'), side)
  const frames = {}
  for (const side of ['left', 'right']) {
    frames[side] = await (await compare.$(`#${side}-preview`)).contentFrame()
    await frames[side].waitForFunction(() => window.readerExperiment?.metrics.firstContentMs != null)
    await frames[side].evaluate(() => readerExperiment.jump(readerExperiment.index.tables[0].firstPage, null, 0))
  }
  const paneWidths = frame => frame.locator('article[data-table="0"] thead th').evaluateAll(cells => cells.map(c => c.getBoundingClientRect().width))
  const right = await paneWidths(frames.right), left = await paneWidths(frames.left)
  const edge = await frames.left.locator('.table-column-resize').first().boundingBox()
  await compare.mouse.move(edge.x + 4, edge.y + 20); await compare.mouse.down()
  await compare.mouse.move(edge.x + 36, edge.y + 20, { steps: 5 }); await compare.mouse.up()
  assert.ok(Math.abs((await paneWidths(frames.left))[0] - left[0] - 32) < 1.5)
  assert.deepEqual(await paneWidths(frames.right), right)
  await compare.evaluate(async () => { await comparisonReader.setMode('left', 'source'); await comparisonReader.setMode('left', 'preview') })
  await frames.left.waitForFunction(() => document.querySelector('article[data-table="0"][data-ready="true"]'))
  assert.ok(Math.abs((await paneWidths(frames.left))[0] - left[0] - 32) < 1.5)
  await compare.screenshot({ path: join(dir, 'comparison.png') })
  assert.deepEqual(pageErrors, [])
  console.log(JSON.stringify({ ok: true, ...report, assertions: report.assertions + projectionComponent.assertions + jqComponent.assertions + toolbar.assertions + translation.assertions + 8 + component.assertions + cells.assertions + plots.assertions + diagrams.assertions + view.assertions + queryComponent.assertions + notesComponent.assertions + checkedAssets.length, checkedAssets, projectionComponent, jqComponent, toolbar, translation, notesComponent, queryComponent, view, sharedComponent: component, cells, plots, diagrams, artifacts: dir }))
} catch (error) { console.error('Web table artifacts:', dir, errors); throw error }
finally {
  await browser?.close()
  if (server.exitCode == null && server.signalCode == null) { const exited = once(server, 'exit'); server.kill('SIGINT'); await exited }
  assert.equal(await readFile(file, 'utf8'), tableFixture)
  assert.deepEqual(await readdir(cache), [])
}
