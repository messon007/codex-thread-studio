import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const directory = await mkdtemp(join(await realpath('target'), 'embedded-browser-'))
const binary = resolve(process.argv.slice(2).find(value => !value.startsWith('--')) || 'target/debug/examples/embedded-host')
const text = '\ufeff# Public 中文😀\r\n\r\nParagraph needle.\r\n\r\n## Data\r\n\r\n| name | value |\r\n| --- | --- |\r\n' + Array.from({ length: 120 }, (_, n) => `| row ${n} | ${n} |\r\n`).join('')
const sources = [join(directory, 'editable.md'), join(directory, 'readonly.md'), join(directory, 'pixels.png')]
for (const path of sources.slice(0, 2)) await writeFile(path, text)
await writeFile(sources[2], Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=', 'base64'))
const children = [], caches = [], urls = [], errors = []
let browser
const parent = createServer(async (request, response) => {
  if (request.url === '/contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript')
    response.end(`import { embeddedImageContract } from '/embedded-image-contract.mjs'; import { embeddedSourceContract } from '/embedded-source-contract.mjs'; export const run = async () => ({image:await embeddedImageContract('/image-view.mjs', ${JSON.stringify(urls[2])}), source:await embeddedSourceContract('/host-client.mjs', ${JSON.stringify(urls[1])})});`)
    return
  }
  if (request.url === '/embedded-image-contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript')
    response.end(await readFile('tests/browser/contracts/embedded-image.mjs'))
    return
  }
  if (request.url === '/embedded-source-contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript')
    response.end(await readFile('tests/browser/contracts/embedded-source.mjs'))
    return
  }

  if (['/image-view.mjs','/image-view.css','/image-frames.mjs','/assets.mjs','/document-prose.css','/projection-view.mjs','/projection-view.css','/projection-html.mjs','/host-client.mjs','/reader-bridge.mjs','/table-view.mjs','/table-view.css','/table-query.mjs','/table-query.css','/query-operation.mjs','/table-plot.mjs','/table-plot.css','/table-controls.mjs','/table-layout.mjs','/table-cells.mjs','/table-cells.css','/clipboard.mjs','/icons.mjs','/component-styles.mjs','/vendor/purify.mjs','/notes.mjs','/translation.mjs','/notes.css'].includes(request.url)) {
    response.setHeader('Content-Type', request.url.endsWith('.css')?'text/css':'text/javascript'); response.end(await readFile('web' + request.url)); return
  }
  response.setHeader('Content-Type', 'text/html'); response.end('<!doctype html><html><body><h1>Public embedding host</h1><div id="readers"></div></body></html>')
})
parent.listen(0, '127.0.0.1'); await once(parent, 'listening')
const parentOrigin = `http://127.0.0.1:${parent.address().port}`
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
try {
  for (let i = 0; i < sources.length; i++) {
    const cache = join(directory, `cache-${i}`); await mkdir(cache); caches.push(cache)
    const child = spawn(binary, [sources[i], ...(i === 0 ? ['--write'] : i === 1 ? [`--notes=${join(directory,'notes-1')}`] : []), `--parent=${parentOrigin}`], { env: { ...process.env, READER_CACHE_DIR: cache }, stdio: ['pipe', 'pipe', 'pipe'] })
    children.push(child); let stdout = '', stderr = ''
    child.stdout.on('data', b => stdout += b); child.stderr.on('data', b => stderr += b)
    const deadline = Date.now() + 30000
    while (!stdout.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
    assert.match(stdout, /http:\/\/127\.0\.0\.1:/, stderr); urls.push(stdout.trim())
  }
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } })
  page.on('pageerror', error => errors.push(error.stack || String(error)))
  await page.goto(parentOrigin)
  const states = await page.evaluate(async urls => {
    const { createReaderClient } = await import('/host-client.mjs')
    window.clients = []; window.framesForTest = []; window.eventsForTest = []
    for (const [i, url] of urls.entries()) {
      const frame = document.createElement('iframe'); frame.title = `Reader ${i}`; frame.src = url; frame.style.cssText = 'width:740px;height:850px;border:0'; document.getElementById('readers').append(frame)
      framesForTest.push(frame); clients.push(createReaderClient(frame, { onEvent: (type, payload) => eventsForTest.push({ i, type, payload }) }))
    }
    return Promise.all(clients.map(client => client.ready()))
  }, urls)
  assert.equal(states[0].view, 'preview'); assert.equal(states[0].capabilities.edit, true); assert.equal(states[1].capabilities.edit, false)
  assert.equal(states[0].capabilities.sourceRangeNavigation,true)
  assert.notEqual(states[0].documentId, states[1].documentId)
  assert.deepEqual(Object.keys(states[0].position).sort(), ['location', 'resourceId', 'revision', 'version', 'view'])
  assert.equal(typeof states[0].timings.indexMs, 'number')
  assert.equal(typeof states[0].timings.indexPeakRssKiB, 'number')
  await page.waitForFunction(() => eventsForTest.some(e => e.i === 0 && e.type === 'state' && e.payload.timings.firstUsefulContentMs != null))
  const command = (method, params = {}, i = 0) => page.evaluate(async ({ method, params, i }) => clients[i].request(method, params), { method, params, i })
  assert.equal(states[2].document.assets, 1)
  const imageInfo = await command('assetInfo', { id: 0 }, 2)
  assert.equal(imageInfo.revision, states[2].revision)
  assert.deepEqual([imageInfo.width, imageInfo.height, imageInfo.frames.length], [2, 1, 1])
  assert.equal(imageInfo.dimensionsExact, true)
  assert.deepEqual(Object.keys(imageInfo.frames[0]).sort(), ['delayMs', 'frame'])
  assert.equal(imageInfo.alt, 'pixels.png')
  const imagePayload = await page.evaluate(async () => {
    const result = await clients[2].request('assetFrame', { id: 0, frame: 0 })
    const bytes = new Uint8Array(await result.blob.arrayBuffer())
    return { revision: result.revision, id: result.id, frame: result.frame,
      type: result.blob.type, size: result.blob.size, signature: [...bytes.subarray(0, 8)] }
  })
  assert.equal(imagePayload.revision, states[2].revision)
  assert.equal(imagePayload.id, 0); assert.equal(imagePayload.frame, 0)
  assert.equal(imagePayload.type, 'image/png'); assert.ok(imagePayload.size > 0 && imagePayload.size < 1024)
  assert.deepEqual(imagePayload.signature, [137, 80, 78, 71, 13, 10, 26, 10])
  assert.equal(await command('assetFrame', { id: 0, frame: 1 }, 2).then(() => false, () => true), true)
  assert.equal(await command('assetFrame', { id: 0, frame: 0 }).then(() => false, () => true), true)
  assert.equal(await command('assetInfo', { id: 0 }).then(() => false, () => true), true)
  assert.equal(await command('assetInfo', { id: 1 }, 2).then(() => false, () => true), true)
  assert.equal(await command('assetInfo', { id: -1 }, 2).then(() => false, () => true), true)
  const imageFrame = page.frames().find(frame => frame.url() === urls[2])
  await imageFrame.locator('figure[data-asset-state="ready"] img').waitFor()
  assert.equal(await imageFrame.locator('figure img').evaluate(image => image.naturalWidth), 2)
  const ordinaryImage = await page.evaluate(async owner => {
    const { mountImageView, readerImageProvider } = await import('/image-view.mjs')
    const viewport = document.createElement('div'), root = document.createElement('div'), neighbor = document.createElement('span')
    viewport.style.cssText = 'height:250px;width:600px;overflow:auto'
    neighbor.textContent = 'Host content'
    root.append(neighbor); viewport.append(root); document.body.prepend(viewport)
    let changes = 0
    const provider = await readerImageProvider(clients[2], owner, 0)
    const first = mountImageView(root, { ...provider, viewport, onChange() { changes++ } })
    await first.ready
    const second = mountImageView(root, { ...provider, viewport })
    await second.ready
    if (first.root.querySelectorAll('.asset-controls').length !== 1) throw Error('Image mount crossed its owning figure')
    const plus = first.root.querySelectorAll('button')[2]
    plus.click()
    const result = { naturalWidth: first.root.querySelector('img').naturalWidth,
      zoom: first.state().zoom, otherZoom: second.state().zoom, controls: root.querySelectorAll('.asset-controls').length }
    const beforeDispose = changes
    first.dispose(); plus.click(); second.dispose()
    result.noLateCallbacks = changes === beforeDispose
    result.hostRetained = root.firstChild === neighbor && root.children.length === 1
    result.bitmapsReleased = !first.root.querySelector('img') && !second.root.querySelector('img')
    viewport.remove()
    return result
  }, states[2])
  assert.deepEqual(ordinaryImage, { naturalWidth: 2, zoom: 1.25, otherZoom: null, controls: 2, noLateCallbacks: true, hostRetained: true, bitmapsReleased: true })
  const blockedFrames = [], framePattern = new URL('api/asset', urls[2]).href + '?**'
  await page.route(framePattern, route => { blockedFrames.push(route) })
  await page.evaluate(() => {
    window.frameSignals = [new AbortController(), new AbortController()]
    window.frameRequests = frameSignals.map(controller => clients[2].request('assetFrame', { id: 0, frame: 0 }, { signal: controller.signal }).then(() => 'success', error => error.name))
  })
  const frameDeadline = Date.now() + 5000
  while (blockedFrames.length < 2 && Date.now() < frameDeadline) await pause(10)
  assert.equal(blockedFrames.length, 2)
  const frameStatus = () => page.evaluate(() => clients[2].request('assetFrame', { id: 0, frame: 0 }).then(result => ({ frame: result.frame }), error => ({ error: error.code })))
  assert.equal((await frameStatus()).error, 'BUSY')
  assert.deepEqual(await page.evaluate(async () => { frameSignals.forEach(controller => controller.abort()); return Promise.all(frameRequests) }), ['AbortError', 'AbortError'])
  for (const route of blockedFrames) await route.abort().catch(() => {})
  await page.unroute(framePattern)
  let recoveredFrame
  const recoveryDeadline = Date.now() + 3000
  while (Date.now() < recoveryDeadline) {
    recoveredFrame = await frameStatus()
    if (!recoveredFrame.error) break
    assert.equal(recoveredFrame.error, 'BUSY')
    await pause(20)
  }
  assert.equal(recoveredFrame?.frame, 0)
  if (process.argv.includes('--webkit')) {
    const native = await promisify(execFile)('/usr/bin/python3', ['tests/browser/contract-webkit.py', parentOrigin], { timeout: 190000 })
    const result = JSON.parse(native.stdout)
    assert.equal(result.image.publicImageProtocol, true)
    assert.equal(result.image.blobTransport, true)
    assert.equal(result.image.assertions, 8)
    assert.equal(result.source.sourceByteRoundTrip, true)
    assert.equal(result.source.assertions, 8)
  }



  assert.equal(states[0].document.sourceBytes,Buffer.byteLength(text))
  assert.equal(states[0].outline.total,2)
  assert.equal((await command('contents',{open:true})).outline.open,true)
  assert.equal((await command('state',{},1)).outline.open,false)
  assert.equal((await command('contents',{open:false})).outline.open,false)
  const rowMatch=(await command('search',{query:'row 119'})).matches[0]
  assert.deepEqual(rowMatch.location,{kind:'cell',table:0,row:119,column:0})
  assert.equal(rowMatch.locationPrecision,'row')
  const projection = await command('projectionWindow',{start:0,count:5})
  assert.equal(projection.revision,states[0].revision)
  assert.equal(projection.pages[0].sourceStartByte,Buffer.byteLength('\ufeff'))
  assert.equal(projection.pages[0].sourceLine,1)
  assert.ok(projection.pages.some(page=>page.table===0))
  const originalSource = await command('sourceWindow',{line:1})
  assert.equal(originalSource.result.text,text)
  assert.equal(originalSource.revision,states[0].revision)
  const emoji=text.replaceAll('\r\n','\n').indexOf('😀')
  const sourceSelection=await command('sourceSelection',{start:originalSource.result.start,end:originalSource.result.end,fromUtf16:emoji,toUtf16:emoji+2})
  assert.equal(sourceSelection.quote,'😀')
  assert.equal(sourceSelection.location.startByte,Buffer.from(text).indexOf(Buffer.from('😀')))
  assert.equal((await command('sourceWindow',{offset:sourceSelection.location.startByte,end:sourceSelection.location.endByte})).result.text,'😀')
  assert.equal(await page.evaluate(()=>clients[0].request('sourceWindow',{offset:0,end:128*1024+1}).then(()=>false,()=>true)),true)
  const restoredSource = await command('navigate',{location:sourceSelection.location})
  assert.equal(restoredSource.view,'source')
  const restoredSelection = await command('selection')
  assert.equal(restoredSelection.quote,'😀')
  assert.deepEqual(restoredSelection.location,sourceSelection.location)
  assert.equal(restoredSelection.precision,'exact')
  assert.equal(await page.evaluate(()=>clients[0].request('navigate',{location:{kind:'source',startByte:1,endByte:2}}).then(()=>false,()=>true)),true)
  await command('setView',{view:'preview'})
  assert.equal(await page.evaluate(()=>clients[0].request('projectionWindow',{start:0,count:6}).then(()=>false,()=>true)),true)
  assert.equal(await page.evaluate(()=>clients[0].request('projectionWindow',{start:0,count:1,revision:'b'.repeat(64)}).then(()=>false,()=>true)),true)
  const ordinaryProjection=await page.evaluate(async owner=>{
    const {mountProjectionView,readerProjectionProvider}=await import('/projection-view.mjs')
    const viewport=document.createElement('div');viewport.style.cssText='height:300px;overflow:auto';document.body.prepend(viewport)
    const view=mountProjectionView(viewport,{provider:readerProjectionProvider(clients[0],owner),viewport})
    await view.show(0,5)
    const deadline=performance.now()+5000
    while(!view.root.querySelector('tbody tr')&&performance.now()<deadline)await new Promise(resolve=>setTimeout(resolve,20))
    const result={heading:view.root.querySelector('h1')?.textContent,tables:view.root.querySelectorAll('[data-marknoria-table-view]').length,first:view.root.querySelector('tbody tr')?.cells[0].textContent,state:view.state()}
    view.dispose();viewport.remove();return result
  },states[0])
  assert.equal(ordinaryProjection.heading,'Public 中文😀')
  assert.equal(ordinaryProjection.tables,1)
  assert.equal(ordinaryProjection.first,'row 0')
  const ordinary = await page.evaluate(async owner => {
    const { mountTableView, readerTableProvider } = await import('/table-view.mjs')
    const workspace=document.createElement('div'),viewport=document.createElement('div'),article=document.createElement('article')
    workspace.style.cssText='position:relative;width:650px;height:250px';viewport.style.cssText='height:100%;overflow:auto';viewport.append(article);workspace.append(viewport);document.body.prepend(workspace)
    const provider=await readerTableProvider(clients[0], owner, 0)
    const table=mountTableView(article,{...provider,viewport,workspace})
    await table.ready;await table.go(119,1)
    const selection=table.selection(), state=table.state()
    table.dispose();workspace.remove()
    return { selection,state }
  },states[0])
  assert.equal(ordinary.state.totalRows,120)
  assert.equal(ordinary.selection.location.row,119)
  assert.equal(ordinary.selection.location.column,1)
  assert.equal(ordinary.selection.quote,'119')
  assert.equal(await page.evaluate(()=>clients[0].request('tableWindow',{table:0,row:0,columns:[0,0]}).then(()=>false,()=>true)),true)
  const ordinaryQuery = await page.evaluate(async owner => {
    const {mountTableQuery,readerQueryProvider}=await import('/table-query.mjs')
    const query=mountTableQuery(document.body,{provider:readerQueryProvider(clients[0],owner,0),revision:owner.revision})
    await query.ready;await query.go(119)
    const state=query.state(),dialog=document.querySelector('.table-query-dialog')
    const lastRows=dialog.querySelectorAll('tbody tr').length
    dialog.querySelector('.table-chart-open').click()
    const deadline=performance.now()+5000
    while(!document.querySelector('.table-plot-dialog [data-chart-row="119"]')&&performance.now()<deadline)await new Promise(r=>setTimeout(r,20))
    const points=document.querySelectorAll('.table-plot-dialog rect').length
    await query.dispose()
    const missing=await clients[0].request('sqlPage',{id:state.id,row:0}).then(()=>false,()=>true)
    return {state,lastRows,points,closed:missing}
  },states[0])
  assert.equal(ordinaryQuery.state.totalRows,120)
  assert.equal(ordinaryQuery.state.rowStart,100)
  assert.equal(ordinaryQuery.lastRows,20)
  assert.equal(ordinaryQuery.points,120)
  assert.equal(ordinaryQuery.closed,true)
  assert.equal(await page.evaluate(()=>clients[0].request('notes',{request:{operation:'list',offset:0}}).then(()=>false,()=>true)),true)
  assert.equal(states[0].capabilities.queryNotes,false);assert.equal(states[1].capabilities.queryNotes,true)
  const durableQueryNote=await page.evaluate(async owner=>{
    const {mountTableQuery,readerQueryProvider}=await import('/table-query.mjs')
    const {mountNotes,readerNoteStorage}=await import('/notes.mjs')
    const root=document.createElement('section'),toolbar=document.createElement('nav');root.append(toolbar);document.body.append(root)
    const query=mountTableQuery(root,{provider:readerQueryProvider(clients[1],owner,0),revision:owner.revision})
    const notes=mountNotes({owner,root,toolbar,storage:readerNoteStorage(clients[1],owner),queryNotes:owner.capabilities.queryNotes,
      selection:async()=>{const value=query.selection();return value?{...value,documentId:owner.documentId,revision:owner.revision}:null}})
    await query.ready;await query.execute('SELECT c1,c2 FROM t ORDER BY CAST(c2 AS INTEGER)');await query.go(119)
    root.querySelector('tbody tr:last-child td:last-child').click();await notes.selectionChanged()
    const menu=root.querySelector('.notes-toolbar');[...menu.querySelectorAll('button')].find(button=>button.textContent==='Favorite').click()
    const deadline=performance.now()+5000
    while(notes.state().busy&&performance.now()<deadline)await new Promise(resolve=>setTimeout(resolve,20))
    await query.dispose()
    const result=await clients[1].request('notes',{request:{operation:'list',offset:0}})
    const note=result.items[0]
    const copy=await clients[1].request('notes',{request:{operation:'copy',id:note.id}})
    notes.dispose();root.remove()
    return {note,text:copy.text}
  },states[1])
  assert.equal(durableQueryNote.note.quote,'119');assert.equal(durableQueryNote.note.anchor.query.row,119)
  assert.equal(durableQueryNote.note.anchor.query.column,1);assert.equal(durableQueryNote.note.anchor.query.sourceRows,120)
  assert.equal(durableQueryNote.note.anchor.location,null);assert.ok(durableQueryNote.text.includes('snapshot, not a source cell'))
  const wrongVersion = await page.evaluate(() => new Promise(resolve => {
    const requestId = crypto.randomUUID()
    const receive = event => {
      if (event.source === framesForTest[0].contentWindow && event.data?.requestId === requestId) { removeEventListener('message', receive); resolve(event.data.error?.code) }
    }
    addEventListener('message', receive)
    framesForTest[0].contentWindow.postMessage({ protocol: 'marknoria.reader', version: 99, kind: 'request', requestId, method: 'state' }, new URL(framesForTest[0].src).origin)
  }))
  assert.equal(wrongVersion, 'VERSION')
  await page.evaluate(() => postMessage({ protocol: 'marknoria.reader', version: 1, kind: 'event', type: 'spoof', payload: {} }, location.origin))
  assert.equal(await page.evaluate(() => eventsForTest.some(e => e.type === 'spoof')), false)
  assert.equal((await command('outline')).total, 2)
  assert.equal((await command('search', { query: 'needle' })).matches.length, 1)
  assert.equal((await command('sql', { table: 0, sql: 'SELECT COUNT(*) FROM data' })).rows[0][0], '120')
  const paged = await command('sql', { table: 0, sql: 'SELECT * FROM data' })
  assert.equal(paged.totalRows, 120)
  assert.equal(states[0].capabilities.charts,true)
  const chart=await command('tableChart',{source:{kind:'table',table:0},x:0,y:1})
  assert.equal(chart.points.length,120);assert.equal(chart.points[119].value,119);assert.equal(chart.datasetScope,'complete-source-table')
  const queryChart=await command('tableChart',{source:{kind:'query',queryId:paged.id},x:2,y:3})
  assert.equal(queryChart.points.length,120);assert.equal(queryChart.source.queryId,paged.id)
  const foreignChart=await page.evaluate(id=>clients[1].request('tableChart',{source:{kind:'query',queryId:id},x:0,y:1}).then(()=>false,()=>true),paged.id)
  assert.equal(foreignChart,true)

  assert.equal((await command('sqlPage', { id: paged.id, row: paged.rowEnd })).rowStart, paged.rowEnd)
  assert.equal((await command('sqlClose', { id: paged.id })).closed, true)
  assert.equal(await page.evaluate(id=>clients[0].request('tableChart',{source:{kind:'query',queryId:id},x:0,y:1}).then(()=>false,()=>true),paged.id),true)
  const stale = await page.evaluate(() => clients[0].request('outline', { revision: '0'.repeat(64) }).then(() => null, e => e.code))
  assert.equal(stale, 'REVISION')
  const denied = await page.evaluate(() => clients[1].request('setView', { view: 'edit' }).then(() => null, e => e.message))
  assert.match(denied, /not granted/)
  const first = page.frameLocator('iframe[title="Reader 0"]'), second = page.frameLocator('iframe[title="Reader 1"]')
  await command('navigate',{location:{kind:'cell',table:0,row:0,column:0}},1)
  const sourceTable=second.locator('article[data-table="0"]')
  await sourceTable.locator('.column-picker summary').click()
  await sourceTable.getByRole('checkbox').nth(1).uncheck()
  await sourceTable.getByRole('button',{name:'Apply',exact:true}).click()
  await sourceTable.locator('thead th').nth(1).waitFor({state:'detached'})
  assert.equal(await sourceTable.locator('thead th').count(),1)
  await command('setView',{view:'source'},1)
  await command('navigate',{location:{kind:'cell',table:0,row:119,column:1}},1)
  const navigatedCell=await command('selection',{},1)
  assert.equal(navigatedCell.quote,'119');assert.deepEqual(navigatedCell.location,{kind:'cell',table:0,row:119,column:1})
  assert.equal((await command('state',{},1)).view,'preview')
  assert.ok(navigatedCell.viewportRect.y>=0&&navigatedCell.viewportRect.y+navigatedCell.viewportRect.height<=850&&navigatedCell.viewportRect.x>=0&&navigatedCell.viewportRect.x+navigatedCell.viewportRect.width<=740,'Requested cell is outside the reader viewport')
  assert.equal(await sourceTable.locator('thead th').count(),2)
  await command('setView', { view: 'source' })
  const sourceCode = first.locator('#source-code-view .cm-content'); await sourceCode.click(); await sourceCode.press('Control+Home'); await sourceCode.press('Shift+End')
  const selected = await command('selection')
  assert.equal(selected.precision, 'exact'); assert.equal(selected.location.kind, 'source')
  assert.equal(Buffer.from(text).subarray(selected.location.startByte, selected.location.endByte).toString(), selected.quote)
  assert.ok(selected.quote.includes('Public 中文😀'))
  await command('setView', { view: 'edit' })
  const editor = first.locator('#editor-mount .cm-content'); await editor.click(); await editor.press('Control+Home'); await editor.press('End'); await editor.pressSequentially(' modified')
  await page.waitForFunction(() => eventsForTest.some(e => e.i === 0 && e.type === 'state' && e.payload.dirty))
  const dirtyLocale=await command('settings',{locale:'fr',messages:{Edit:'Modifier','Edit *':'Modifier *'},codeFontSize:18})
  assert.equal(dirtyLocale.dirty,true)
  assert.equal(await first.locator('#edit-mode').textContent(),'Modifier *')
  assert.equal(await first.locator('#editor-mount .cm-scroller').evaluate(node=>getComputedStyle(node).fontSize),'18px')
  assert.ok((await editor.textContent()).includes('modified'))
  assert.equal(await readFile(sources[0],'utf8'),text)
  await command('setView', { view: 'source' })
  assert.equal(await first.locator('#edit-notice').isVisible(), true)
  await command('setView', { view: 'edit' })
  assert.equal(await first.locator('#edit-notice').isVisible(), false)
  assert.ok((await editor.textContent()).includes('modified'))
  const saved = await command('save'); assert.equal(saved.dirty, false); assert.equal(saved.needsRefresh, true)
  await command('setView', { view: 'preview' })
  assert.ok((await readFile(sources[0], 'utf8')).includes('Public 中文😀 modified\r\n'))
  assert.equal(await readFile(sources[1], 'utf8'), text)
  const configured=await command('settings', { theme: 'dark', toolbar: 'host', fontSize: 16, contentFontSize:21,codeFontSize:18,locale:'fr',messages:{Preview:'Aperçu',Edit:'Modifier','Edit *':'Modifier *',Columns:'Colonnes'},palette:{brand:'#345678'} })
  assert.equal(configured.presentation.locale,'fr');assert.equal(configured.presentation.contentFontSize,21)
  assert.equal(configured.capabilities.presentation.version,1)
  assert.equal(await first.locator('#preview-mode').textContent(),'Aperçu')
  assert.equal(await first.locator('#edit-mode').textContent(),'Modifier')
  await command('navigate',{location:{kind:'line',line:1}})
  await first.locator('article.page h1').first().waitFor()
  assert.equal(await first.locator('article.page:has(h1)').first().evaluate(node=>getComputedStyle(node).fontSize),'21px')
  assert.equal(await first.locator('.reader-shell').evaluate(node=>getComputedStyle(node).getPropertyValue('--reader-brand').trim()),'#345678')
  assert.equal(await first.locator('#syntax-theme').getAttribute('href'),'./syntax-dark.css')
  assert.equal(await second.locator('#preview-mode').textContent(),'Preview')
  const beforeInvalid=JSON.stringify(configured.presentation)
  const invalid=await command('settings',{theme:'light',fontSize:99}).then(()=>false,()=>true)
  assert.equal(invalid,true);assert.equal(JSON.stringify((await command('state')).presentation),beforeInvalid)

  assert.equal(await first.locator('.reader-header').isVisible(), false)
  assert.equal(await first.locator('.reader-footer').isVisible(), false)
  assert.equal(await second.locator('.reader-header').isVisible(), true)
  assert.equal(await second.locator('.reader-footer').isVisible(), true)
  await command('setView', { view: 'edit' })
  assert.equal(await first.locator('#edit-save').isVisible(), false)
  assert.equal(await first.locator('#edit-reload').isVisible(), false)
  assert.equal(await first.locator('#edit-status').isVisible(), false)
  assert.equal(await first.locator('#edit-next').isVisible(), true)
  assert.equal(await first.locator('#edit-undo').isVisible(), true)
  await command('setView', { view: 'preview' })
  assert.equal(await first.getByRole('button', { name: 'Notes', exact: true }).count(), 0)
  await first.locator('body').press('Control+o')
  await page.waitForFunction(() => eventsForTest.some(e => e.i === 0 && e.type === 'openRequested'))
  const cancelled = await page.evaluate(async () => {
    const abort = new AbortController()
    const running = clients[1].request('sql', { table: 0, sql: 'WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT SUM(x) FROM n' }, { signal: abort.signal }).then(() => false, () => true)
    const deadline = Date.now() + 3000
    while (!eventsForTest.some(e => e.i === 1 && e.type === 'state' && e.payload.busy) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10))
    abort.abort(); return running
  })
  assert.equal(cancelled, true)
  assert.equal(await page.evaluate(() => eventsForTest.some(e => e.i === 1 && e.type === 'state' && e.payload.busy)), true)
  assert.equal((await command('sql', { table: 0, sql: 'SELECT COUNT(*) FROM data' }, 1)).rows[0][0], '120')
  await command('setView', { view: 'edit' })
  await editor.click(); await editor.press('Control+End'); await editor.pressSequentially(' unsaved')
  const external = '# External change\r\nKeep this file.\r\n'
  await writeFile(sources[0], external)
  const conflict = await page.evaluate(() => clients[0].request('save').then(() => null, error => error.code))
  assert.equal(conflict, 'REVISION'); assert.equal(await readFile(sources[0], 'utf8'), external)
  const dirtyClose = await page.evaluate(() => clients[0].dispose().then(() => null, error => error.code))
  assert.equal(dirtyClose, 'DIRTY')
  assert.equal((await command('state')).dirty, true)
  assert.equal((await readFile(sources[0], 'utf8')).includes(' unsaved'), false)
  await page.evaluate(() => Promise.all([clients[0].dispose({ force: true }), clients[1].dispose(), clients[2].dispose()]))
  assert.equal(await page.evaluate(() => eventsForTest.filter(e => e.type === 'disposed').length), 3)
  assert.equal(await first.locator('.page').count(), 0)
  assert.equal(await second.locator('.page').count(), 0)
  assert.equal(await page.evaluate(() => clients.reduce((sum, client) => sum + client.active, 0)), 0)
  // Closing while indexing must stop polling, even if the frame is retained.
  const earlyUrl = new URL(urls[0]), path = `${earlyUrl.origin}/api/document`
  let polls = 0
  await page.route(path, async route => {
    polls++
    await route.fulfill({ contentType: 'application/json', body: JSON.stringify({ state: 'indexing', documentId: 'synthetic-indexing', editable: false, sourceAvailable: true }) })
  })
  await page.evaluate(async url => {
    const { createReaderClient } = await import('/host-client.mjs')
    const frame = document.createElement('iframe'); frame.src = url; document.body.append(frame)
    window.earlyClient = createReaderClient(frame)
    const hello = await earlyClient.request('hello')
    if (hello.state !== 'indexing') throw Error('Expected the synthetic indexing phase')
    await earlyClient.dispose()
    const immediate = document.createElement('iframe'); immediate.src = url; document.body.append(immediate)
    await createReaderClient(immediate).dispose()
  }, urls[0])
  const stoppedPolls = polls; await pause(500); assert.equal(polls, stoppedPolls)
  await page.unroute(path)
  const detachedCleanup = await page.evaluate(async url => {
    const { createReaderClient } = await import('/host-client.mjs')
    const client = createReaderClient({ src: url, contentWindow: { postMessage() {} } }, { timeout: 100 })
    const failure = await client.dispose({ force: true }).then(() => null, error => error.code)
    const after = await client.request('state').then(() => null, error => error.message)
    return { failure, after, pending: client.active }
  }, urls[0])
  assert.equal(detachedCleanup.failure, 'TIMEOUT')
  assert.match(detachedCleanup.after, /disposed/)
  assert.equal(detachedCleanup.pending, 0)
  assert.deepEqual(errors, [])
  await browser.close(); browser = null
  for (const child of children) { const exit = once(child, 'exit'); child.stdin.end('\n'); await exit; assert.equal(child.exitCode, 0) }
  for (const cache of caches) assert.deepEqual(await readdir(cache), [])
  console.log(JSON.stringify({ ok: true, publicProtocol: true, multiPane: true, exactSourceSelection: true, cancellation: true, cleanup: true, artifacts: directory }))
} finally {
  await browser?.close()
  for (const child of children) if (child.exitCode == null) { const exit = once(child, 'exit'); child.kill('SIGKILL'); await exit }
  await new Promise(resolve => parent.close(resolve))
}
