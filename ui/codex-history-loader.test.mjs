import test from 'node:test'
import assert from 'node:assert/strict'
import { createCodexHistoryLoader } from './codex-history-loader.mjs'
const turn = id => ({ id, status: 'completed', items: [] })
const cached = { model: { threadId: 'one', turns: [turn('old'), turn('anchor')] } }
function fixture(overrides = {}) {
  const calls = []
  const services = {
    historyTailCapability: () => undefined,
    rememberHistoryTailCapability: (...args) => calls.push(['capability', ...args]),
    threadForRef: () => ({ name: 'catalog' }),
    requestCodexResume: async (...args) => { calls.push(['resume', ...args]); return { thread: { id: 'one' }, initialTurnsPage: { data: [turn('new'), turn('anchor')] } } },
    dispatchBackendRpc: async (...args) => { calls.push(['rpc', ...args]); return { thread: { id: 'one', turns: [turn('full')] } } },
    ...overrides,
  }
  return { ...createCodexHistoryLoader(services), calls }
}
test('selection uses initial tail without an extra list/read and forwards environment settings', async () => {
  const loader = fixture()
  const result = await loader.loadCodexHistoryForSelection('ept-codex', 'one', cached, { environmentRoot: '/work', environmentRevision: 'rev' })
  assert.equal(result.historyMode, 'tail')
  assert.deepEqual(result.result.thread.turns.map(t => t.id), ['old', 'anchor', 'new'])
  assert.deepEqual(loader.calls[0], ['resume', 'ept-codex', 'one', { environmentRoot: '/work', environmentRevision: 'rev', incremental: true }])
  assert.equal(loader.calls.filter(c => c[0] === 'rpc').length, 0)
})
test('cold and unsupported selections resume once; legacy full responses are not read again', async () => {
  for (const state of [null, cached]) {
    const loader = fixture({ historyTailCapability: () => false })
    const result = await loader.loadCodexHistoryForSelection('codex', 'one', state)
    assert.equal(result.historyMode, 'full')
    assert.equal(loader.calls.length, 1)
  }
  const loader = fixture({ requestCodexResume: async () => ({ thread: { turns: [turn('legacy')] } }) })
  assert.equal((await loader.loadCodexHistoryForSelection('codex', 'one', cached)).historyMode, 'legacy-full')
  assert.deepEqual(loader.calls, [['capability', 'codex', 'one', false]])
})
test('an unmatched tail replaces history from pages without an unbounded read', async () => {
  const loader = fixture({ requestCodexResume: async () => ({ thread: { name: 'resume metadata' }, initialTurnsPage: { data: [turn('unrelated')] } }) })
  const result = await loader.loadCodexHistoryForSelection('codex', 'one', cached)
  assert.equal(result.historyMode, 'paged-full')
  assert.equal(result.result.thread.name, 'resume metadata')
  assert.deepEqual(result.result.thread.turns.map(t => t.id), ['unrelated'])
  assert.equal(loader.calls.filter(c => c[0] === 'rpc').length, 0)
})
test('background tail loading never resumes and propagates non-compatibility errors', async () => {
  const loader = fixture({ dispatchBackendRpc: async (_backend, method) => {
    assert.equal(method, 'thread/turns/list')
    return { data: [turn('new'), turn('anchor')] }
  } })
  const result = await loader.loadCodexHistoryForBackground('codex', 'one', cached)
  assert.equal(result.historyMode, 'tail')
  assert.equal(result.result.thread.name, 'catalog')
  assert.equal(loader.calls.some(c => c[0] === 'resume'), false)
  const error = new Error('connection lost')
  const broken = fixture({ dispatchBackendRpc: async () => { throw error } })
  await assert.rejects(broken.loadCodexHistoryForBackground('codex', 'one', cached), value => value === error)
})

test('unsupported pagination falls back once, without masking unrelated resume failures', async () => {
  let resumes = 0
  const loader = fixture({ requestCodexResume: async () => {
    resumes++
    if (resumes === 1) throw new Error('Invalid params: unknown field excludeTurns')
    return { thread: { id: 'one', turns: [turn('full')] } }
  } })
  const result = await loader.loadCodexHistoryForSelection('codex', 'one', cached)
  assert.equal(result.historyMode, 'compatibility-fallback')
  assert.equal(resumes, 2)
  const error = new Error('thread-store conflict: active writer')
  const failed = fixture({ requestCodexResume: async () => { throw error } })
  await assert.rejects(failed.loadCodexHistoryForSelection('codex', 'one', cached), value => value === error)
  assert.deepEqual(failed.calls, [])
})

test('cold selection restores more than 64 MiB and 100 pages through bounded requests', async () => {
  const text = 'x'.repeat(176_000), count = 402, calls = [], bytes = []
  const page = end => {
    const data = Array.from({length:Math.min(4,end)},(_,i)=>({ ...turn(String(end-i)), items:[{type:'agentMessage',id:`item-${end-i}`,text}] }))
    const result = {data,nextCursor:end>4?String(end-4):null}
    bytes.push(Buffer.byteLength(JSON.stringify(result)))
    return result
  }
  const loader = fixture({
    requestCodexResume: async (_backend,id,options) => {
      calls.push('resume');assert.equal(options.incremental,true)
      assert.equal(options.environmentRoot,'/work');assert.equal(options.environmentRevision,'revision')
      return {thread:{id,name:'Large fixture'},initialTurnsPage:page(count)}
    },
    dispatchBackendRpc: async (_backend,method,params) => {
      calls.push(method);assert.equal(method,'thread/turns/list');assert.equal(params.limit,4)
      assert.equal(params.sortDirection,'desc');assert.equal(params.itemsView,'full')
      return page(Number(params.cursor))
    },
  })
  const result=await loader.loadCodexHistoryForSelection('codex','one',null,{environmentRoot:'/work',environmentRevision:'revision'})
  assert.equal(result.historyMode,'paged-full');assert.equal(result.tail.pageCount,101)
  assert.equal(result.result.thread.name,'Large fixture')
  assert.deepEqual(result.result.thread.turns.map(t=>t.id),Array.from({length:count},(_,i)=>String(i+1)))
  assert.ok(bytes.reduce((a,b)=>a+b,0)>64*1024*1024)
  assert.ok(Math.max(...bytes)<1024*1024)
  assert.equal(calls.filter(c=>c==='resume').length,1)
})

test('cold background reads metadata and pages without resuming or requesting complete history', async () => {
  const calls=[]
  const loader=fixture({dispatchBackendRpc:async (_backend,method,params)=>{
    calls.push([method,params])
    if(method==='thread/read'){assert.equal(params.includeTurns,false);return {thread:{id:'one',name:'metadata'}}}
    assert.equal(method,'thread/turns/list')
    return params.cursor ? {data:[turn('old')]} : {data:[turn('new')],nextCursor:'older'}
  }})
  const result=await loader.loadCodexHistoryForBackground('codex','one',null)
  assert.equal(result.historyMode,'paged-full');assert.equal(result.result.thread.name,'metadata')
  assert.deepEqual(result.result.thread.turns.map(t=>t.id),['old','new'])
  assert.equal(loader.calls.some(c=>c[0]==='resume'),false)
  assert.equal(calls.length,3)
})

test('broken pagination and transport errors never retry as unbounded history', async () => {
  for(const response of [()=>({data:[turn('old')],nextCursor:'repeat'}),()=>{throw Error('connection lost')}]){
    const calls=[]
    const loader=fixture({
      requestCodexResume:async()=>({thread:{id:'one'},initialTurnsPage:{data:[turn('new')],nextCursor:'repeat'}}),
      dispatchBackendRpc:async(_backend,method)=>{calls.push(method);return response()},
    })
    await assert.rejects(loader.loadCodexHistoryForSelection('codex','one',null),/repeated a cursor|connection lost/)
    assert.deepEqual(calls,['thread/turns/list'])
    assert.equal(loader.calls.some(c=>c[0]==='capability'),false)
  }
})

test('an unmatched background cache refreshes authoritative metadata without fetching full turns', async () => {
  const methods=[]
  const loader=fixture({dispatchBackendRpc:async(_backend,method,params)=>{
    methods.push(method)
    if(method==='thread/read'){assert.equal(params.includeTurns,false);return {thread:{id:'one',name:'updated',status:'idle'}}}
    assert.equal(method,'thread/turns/list');return {data:[turn('replacement')]}
  }})
  const result=await loader.loadCodexHistoryForBackground('codex','one',cached)
  assert.deepEqual(methods,['thread/turns/list','thread/read'])
  assert.equal(result.result.thread.name,'updated')
  assert.deepEqual(result.result.thread.turns.map(t=>t.id),['replacement'])
  assert.equal(result.historyMode,'paged-full')
})
