// Real OpenCode session + Studio's protocol adapter, with private data and a local mock model.
// STUDIO_OPENCODE_BIN=/path/to/opencode node scripts/smoke-opencode-session.mjs
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
import { setTimeout as delay } from 'node:timers/promises'
import { createOpenCodeProtocol } from '../ui/opencode-protocol.mjs'

const temporary = await mkdtemp(join(tmpdir(), 'studio-opencode-smoke-'))
const provider = createServer(async (request, response) => {
  for await (const _ of request) { /* drain request */ }
  if (!request.url.endsWith('/chat/completions')) { response.writeHead(404); response.end(); return }
  provider.requests += 1
  const chunks = [
    { delta: { role: 'assistant', content: 'local OpenCode smoke reply' }, finish_reason: null },
    { delta: {}, finish_reason: 'stop' },
  ].map((choice, index) => ({ id: 'chatcmpl-smoke', object: 'chat.completion.chunk', created: 1, model: 'smoke', choices: [{ index: 0, ...choice }], ...(index ? { usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } } : {}) }))
  response.writeHead(200, { 'Content-Type': 'text/event-stream' })
  response.end(chunks.map(chunk => `data: ${JSON.stringify(chunk)}\n\n`).join('') + 'data: [DONE]\n\n')
})
provider.requests = 0
await new Promise(resolve => provider.listen(0, '127.0.0.1', resolve))
let child, events, readerTask
const received = [], eventAbort = new AbortController()
try {
  for (const directory of ['config', 'data', 'cache', 'state', 'work']) await mkdir(join(temporary, directory))
  const work = join(temporary, 'work')
  const config = { model: 'smoke/smoke', small_model: 'smoke/smoke', share: 'disabled', autoupdate: false,
    enabled_providers: ['smoke'], permission: { '*': 'deny' }, provider: { smoke: {
      npm: '@ai-sdk/openai-compatible', name: 'Local smoke', options: { baseURL: `http://127.0.0.1:${provider.address().port}/v1`, apiKey: 'local-fixture' },
      models: { smoke: { name: 'smoke', limit: { context: 4096, output: 256 } } },
    } } }
  child = spawn(process.env.STUDIO_OPENCODE_BIN || 'opencode', ['serve', '--hostname', '127.0.0.1', '--port', '0'], {
    cwd: work, env: { ...process.env, XDG_CONFIG_HOME: join(temporary, 'config'), XDG_DATA_HOME: join(temporary, 'data'), XDG_CACHE_HOME: join(temporary, 'cache'), XDG_STATE_HOME: join(temporary, 'state'),
      OPENCODE_CONFIG_CONTENT: JSON.stringify(config), OPENCODE_DISABLE_MODELS_FETCH: 'true', OPENCODE_DISABLE_DEFAULT_PLUGINS: 'true', OPENCODE_DISABLE_CLAUDE_CODE: 'true', OPENCODE_SERVER_PASSWORD: 'private-smoke',
    }, stdio: ['ignore', 'pipe', 'pipe'] })
  let logs = ''
  child.stderr.on('data', chunk => { logs = (logs + chunk).slice(-10000) })
  const address = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(Error('OpenCode startup timed out: ' + logs)), 30000)
    createInterface({ input: child.stdout }).on('line', line => { const match = line.match(/http:\/\/127\.0\.0\.1:\d+/); if (match) { clearTimeout(timeout); resolve(match[0]) } })
    child.once('error', error => { clearTimeout(timeout); reject(error) })
    child.once('exit', code => { clearTimeout(timeout); reject(Error(`OpenCode exited ${code}: ${logs}`)) })
  })
  const auth = { Authorization: `Basic ${Buffer.from('opencode:private-smoke').toString('base64')}` }
  const fetchApi = (path, options = {}) => fetch(address + path, { ...options, headers: { ...options.headers, ...auth } })
  assert.equal((await fetchApi('/global/health')).status, 200)
  assert.equal((await fetch(address + '/global/health')).status, 401)
  events = await fetchApi('/global/event', { signal: eventAbort.signal })
  assert.equal(events.status, 200)
  readerTask = (async () => {
    let pending = ''
    for await (const chunk of events.body) {
      pending += new TextDecoder().decode(chunk)
      const blocks = pending.split('\n\n'); pending = blocks.pop()
      for (const block of blocks) for (const line of block.split('\n')) if (line.startsWith('data: ')) received.push(JSON.parse(line.slice(6)))
    }
  })().catch(error => { if (!eventAbort.signal.aborted) throw error })
  const state = { backend: 'opencode', ready: true, threads: [], threadsByBackend: { opencode: [] }, hiddenSessionDirectories: [], sessionDirectoryIgnore: [] }
  const protocol = createOpenCodeProtocol(state, {
    gatewayFetch: (path, options) => fetchApi(path.replace(/^\/opencode/, ''), options),
    selectedThread: () => state.threads[0], currentTurnOptions: () => ({}), t: text => text,
    studioPerformance: { start: () => () => {} }, threadCatalogKey: (backend, id) => `${backend}:${id}`, openCodeHistoryEventSequences: new Map(),
  })
  const { thread } = await protocol.openCodeRpc('thread/start', { cwd: work, name: 'Ecosystem smoke', model: 'smoke/smoke' })
  state.threads.push(thread); state.threadsByBackend.opencode = state.threads; state.selectedId = thread.id
  await protocol.openCodeRpc('thread/name/set', { threadId: thread.id, name: 'Reviewed smoke' })
  const { turn } = await protocol.openCodeRpc('turn/start', { threadId: thread.id, model: 'smoke/smoke', input: [{ type: 'text', text: 'Reply once using the local fixture. Do not call tools.' }] })
  assert.match(turn.id, /^msg/)
  let history
  for (let attempt = 0; attempt < 150; attempt += 1) {
    history = await protocol.fetchOpenCodeMessageHistory(thread.id, work, {})
    const assistant = history.messages.find(message => message.info.role === 'assistant' && message.info.time?.completed)
    if (assistant) { assert.ok(assistant.parts.some(part => part.type === 'text' && part.text.includes('local OpenCode smoke reply'))); break }
    assert.ok(!received.some(event => event.payload?.type === 'session.error'), JSON.stringify(received.filter(event => event.payload?.type === 'session.error')))
    if (attempt === 149) throw Error('OpenCode did not complete its local model turn: ' + logs)
    await delay(100)
  }
  assert.ok(provider.requests > 0)
  assert.ok(received.some(event => event.payload?.type === 'message.part.updated'))
  const resumed = await protocol.openCodeRpc('thread/resume', { threadId: thread.id })
  assert.equal(resumed.thread.id, thread.id)
  assert.equal(resumed.historyComplete, true)
  const fork = await protocol.openCodeRpc('thread/fork', { threadId: thread.id })
  assert.notEqual(fork.thread.id, thread.id)
  await protocol.openCodeRpc('thread/delete', { threadId: fork.thread.id })
  await protocol.openCodeRpc('thread/delete', { threadId: thread.id })
  console.log('OpenCode: authenticated server, Studio adapter create/rename/send, live SSE, local-model completion, history/resume/fork/delete passed')
} finally {
  eventAbort.abort(); await readerTask
  if (child?.exitCode === null) { child.kill(); await new Promise(resolve => child.once('exit', resolve)) }
  provider.closeAllConnections(); await new Promise(resolve => provider.close(resolve))
  await rm(temporary, { recursive: true, force: true })
}
