// Real Rust jq engine + public iframe protocol + ordinary DOM query component.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
const dir = await mkdtemp(join(await realpath('target'), 'embedded-jq-')),
  cache = join(dir, 'cache'),
  source = join(dir, 'public.jsonl')
await mkdir(cache)
const original =
  Array.from({ length: 1500 }, (_, id) =>
    JSON.stringify({
      id,
      name: `${String(id).padStart(6, '0')} 中文😀`,
      literal: '<script>inert</script>'
    })
  ).join('\n') + '\n'
await writeFile(source, original)
const modules = new Set([
  'host-client.mjs',
  'reader-bridge.mjs',
  'jq-query.mjs', 'query-help.mjs',
  'jq-query.css',
  'table-query.css',
  'translation.mjs',
  'query-operation.mjs',
  'icons.mjs',
  'clipboard.mjs',
  'component-styles.mjs'
])
const parent = createServer(async (req, res) => {
  const name = req.url.slice(1)
  if (modules.has(name)) {
    res.setHeader('Content-Type', name.endsWith('.css') ? 'text/css' : 'text/javascript')
    res.end(await readFile(join('web', name)))
    return
  }
  res.setHeader('Content-Type', 'text/html')
  res.end('<!doctype html><html><body><h1>Public jq host</h1></body></html>')
})
parent.listen(0, '127.0.0.1')
await once(parent, 'listening')
const origin = `http://127.0.0.1:${parent.address().port}`,
  children = []
let browser,
  assertions = 0
const check = (a, b) => {
    assert.deepEqual(a, b)
    assertions++
  },
  pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
async function service(disabled = false) {
  const child = spawn(
    resolve(process.argv[2] || 'target/release/examples/embedded-host'),
    [source, `--parent=${origin}`, ...(disabled ? ['--no-queries'] : [])],
    { env: { ...process.env, READER_CACHE_DIR: cache }, stdio: ['pipe', 'pipe', 'pipe'] }
  )
  children.push(child)
  let out = '',
    errors = ''
  child.stdout.on('data', (b) => (out += b))
  child.stderr.on('data', (b) => (errors += b))
  const deadline = Date.now() + 30000
  while (!out.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
  assert.match(out, /http:\/\/127\.0\.0\.1:/, errors)
  return out.trim()
}
try {
  const urls = [await service(), await service(true)]
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } }),
    errors = []
  page.on('pageerror', (error) => errors.push(String(error)))
  await page.goto(origin)
  const owners = await page.evaluate(async (urls) => {
    const { createReaderClient } = await import('/host-client.mjs')
    window.clients = urls.map((url, i) => {
      const frame = document.createElement('iframe')
      frame.title = `Reader ${i}`
      frame.src = url
      frame.style.cssText = 'width:700px;height:450px'
      document.body.append(frame)
      return createReaderClient(frame)
    })
    return Promise.all(clients.map((client) => client.ready()))
  }, urls)
  check(owners[0].capabilities.queries, true)
  check(owners[0].capabilities.edit, false)
  check(owners[1].capabilities.queries, false)
  const report = await page.evaluate(async (owners) => {
    const { mountJqQuery, readerJqProvider } = await import('/jq-query.mjs'),
      { createTranslator } = await import('/translation.mjs')
    const translate = createTranslator(),
      root = document.createElement('section')
    document.body.append(root)
    const provider = readerJqProvider(clients[0], owners[0]),
      query = mountJqQuery(root, { provider, revision: owners[0].revision, translate })
    try {
      await query.ready
      await query.execute('select(.id == 1499) | .name')
      const exact = root.querySelector('.jq-results').textContent
      await query.execute('map(.id) | length', { slurp: true })
      const aggregate = root.querySelector('.jq-results').textContent
      await query.execute('.', { slurp: false })
      const complete = query.state()
      await query.go(complete.pages - 1)
      const last = root.querySelector('.jq-results').textContent
      const id = query.state().id
      const foreign = await readerJqProvider(clients[1], owners[1])
        .page({ id, page: 0 })
        .then(
          () => false,
          () => true
        )
      const denied = await readerJqProvider(clients[1], owners[1])
        .run({ filter: '.', slurp: false })
        .then(
          () => false,
          () => true
        )
      const stale = await readerJqProvider(clients[0], { ...owners[0], revision: 'stale' })
        .run({ filter: '.', slurp: false })
        .then(
          () => false,
          () => true
        )
      translate.update({
        locale: 'fr',
        messages: { 'Run filter': 'Exécuter', 'Query JSON with jq': 'Interroger JSON' }
      })
      const translated = root.querySelector('.jq-run').textContent,
        literalAfter = root.querySelector('.jq-results').textContent
      await query.execute('.[').catch(() => {})
      const error = query.state().message,
        retained = root.querySelector('.jq-results').textContent
      await query.dispose()
      const expired = await provider.page({ id, page: 0 }).then(
        () => false,
        () => true
      )
      return {
        exact,
        aggregate,
        complete,
        last,
        foreign,
        denied,
        stale,
        translated,
        literalAfter,
        error,
        retained,
        expired,
        clean: !root.querySelector('dialog'),
        subscriptions: translate.state().subscribers
      }
    } finally {
      await query.dispose()
      root.remove()
    }
  }, owners)
  check(report.exact, '"001499 中文😀"\n\n')
  check(report.aggregate, '1500\n\n')
  check(report.complete.totalValues, 1500)
  check(report.complete.pages > 1, true)
  check(report.last.includes('"id": 1499'), true)
  check(report.foreign, true)
  check(report.denied, true)
  check(report.stale, true)
  check(report.translated, 'Exécuter')
  check(report.literalAfter, report.last)
  check(report.error.includes('Invalid jq filter'), true)
  check(report.retained, report.last)
  check(report.expired, true)
  check(report.clean, true)
  check(report.subscriptions, 0)
  await page.screenshot({ path: join(dir, 'ordinary-jq.png') })
  await page.evaluate(() => Promise.all(clients.map((client) => client.dispose())))
  check(errors, [])
  await browser.close()
  browser = null
  for (const child of children) {
    const exited = once(child, 'exit')
    child.stdin.end('\n')
    await Promise.race([exited, pause(10000)])
    check(child.exitCode, 0)
  }
  check(await readdir(cache), [])
  check(await readFile(source, 'utf8'), original)
  console.log(
    JSON.stringify({
      ok: true,
      assertions,
      publicProtocol: true,
      ordinaryHost: true,
      completeRustInput: true,
      readOnly: true,
      cleanup: true,
      artifacts: dir
    })
  )
} finally {
  await browser?.close()
  parent.closeAllConnections()
  parent.close()
  for (const child of children)
    if (child.exitCode == null && child.signalCode == null) {
      const exited = once(child, 'exit')
      child.kill('SIGKILL')
      await exited
    }
}
