import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createDocumentWorkspaceController, createDocumentWorkspaceState } from './document-workspace-controller.mjs'

function documentWorkspaceFixture(context, gatewayFetch, overrides = {}) {
  const elements = new Map()
  const element = (selector) => {
    if (!elements.has(selector)) {
      const classes = new Set()
      elements.set(selector, {
        dataset: {},
        classList: {
          add: (...values) => values.forEach((value) => classes.add(value)),
          remove: (...values) => values.forEach((value) => classes.delete(value)),
          contains: (value) => classes.has(value),
          toggle(value, force = !classes.has(value)) {
            if (force) classes.add(value)
            else classes.delete(value)
            return force
          },
        },
        setAttribute() {},
        replaceChildren() {},
        querySelectorAll: () => [],
      })
    }
    return elements.get(selector)
  }
  const previousStyle = Object.getOwnPropertyDescriptor(globalThis, 'getComputedStyle')
  Object.defineProperty(globalThis, 'getComputedStyle', { configurable: true, value: () => ({ getPropertyValue: () => '' }) })
  const previousDocument = Object.getOwnPropertyDescriptor(globalThis, 'document')
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { querySelector: element, documentElement: { dataset: {} } } })
  let cleanup = () => {}
  context.after(() => {
    cleanup()
    if (previousStyle) Object.defineProperty(globalThis, 'getComputedStyle', previousStyle)
    else delete globalThis.getComputedStyle
    if (previousDocument) Object.defineProperty(globalThis, 'document', previousDocument)
    else delete globalThis.document
  })
  const state = createDocumentWorkspaceState()
  let threadKey = 'thread-1'
  let requestId = 0
  let markdownRenders = 0
  const errors = []
  const controller = createDocumentWorkspaceController({
    state,
    loadReaderMessages: () => import('../crates/marknoria/web/ui-messages.mjs'),
    gatewayFetch,
    randomId: () => `request-${++requestId}`,
    reportError: error => errors.push(error),
    view: {
      selectedThread: () => ({ cwd: '/project' }),
      selectedStateKey: () => threadKey,
      activateRightWorkspace: (tool) => { state.activeRightWorkspace = tool },
      hideComposerMenu() {},
      closeActionMenus() {},
      disposeMarkdownImageAssets() {},
      disconnectMarkdownImageObserver() {},
      applyRightRailWidth() {},
      hideSelection() {},
      renderMarkdownDocument() { markdownRenders += 1; throw new Error('Unexpected Markdown rendering') },
      hydrateMarkdownImages() {},
      ...overrides,
    },
  })
  cleanup = () => controller.close({ restoreMap: false })
  return { state, controller, element, errors, markdownRenders: () => markdownRenders, selectThread: (key) => { threadKey = key } }
}

function assertDocumentError(fixture, message, markdownRenders = 0) {
  assert.equal(fixture.state.artifact.loading, false)
  assert.equal(fixture.state.artifact.error, message)
  assert.equal(fixture.element('#artifact-loading').classList.contains('hidden'), true)
  assert.equal(fixture.element('#artifact-error').classList.contains('hidden'), false)
  assert.equal(fixture.element('#artifact-error-message').textContent, message)
  assert.equal(fixture.markdownRenders(), markdownRenders)
}

test('large Markdown uses one retained frame without whole-document rendering and closes its lease', async (context) => {
  const closed = []
  const result = { root: '/project', path: 'large.md', size: 32 * 1024 * 1024, readOnly: true, reader: { id: 'reader-1', url: '/markdown-reader/reader-1/?embedded=1#token' } }
  const fixture = documentWorkspaceFixture(context, async (url, options) => {
    if (url.endsWith('/close')) { closed.push(JSON.parse(options.body).id); return new Response('{}') }
    return new Response(JSON.stringify(result))
  })
  const frames = []
  document.createElement = tag => {
    if (tag === 'canvas') return { getContext: () => null }
    const frame = { setAttribute() {}, addEventListener() {}, remove() { this.removed = true } }
    frames.push(frame)
    return frame
  }
  await fixture.controller.open({ path: 'large.md' })
  assert.equal(fixture.state.artifact.kind, 'large-markdown')
  assert.equal(fixture.state.artifact.content, undefined)
  assert.equal(fixture.markdownRenders(), 0)
  assert.equal(fixture.element('#artifact-view-switch').classList.contains('hidden'), false)
  assert.equal(fixture.element('#artifact-edit').classList.contains('hidden'), true)
  assert.equal(frames.length, 1)
  assert.equal(frames[0].src, result.reader.url)
  fixture.controller.render()
  fixture.controller.setView('edit')
  assert.equal(frames.length, 1)
  fixture.controller.close({ restoreMap: false })
  assert.deepEqual(closed, ['reader-1'])
  assert.equal(frames[0].removed, true)
})

test('late large Markdown metadata releases its service after the document closes', async (context) => {
  let resolveResponse
  const closed = []
  const fixture = documentWorkspaceFixture(context, async (url, options) => {
    if (url.endsWith('/close')) { closed.push(JSON.parse(options.body).id); return new Response('{}') }
    return new Promise(resolve => { resolveResponse = resolve })
  })
  const pending = fixture.controller.open({ path: 'large.md' })
  fixture.controller.close({ restoreMap: false })
  resolveResponse(new Response(JSON.stringify({ largeMarkdown: { id: 'stale-reader' } })))
  await pending
  assert.equal(fixture.state.artifact, null)
  assert.deepEqual(closed, ['stale-reader'])
})

test('document network failures clear Loading and display the error', async (context) => {
  const failure = new Error('Network connection closed')
  const fixture = documentWorkspaceFixture(context, async () => { throw failure })

  await assert.rejects(fixture.controller.open({ path: 'large.md' }), failure)
  assertDocumentError(fixture, failure.message)
})

test('document response decoding failures clear Loading and display the error', async (context) => {
  const failure = new SyntaxError('Invalid JSON response')
  const fixture = documentWorkspaceFixture(context, async () => ({ ok: true, json: async () => { throw failure } }))

  await assert.rejects(fixture.controller.open({ path: 'large.md' }), failure)
  assertDocumentError(fixture, failure.message)
})

test('invalid common reader metadata shows an explicit error', async context => {
  const fixture = documentWorkspaceFixture(context, async () => new Response(JSON.stringify({content:'legacy content'})))
  await assert.rejects(fixture.controller.open({path:'large.pdf'}), /Invalid reader response/)
  assertDocumentError(fixture, 'Invalid reader response')
})

test('oversized documents display the server rejection without Markdown rendering', async (context) => {
  const message = 'file exceeds the 5 MiB review limit'
  const fixture = documentWorkspaceFixture(context, async () => new Response(JSON.stringify({ error: { message } }), { status: 413 }))

  await assert.rejects(fixture.controller.open({ path: 'large.md' }), { message })
  assertDocumentError(fixture, message)
})

test('late document failures cannot overwrite a newer request', async (context) => {
  let rejectFirst
  const pending = new Promise((resolve, reject) => { rejectFirst = reject })
  let requests = 0
  const fixture = documentWorkspaceFixture(context, () => ++requests === 1 ? pending : Promise.reject(new Error('Second request failed')))
  const first = fixture.controller.open({ path: 'first.md' })
  await assert.rejects(fixture.controller.open({ path: 'second.md' }), { message: 'Second request failed' })
  const current = fixture.state.artifact
  rejectFirst(new Error('Stale failure'))
  await first

  assert.equal(fixture.state.artifact, current)
  assertDocumentError(fixture, 'Second request failed')
})

test('late document failures do not reopen closed documents', async (context) => {
  let rejectRequest
  const fixture = documentWorkspaceFixture(context, () => new Promise((resolve, reject) => { rejectRequest = reject }))
  const pending = fixture.controller.open({ path: 'large.md' })
  fixture.controller.close({ restoreMap: false })
  rejectRequest(new Error('Stale failure'))
  await pending

  assert.equal(fixture.state.artifact, null)
  assert.equal(fixture.element('#artifact-rail').classList.contains('hidden'), true)
})

test('late document failures cannot change another session', async (context) => {
  let rejectRequest
  const fixture = documentWorkspaceFixture(context, () => new Promise((resolve, reject) => { rejectRequest = reject }))
  const pending = fixture.controller.open({ path: 'large.md' })
  const current = fixture.state.artifact
  fixture.selectThread('thread-2')
  rejectRequest(new Error('Stale failure'))
  await pending

  assert.equal(fixture.state.artifact, current)
  assert.equal(current.error, undefined)
})

test('Document Workspace state factories isolate search and outline state', () => {
  const first = createDocumentWorkspaceState()
  const second = createDocumentWorkspaceState()

  first.artifactSearchMatches.push({ id: 'match-1' })
  first.artifactOutlineCollapsed.add('section-1')

  assert.deepEqual(second.artifactSearchMatches, [])
  assert.deepEqual([...second.artifactOutlineCollapsed], [])
  assert.equal(second.artifactView, 'preview')
  assert.equal(second.artifact, null)
})

test('late document rendering cannot reopen an inactive right workspace', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const start = source.indexOf('function renderArtifact()')
  const end = source.indexOf('\nasync function loadEpubReadingState', start)
  const render = source.slice(start, end)
  const inactiveGuard = render.indexOf("if (state.activeRightWorkspace !== 'document')")
  const showRail = render.indexOf("rail.classList.remove('hidden')")

  assert.ok(inactiveGuard >= 0)
  assert.ok(inactiveGuard < showRail)
  assert.match(render.slice(inactiveGuard, showRail), /rail\.classList\.add\('hidden'\)[\s\S]*return/u)
})

test('Document outline is transient and remains open after chapter navigation', () => {
  const source = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  const navigate = source.slice(source.indexOf('async function navigateArtifactOutlineItem('), source.indexOf('function setArtifactOutlineActive('))

  assert.doesNotMatch(source, /ResizeObserver/u)
  assert.doesNotMatch(source, /classList\.contains\('compact'\)/u)
  assert.doesNotMatch(source, /resize: updateArtifactOutlineLayout/u)
  assert.doesNotMatch(navigate, /setArtifactOutlineOpen/u)
  assert.match(source, /function resetArtifactOutline\(\)[\s\S]*state\.artifactOutlineOpen = false/u)
})

for (const [path, sourceAvailable, readOnly] of [['data.csv',true,false],['data.tsv',true,false],['data.xlsx',false,true],
  ...['small.md','plain.txt','script.py','main.c','lib.cpp','lib.rs','data.json','data.jsonl','config.yaml','config.toml','data.xml','page.html'].map(path=>[path,true,false]),
  ['photo.jpg',false,true],['book.pdf',false,true]]) {
  test(`${path} mounts the common reader in Preview with format-aware source/edit controls`, async context => {
    const requests=[]
    const result={root:'/project',path,size:2048,readOnly,sourceAvailable,format:path.split('.').at(-1),reader:{id:'table-1',url:'/markdown-reader/table-1/?embedded=1#token'}}
    const fixture=documentWorkspaceFixture(context,async(url,options)=>{
      requests.push(url)
      return new Response(JSON.stringify(url.endsWith('/close')?{}:result))
    })
    document.createElement=tag=>tag==='canvas'?{getContext:()=>null}:{setAttribute(){},addEventListener(){},remove(){}}
    await fixture.controller.open({path})
    assert.equal(requests[0],'/studio/reader/open')
    assert.equal(fixture.state.artifact.kind,'large-markdown')
    assert.equal(fixture.state.artifact.largeMarkdown.id,'table-1')
    assert.equal(fixture.state.artifactView,'preview')
    assert.equal(fixture.element('#artifact-source').classList.contains('hidden'),!sourceAvailable)
    assert.equal(fixture.element('#artifact-edit').classList.contains('hidden'),readOnly)
    assert.equal(fixture.markdownRenders(),0)
    fixture.controller.close({restoreMap:false})
    assert.equal(requests.at(-1),'/studio/markdown/close')
  })
}
test('shared table authorization failures never fall back to another file endpoint',async context=>{
  const requests=[]
  const fixture=documentWorkspaceFixture(context,async url=>{requests.push(url);return new Response(JSON.stringify({error:{message:'File not granted'}}),{status:403})})
  await assert.rejects(fixture.controller.open({path:'data.xlsx'}),/File not granted/)
  assert.deepEqual(requests,['/studio/reader/open'])
  assertDocumentError(fixture,'File not granted')
})
for (const path of ['data.csv','data.xlsx','small.md','photo.jpg','book.pdf','book.epub']) {
 test(`${path} preserves the explicit unsupported error without a renderer fallback`, async context => {
  const requests=[]
  const fixture=documentWorkspaceFixture(context,async url=>{requests.push(url);return new Response(JSON.stringify({error:{message:'Unsupported WSL format'}}),{status:501})})
  await assert.rejects(fixture.controller.open({path}),/Unsupported WSL format/)
  assert.deepEqual(requests,['/studio/reader/open'])
 })
}

test('EPUB opens the shared session and retains its existing content-keyed reading state', async context => {
 const requests=[],saved={cfi:'epubcfi(/6/4!/4/2/1:0)',theme:'sepia',flow:'scrolled',fontScale:1.25}
 const result={root:'/project',path:'book.epub',size:2048,readOnly:true,sourceAvailable:false,format:'epub',bookHash:'fnv1a64:0123456789abcdef',reader:{id:'epub-1',url:'/markdown-reader/epub-1/?embedded=1#token'}}
 const fixture=documentWorkspaceFixture(context,async(url,options)=>{
  requests.push({url,body:JSON.parse(options.body)})
  return new Response(JSON.stringify(url==='/studio/reader/open'?result:url==='/studio/epub/state'?saved:{}))
 },{reportClientError:()=>{}})
 document.createElement=tag=>tag==='canvas'?{getContext:()=>null}:{setAttribute(){},addEventListener(){},remove(){}}
 await fixture.controller.open({path:'book.epub'})
 assert.deepEqual(requests.map(value=>value.url),['/studio/reader/open','/studio/epub/state'])
 assert.equal(requests[1].body.bookHash,result.bookHash)
 assert.equal(fixture.state.artifact.kind,'large-markdown')
 assert.deepEqual(fixture.state.artifact.readingState,saved)
 assert.equal(fixture.state.artifact.bytes,undefined)
 assert.equal(fixture.element('#artifact-source').classList.contains('hidden'),true)
 assert.equal(fixture.element('#artifact-edit').classList.contains('hidden'),true)
 fixture.controller.close({restoreMap:false})
 assert.deepEqual(requests.slice(-2).map(value=>value.url),['/studio/epub/state','/studio/markdown/close'])
})
for(const mode of ['failure','closed','changed'])test(`EPUB ${mode} during reading-state load releases the new session`,async context=>{
 let resolveState
 const closed=[],result={root:'/project',path:'book.epub',format:'epub',bookHash:'fnv1a64:0123456789abcdef',reader:{id:'new-epub',url:'/markdown-reader/new-epub/#token'}}
 const fixture=documentWorkspaceFixture(context,async(url,options)=>{
  if(url.endsWith('/close')){closed.push(JSON.parse(options.body).id);return new Response('{}')}
  if(url==='/studio/epub/state')return mode==='failure'?new Response(JSON.stringify({error:{message:'State storage failed'}}),{status:500}):new Promise(resolve=>{resolveState=resolve})
  return new Response(JSON.stringify(result))
 })
 const pending=fixture.controller.open({path:'book.epub',...(mode==='changed'?{epubBookHash:'fnv1a64:ffffffffffffffff'}:{})})
 if(mode==='closed'){
  for(let i=0;i<40&&!resolveState;i++)await Promise.resolve()
  assert.equal(typeof resolveState,'function')
  fixture.controller.close({restoreMap:false})
  resolveState(new Response('{}'))
  await pending
  assert.equal(fixture.state.artifact,null)
 }else await assert.rejects(pending,mode==='failure'?/State storage failed/:/book has changed/)
 assert.deepEqual(closed,['new-epub'])
})

for (const status of [400,401,403,500]) for (const path of ['small.md','photo.jpg','book.pdf']) {
 test(`${path} HTTP ${status} never falls back to a legacy read`,async context=>{
  const requests=[]
  const fixture=documentWorkspaceFixture(context,async url=>{requests.push(url);return new Response(JSON.stringify({error:{message:'Denied'}}),{status})})
  await assert.rejects(fixture.controller.open({path}),/Denied/)
  assert.deepEqual(requests,['/studio/reader/open'])
 })
}
for (const view of ['source','edit']) test(`common entry restores explicit ${view} instead of fresh Preview`,async context=>{
 const result={root:'/project',path:'script.py',format:'text',sourceAvailable:true,readOnly:false,reader:{id:'source-1',url:'/markdown-reader/source-1/#token'}}
 const fixture=documentWorkspaceFixture(context,async()=>new Response(JSON.stringify(result)))
 document.createElement=tag=>tag==='canvas'?{getContext:()=>null}:{setAttribute(){},addEventListener(){},remove(){}}
 const previous=globalThis.requestAnimationFrame
 globalThis.requestAnimationFrame=()=>0
 context.after(()=>{if(previous)globalThis.requestAnimationFrame=previous;else delete globalThis.requestAnimationFrame})
 await fixture.controller.open({path:'script.py'},{viewState:{view}})
 assert.equal(fixture.state.artifactView,view)
 assert.equal(fixture.state.artifact.readerSourceAvailable,true)
 assert.equal(fixture.markdownRenders(),0)
})

test('WSL document link candidates preserve Unicode and fragments without changing the project grant',async()=>{
 const {resolveWslDocumentLink}=await import('./document-workspace-controller.mjs')
 const file={root:'/project',documentRoot:'/project',path:'/project/docs/notes.md'}
 assert.deepEqual(resolveWslDocumentLink(file,'../images/%E4%B8%AD%E6%96%87%20a.png#detail'),{path:'/project/images/中文 a.png',fragment:'#detail'})
 assert.deepEqual(resolveWslDocumentLink(file,'/images/a.png'),{path:'/project/images/a.png',fragment:''})
 assert.deepEqual(resolveWslDocumentLink(file,'file:///shared/a.csv'),{path:'/shared/a.csv',fragment:''})
 assert.throws(()=>resolveWslDocumentLink(file,'//untrusted/a.png'),/authorized local file/)
 assert.throws(()=>resolveWslDocumentLink(file,'a%00.png'),/Invalid/)
 assert.deepEqual(resolveWslDocumentLink({...file,path:'/project/literal%20dir/notes.md'},'next.md'),{path:'/project/literal%20dir/next.md',fragment:''})
 assert.equal(file.root,'/project')
})
