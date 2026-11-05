// Real Windows Studio gateway -> explicitly selected WSL distribution/user.
// Run after provisioning Ubuntu-24.04/WSL2 + reader-test; never substitutes Linux tests.
import assert from 'node:assert/strict'
import {spawn, execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {createHash, randomUUID} from 'node:crypto'
import {mkdir, mkdtemp, readdir, writeFile} from 'node:fs/promises'
import {join, resolve, dirname} from 'node:path'
import {fileURLToPath} from 'node:url'
assert.equal(process.platform, 'win32', 'This check requires real Windows + WSL')
const exec = promisify(execFile)
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const distribution = process.env.STUDIO_TEST_WSL_DISTRIBUTION || 'Ubuntu-24.04'
const user = process.env.STUDIO_TEST_WSL_USER || 'reader-test'
const fixture = `/home/${user}/reader-check-${randomUUID()}`
const project = `${fixture}/project`, shared = `${fixture}/shared`, source = `${project}/notes.md`
const original = '\ufeff# Actual WSL 中文 😀\r\n\r\n![project](project.png) ![shared](../shared/shared.png)\r\n'
const png = 'iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII='
const wsl = async (...args) => (await exec('wsl.exe', ['--distribution', distribution, '--user', user, '--exec', ...args], {timeout: 30000, maxBuffer: 1024 * 1024})).stdout.trim()
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
const output = join(root, 'target/windows-wsl-reader-smoke')
await mkdir(output, {recursive: true})
const directory = await mkdtemp(join(output, 'run-')), cache = join(directory, 'reader-cache')
await mkdir(cache)
const config = join(directory, 'config', 'codex-thread-studio')
await mkdir(config, {recursive: true})
await writeFile(join(config, 'settings.json'), JSON.stringify({wslDistribution: distribution, wslUser: user, sharedDocumentDirectories: [shared]}))
let child, log = '', setup = false, closeLease, failure
const leases = new Set()
try {
  const wslKernel = await wsl('/usr/bin/uname', '-r')
  assert.match(wslKernel, /microsoft.*WSL2/iu, 'CI must exercise the documented WSL2 backend')
  console.log(JSON.stringify({actualWindowsWslKernel: wslKernel, distribution, user}))
  const identity = JSON.parse(await wsl('python3', '-c', 'import os,json;print(json.dumps([os.environ["WSL_DISTRO_NAME"],os.getuid(),__import__("pwd").getpwuid(os.getuid()).pw_name]))'))
  assert.equal(identity[0], distribution); assert.equal(identity[2], user); assert.notEqual(identity[1], 0)
  setup = true
  await wsl('python3', '-c', 'import os,sys,base64;root,md,png=sys.argv[1:];os.mkdir(root,0o700);os.mkdir(root+"/project");os.mkdir(root+"/shared");open(root+"/project/notes.md","wb").write(base64.b64decode(md));[open(root+p,"wb").write(base64.b64decode(png)) for p in ["/project/project.png","/shared/shared.png"]]', fixture, Buffer.from(original).toString('base64'), png)
  // Separate raw wsl.exe pipe fidelity from the production supervised launcher.
  const raw = (await exec('wsl.exe', ['--distribution', distribution, '--user', user, '--exec', '/usr/bin/head', '-c', '134217729', source], {encoding: null, timeout: 30000, maxBuffer: 1024 * 1024})).stdout
  const expectedBytes = Buffer.from(original)
  const firstDifference = Array.from({length: Math.max(raw.length, expectedBytes.length)}, (_, i) => i).find(i => raw[i] !== expectedBytes[i]) ?? null
  const digest = bytes => createHash('sha256').update(bytes).digest('hex')
  const rawDiagnostic = {rawWindowsWslPipe: raw.equals(expectedBytes), expectedBytes: expectedBytes.length, actualBytes: raw.length, expectedSha256: digest(expectedBytes), actualSha256: digest(raw), firstDifference}
  console.log(JSON.stringify(rawDiagnostic))
  assert.equal(rawDiagnostic.rawWindowsWslPipe, true, 'Raw WSL pipe differs from the original fixture')
  const canonical = await wsl('realpath', source)
  const expectedId = 'wsl:v1:' + createHash('sha256').update(JSON.stringify(['wsl-document-v1', identity[0], identity[1], canonical])).digest('hex')
  const environment = {...process.env, XDG_CONFIG_HOME: join(directory, 'config'), XDG_DATA_HOME: join(directory, 'data'), XDG_CACHE_HOME: join(directory, 'cache'), READER_CACHE_DIR: cache, CODEX_HOME: join(directory, 'codex'), CODEX_THREAD_STUDIO_WINDOWS_BACKEND: 'wsl'}
  child = spawn(process.env.STUDIO_REMOTE_BINARY || join(root, 'target/debug/codex-thread-studio.exe'), ['--serve', '--listen', '127.0.0.1:0'], {env: environment, stdio: ['ignore', 'pipe', 'pipe']})
  let spawnError
  child.on('error', error => { spawnError = error })
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => { log = (log + bytes).slice(-100000) })
  const until = Date.now() + 30000
  while (!/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u.test(log)) {
    if (spawnError) throw spawnError
    assert.ok(Date.now() < until && child.exitCode === null, 'Studio gateway did not start: ' + log.replace(/token=[a-f0-9]+/gu, 'token=[redacted]'))
    await pause(30)
  }
  const url = new URL(log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u)[0])
  const auth = {Authorization: 'Bearer ' + new URLSearchParams(url.hash.slice(1)).get('token')}
  const gateway = (path, method = 'GET', body) => fetch(new URL(path, url), {method, headers: {...auth, ...(body ? {'Content-Type': 'application/json'} : {})}, ...(body ? {body: JSON.stringify(body)} : {}), signal: AbortSignal.timeout(30000)})
  closeLease = async id => {
    const response = await fetch(new URL('/studio/markdown/close', url), {method: 'POST', headers: {...auth, 'Content-Type': 'application/json'}, body: JSON.stringify({id}), signal: AbortSignal.timeout(3000)})
    assert.ok([200, 410].includes(response.status), 'Could not close reader lease')
    leases.delete(id)
  }
  const open = async () => {
    const response = await gateway('/studio/reader/open', 'POST', {root: project, path: source})
    assert.equal(response.status, 200, await response.clone().text())
    const opened = await response.json()
    leases.add(opened.reader.id)
    assert.equal(opened.executionEnvironment, 'wsl'); assert.equal(opened.readOnly, true)
    const endpoint = new URL(opened.reader.url, url), headers = {'x-reader-token': endpoint.hash.slice(1)}
    const call = path => fetch(new URL(path, endpoint), {headers, signal: AbortSignal.timeout(30000)})
    let document
    const deadline = Date.now() + 30000
    do {
      const response = await call('api/document'); assert.equal(response.status, 200)
      document = await response.json(); assert.notEqual(document.state, 'error', document.error)
      if (document.state === 'ready') break
      assert.ok(Date.now() < deadline, 'WSL indexing timed out'); await pause(30)
    } while (true)
    headers['x-reader-document'] = document.documentId
    assert.equal(document.resourceId, expectedId)
    return {opened, call, document}
  }
  const first = await open(), {call, document} = first
  assert.equal((await (await call('api/source?line=1')).json()).result.text, original)
  assert.equal((await (await call('api/capabilities')).json()).edit, false)
  assert.equal((await call('api/edit')).status, 403)
  const imagePath = id => `api/reference-image?id=${id}&revision=${document.index.revision}`
  const readyImage = async id => {
    const deadline = Date.now() + 30000
    while (Date.now() < deadline) {
      const response = await call(imagePath(id)); assert.ok([200, 202].includes(response.status), await response.clone().text())
      const image = await response.json(); if (image.state === 'ready') return image
      await pause(30)
    }
    throw Error('WSL image preparation timed out')
  }
  const projectImage = await readyImage(0), sharedImage = await readyImage(1)
  const framePath = (id, image) => imagePath(id) + `&frame=0&imageRevision=${image.imageRevision}`
  assert.equal((await call(framePath(0, projectImage))).status, 200)
  for (let n = 0; n < 2; n++) assert.equal((await call(framePath(1, sharedImage))).status, 200)
  const preferences = await (await gateway('/studio/preferences')).json()
  assert.equal((await gateway('/studio/preferences', 'PUT', {...preferences, sharedDocumentDirectories: []})).status, 200)
  assert.equal((await call(imagePath(1))).status, 403)
  assert.equal((await call(framePath(1, sharedImage))).status, 403)
  assert.equal((await call(framePath(0, projectImage))).status, 200)
  await closeLease(first.opened.reader.id)
  assert.equal((await call('api/document')).status, 410)
  const reopened = await open()
  assert.equal(reopened.document.resourceId, document.resourceId)
  await closeLease(reopened.opened.reader.id)
  const deadline = Date.now() + 10000
  while ((await readdir(cache)).length && Date.now() < deadline) await pause(30)
  assert.deepEqual(await readdir(cache), [])
  assert.equal(await wsl('python3', '-c', 'import sys,base64;print(base64.b64encode(open(sys.argv[1],"rb").read()).decode())', source), Buffer.from(original).toString('base64'))
  console.log(JSON.stringify({ok: true, actualWindowsWsl: true, productionGateway: true, distribution, user, wslKernel, uid: identity[1], exactSource: true, readOnly: true, cachedResourceRevocation: true, stableIdentity: true, cleanup: true}))
} catch (error) {
  failure = error
  throw error
} finally {
  const cleanupErrors = []
  for (const id of leases) {
    try { await closeLease(id) } catch (error) { cleanupErrors.push(error) }
  }
  if (child && child.exitCode === null && child.signalCode === null && child.pid) {
    try { await exec('taskkill.exe', ['/pid', String(child.pid), '/t', '/f'], {timeout: 10000}) }
    catch (error) { if (child.exitCode === null) cleanupErrors.push(error) }
  }
  if (setup) {
    try { await wsl('python3', '-c', 'import shutil,sys,os;shutil.rmtree(sys.argv[1]) if os.path.exists(sys.argv[1]) else None', fixture) }
    catch (error) { cleanupErrors.push(error) }
  }
  if (cleanupErrors.length) {
    if (failure) console.error(`Additionally, ${cleanupErrors.length} isolated fixture cleanup steps failed`)
    else throw new AggregateError(cleanupErrors, 'Real WSL check cleanup failed')
  }
}
