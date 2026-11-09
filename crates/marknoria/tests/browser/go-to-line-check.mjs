import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { once } from 'node:events'
import { promisify } from 'node:util'
import { mkdir, mkdtemp, writeFile, readFile } from 'node:fs/promises'
import { resolve, join } from 'node:path'
const base = resolve('target/navigation-checks')
await mkdir(base, { recursive: true })
const dir = await mkdtemp(join(base, 'run-')), root = join(dir, 'files')
await mkdir(root)
const text = ['\uFEFF# Navigation', '', '```rust', ...Array.from({length: 600}, (_, i) => `let source_line_${i} = "中文😀";`), '```', '', '## Table', '', '| Name | Value |', '|---|---|', ...Array.from({length: 350}, (_, i) => `| row-${i} | table-needle-${i} |`), ''].join('\r\n')
await writeFile(join(root, 'navigation.md'), text)
await writeFile(join(root, 'compact.json'), JSON.stringify({ records: Array.from({length: 12000}, (_, i) => ({ id:i, text:`json-value-${i}` })) }) + '\n')
const server = spawn(resolve(process.argv.slice(2).find(arg => !arg.startsWith('--')) || 'target/debug/marknoria-server'), [join(root, 'navigation.md'), '--root', root], { env:{...process.env,READER_CACHE_DIR:join(dir,'cache'),MARKNORIA_CONFIG_DIR:join(dir,'config')}, stdio:['ignore','pipe','pipe'] })
let stdout = '', stderr = '', browser
server.stdout.on('data', data => stdout += data); server.stderr.on('data', data => stderr += data)
try {
  for (let n=0; n<900 && !stdout.includes('http://'); n++) { if (server.exitCode != null) throw Error(stderr); await new Promise(resolve => setTimeout(resolve,30)) }
  const url = new URL(stdout.trim().split('\n')[0]), headers = {'x-reader-token':url.hash.slice(1)}
  const {chromium} = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
  const page = await browser.newPage({viewport:{width:1440,height:850}}), errors = []
  page.on('pageerror', error => errors.push(String(error)))
  const contract = (await readFile('tests/browser/contracts/go-to-line.mjs','utf8')).replace('export async function run()', 'async function run()')
  await page.goto(url.href)
  await page.waitForFunction(() => window.readerExperiment?.metrics.firstContentMs != null)
  await page.keyboard.press('Control+g')
  await page.locator('.reader-go-to-line[open]').waitFor()
  await page.keyboard.press('Escape')
  const markdown = await page.evaluate(`(async()=>{${contract};return run()})()`)
  await page.locator('#preview-mode').click()
  await page.waitForFunction(() => !readerExperiment.editor.visible && !document.getElementById('preview-mode').disabled)
  await page.locator('.reader-settings > summary').click()
  await page.locator('#reading-text-size').fill('21')
  await page.locator('#reading-text-size').dispatchEvent('change')
  await page.waitForFunction(() => readerExperiment.pendingNavigation == null)
  await page.reload()
  await page.waitForFunction(() => readerExperiment?.metrics.firstContentMs != null)
  assert.equal(await page.locator('#reading-text-size').inputValue(), '21', 'Font preference did not survive reload')
  // Full compact JSON uses source physical lines rather than pretty-print lines.
  const opened = await fetch(new URL('/api/open?path=compact.json',url),{method:'POST',headers})
  assert.equal(opened.status,200,await opened.clone().text())
  await page.reload()
  const json = await page.evaluate(`(async()=>{${contract};return run()})()`)
  assert.deepEqual(errors,[])
  if (process.argv.includes('--webkit')) {
    const reset = await fetch(new URL('/api/open?path=navigation.md',url),{method:'POST',headers})
    assert.equal(reset.status,200,await reset.clone().text())
    const result = await promisify(execFile)('/usr/bin/python3',['tests/browser/search-position-webkit.py',url.href,'--contract','go-to-line.mjs'],{env:{...process.env,GDK_BACKEND:'wayland',XDG_CACHE_HOME:join(dir,'webkit-cache')},timeout:180000})
    console.log(result.stdout.trim())
  }
  console.log(JSON.stringify({ok:true,markdown,json,browserVersion:browser.version(),artifacts:dir}))
} finally {
  await browser?.close()
  if (server.exitCode == null && server.signalCode == null) {const exited=once(server,'exit');server.kill('SIGINT');await exited}
}
