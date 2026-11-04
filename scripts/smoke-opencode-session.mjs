// Real OpenCode session + Studio's protocol adapter, with private data and a local mock model.
// STUDIO_OPENCODE_BIN=/path/to/opencode node scripts/smoke-opencode-session.mjs
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { appendFile, mkdtemp, mkdir, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
import { setTimeout as delay } from 'node:timers/promises'
import { createOpenCodeProtocol } from '../ui/opencode-protocol.mjs'

const nativeFixture = process.argv[2] === '--native-fixture' ? process.argv[3] : null
assert.ok(process.argv.length === 2 || (nativeFixture && process.argv.length === 4),
  'Usage: node smoke-opencode-session.mjs [--native-fixture EXISTING_PRIVATE_DIRECTORY]')
const temporary = await mkdtemp(join(nativeFixture || tmpdir(), 'studio-opencode-smoke-'))
const provider = createServer(async (request, response) => {
  let body = ''
  for await (const chunk of request) {
    body += chunk
    if (body.length > 8 * 1024 * 1024) { response.writeHead(413); response.end(); return }
  }
  if (!request.url.endsWith('/chat/completions')) { response.writeHead(404); response.end(); return }
  provider.requests += 1
  const sequence = provider.requests
  if (nativeFixture) await appendFile(join(nativeFixture, 'model-requests.jsonl'),
    JSON.stringify({ sequence, body: JSON.parse(body) }) + '\n')
  const prefix = `local OpenCode smoke reply ${sequence} `
  const chunks = [
    { delta: { role: 'assistant', content: prefix }, finish_reason: null },
    { delta: { content: '中文😀' }, finish_reason: null },
    { delta: {}, finish_reason: 'stop' },
  ].map((choice, index) => ({ id: `chatcmpl-smoke-${sequence}`, object: 'chat.completion.chunk', created: 1, model: 'smoke', choices: [{ index: 0, ...choice }], ...(index === 2 ? { usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } } : {}) }))
  response.writeHead(200, { 'Content-Type': 'text/event-stream' })
  if (nativeFixture) await delay(350)
  response.write(`data: ${JSON.stringify(chunks[0])}\n\n`)
  if (nativeFixture) await delay(600)
  response.end(chunks.slice(1).map(chunk => `data: ${JSON.stringify(chunk)}\n\n`).join('') + 'data: [DONE]\n\n')
})
provider.requests = 0
await new Promise(resolve => provider.listen(0, '127.0.0.1', resolve))
let child, events, readerTask
const received = [], eventAbort = new AbortController()
async function stopServer() {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return
  const stopped = new Promise(resolve => child.once('exit', resolve))
  const timeout = setTimeout(() => child.kill('SIGKILL'), 5000)
  child.kill()
  try { await stopped } finally { clearTimeout(timeout) }
}
try {
  for (const directory of ['home', 'config', 'data', 'cache', 'state', 'work']) await mkdir(join(temporary, directory))
  const work = join(temporary, 'work')
  const config = { model: 'smoke/smoke', small_model: 'smoke/smoke', share: 'disabled', autoupdate: false,
    enabled_providers: ['smoke'], permission: { '*': 'deny' }, provider: { smoke: {
      npm: '@ai-sdk/openai-compatible', name: 'Local smoke', options: { baseURL: `http://127.0.0.1:${provider.address().port}/v1`, apiKey: 'local-fixture' },
      models: { smoke: { name: 'smoke', limit: { context: 4096, output: 256 } } },
    } } }
  const environment = { ...process.env, HOME: join(temporary, 'home'), XDG_CONFIG_HOME: join(temporary, 'config'), XDG_DATA_HOME: join(temporary, 'data'), XDG_CACHE_HOME: join(temporary, 'cache'), XDG_STATE_HOME: join(temporary, 'state'),
    OPENCODE_CONFIG_CONTENT: JSON.stringify(config), OPENCODE_CONFIG_DIR: join(temporary, 'config'),
    OPENCODE_DISABLE_MODELS_FETCH: 'true', OPENCODE_DISABLE_DEFAULT_PLUGINS: 'true', OPENCODE_DISABLE_CLAUDE_CODE: 'true', OPENCODE_DISABLE_PROJECT_CONFIG: 'true', OPENCODE_DISABLE_AUTOUPDATE: 'true',
  }
  delete environment.OPENCODE_CONFIG
  child = spawn(process.env.STUDIO_OPENCODE_BIN || 'opencode', ['serve', '--hostname', '127.0.0.1', '--port', '0'], {
    cwd: work, env: { ...environment, OPENCODE_SERVER_PASSWORD: 'private-smoke' }, stdio: ['ignore', 'pipe', 'pipe'] })
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
  const health = await fetchApi('/global/health')
  assert.equal(health.status, 200)
  const version = (await health.json()).version
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
  if (nativeFixture) {
    eventAbort.abort(); await readerTask
    await stopServer()
    console.log(JSON.stringify({ nativeFixtureReady: true, temporary, threadId: thread.id, work, version,
      modelRequests: provider.requests, model: 'smoke/smoke',
      environment: Object.fromEntries(Object.entries(environment).filter(([key]) =>
        key === 'HOME' || key.startsWith('XDG_') || key.startsWith('OPENCODE_'))) }))
    // The native client now owns its own gateway/OpenCode process. Keep only the
    // shared loopback model alive until the owning test closes this input pipe.
    await new Promise(resolve => { process.stdin.once('end', resolve); process.stdin.resume() })
  } else {
    await protocol.openCodeRpc('thread/delete', { threadId: thread.id })
    console.log('OpenCode: authenticated server, Studio adapter create/rename/send, live SSE, local-model completion, history/resume/fork/delete passed')
  }
} finally {
  eventAbort.abort(); await readerTask
  await stopServer()
  provider.closeAllConnections(); await new Promise(resolve => provider.close(resolve))
  if (!nativeFixture) await rm(temporary, { recursive: true, force: true })
}
