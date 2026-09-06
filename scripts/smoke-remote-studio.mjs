import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm, access } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
// Optional integration test; build the binary and provide an installed Playwright.
const binary = process.env.STUDIO_REMOTE_BINARY || fileURLToPath(new URL('../target/debug/codex-thread-studio', import.meta.url))
const profile = await mkdtemp(join(tmpdir(), 'ssh-studio-smoke-'))
const env = { ...process.env, XDG_CONFIG_HOME:profile, XDG_DATA_HOME:join(profile,'data'), CODEX_HOME:join(profile,'codex') }
const children = []
const check = (condition, message) => { if (!condition) throw Error(message) }
function start(args) {
  const child = spawn(binary, args, {env,stdio:['ignore','pipe','pipe']})
  children.push(child)
  child.log = ''
  child.stdout.on('data', bytes => child.log += bytes)
  child.stderr.on('data', bytes => child.log += bytes)
  return child
}
function exited(child) { return new Promise((resolve,reject) => {child.on('exit',resolve);child.on('error',reject)}) }
async function ready(child) {
  const deadline = Date.now()+15000
  while (Date.now()<deadline) {
    const url = child.log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/)?.[0]
    if (url) return new URL(url)
    if (child.exitCode !== null) throw Error(`server startup failed: ${child.log}`)
    await new Promise(resolve => setTimeout(resolve,50))
  }
  throw Error('server readiness timeout')
}
let browser
try {
  const server = start(['--serve','--listen','127.0.0.1:0'])
  const url = await ready(server)
  const settingsPath = join(profile,'codex-thread-studio/settings.json')
  const settings = await readFile(settingsPath,'utf8')
  await access(join(profile,'codex-thread-studio/studio.sqlite3'))
  for (const args of [['--serve','--listen','127.0.0.1:0'], []]) {
    const duplicate = start(args)
    const code = await exited(duplicate)
    check(code === 2 && duplicate.log.includes('already in use'), 'duplicate desktop/server was not rejected')
  }
  check(await readFile(settingsPath,'utf8') === settings, 'rejected instance changed settings')
  const unauth = await fetch(new URL('/studio/session-state',url))
  check(unauth.status === 401, 'API accepts missing credentials')
  browser = await chromium.launch({headless:true,executablePath:process.env.STUDIO_CHROMIUM_PATH})
  const context = await browser.newContext()
  // Exercise the actual served bootstrap and auth API without loading app.js,
  // which would connect to and start backend sessions.
  await context.route(`${url.origin}/`, route => route.fulfill({contentType:'text/html',body:'<script src="/remote-bootstrap.js"></script><button id="link">Link</button>'}))
  const page = await context.newPage()
  await page.goto(url.href)
  const authStatus = () => page.evaluate(async () => (await fetch('/studio/session-state',{headers:{Authorization:`Bearer ${window.__CODEX_THREAD_STUDIO_GATEWAY__.token}`}})).status)
  check(await authStatus() === 200, 'initial authentication failed')
  check(!page.url().includes('token'), 'token was not cleared')
  await page.reload()
  check(await authStatus() === 200, 'reload authentication failed')
  const native = await context.newPage()
  await native.addInitScript(() => Object.defineProperty(window,'__CODEX_THREAD_STUDIO_GATEWAY__',{value:Object.freeze({token:'native-credential',hostPlatform:'linux'}),configurable:false}))
  await native.goto(url.origin)
  check(await native.evaluate(() => window.__CODEX_THREAD_STUDIO_GATEWAY__.token === 'native-credential' && !window.__CODEX_THREAD_STUDIO_GATEWAY__.remote), 'desktop credentials overwritten')
  const stopped = exited(server)
  server.kill('SIGKILL')
  await stopped
  const restarted = start(['--serve','--listen','127.0.0.1:0'])
  await ready(restarted)
  console.log('PASS: headless startup, current studio DB, HTTP auth, reload auth, native bootstrap preservation, duplicate server/desktop exclusion, settings unchanged, lock released after SIGKILL')
} finally {
  await browser?.close()
  for (const child of children) if (child.exitCode === null && child.signalCode === null) { const done=exited(child);child.kill('SIGKILL');await done }
  await rm(profile,{recursive:true,force:true})
}
