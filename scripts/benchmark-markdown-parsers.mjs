import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { cpus, platform, arch } from 'node:os'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { performance } from 'node:perf_hooks'
import { marked } from '../ui/vendor/marked.esm.js'
import { plantumlMarkdownExtension } from '../ui/diagram-source.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
marked.setOptions({ async: false, breaks: true, gfm: true })
const args = process.argv.slice(2)
if (!args.includes('--core')) marked.use({ extensions: [plantumlMarkdownExtension()] })
const option = (name, fallback) => {
  const index = args.indexOf(name)
  return index < 0 ? fallback : args[index + 1]
}
const percentile = (values, fraction) => [...values].sort((left, right) => left - right)[Math.ceil(values.length * fraction) - 1]

if (args[0] === '--worker') {
  const source = readFileSync(args[1], 'utf8')
  const start = performance.now()
  let output = marked.parse(source)
  const firstMs = performance.now() - start
  const outputBytes = Buffer.byteLength(output)
  output = null
  for (let index = 0; index < 5; index++) marked.parse(source)
  const samplesMs = []
  for (let index = 0; index < Number(args[2]); index++) {
    const start = performance.now()
    output = marked.parse(source)
    samplesMs.push(performance.now() - start)
    if (Buffer.byteLength(output) !== outputBytes) throw Error('unstable output')
    output = null
  }
  console.log(JSON.stringify({ firstMs, inputBytes: Buffer.byteLength(source), outputBytes, samplesMs }))
  process.exit(0)
}

const outputDir = resolve(option('--output', resolve(root, 'target/markdown-parser-results')))
mkdirSync(outputDir, { recursive: true })
const samples = Number(option('--samples', '15'))
const rounds = Number(option('--rounds', '3'))
if (!Number.isInteger(samples) || samples < 1 || !Number.isInteger(rounds) || rounds < 1) throw Error('positive samples/rounds required')
const fixtures = {
  headings: '# 标题 😀\n\nIntro *emphasis* **strong** `inline`.\n\nSecond\n------\n',
  hardbreaks: '第一行\n第二行\nthird  \nfourth\n',
  table: '| left | right | center |\n| :--- | ---: | :---: |\n| a | b | c |\n',
  escapedPipes: '| A | B |\n| --- | --- |\n| a\\|b | `x\\|y` |\n',
  tasks: '- [x] ~~done~~\n- [ ] pending\n  - nested **item**\n',
  autolinks: 'See https://example.com/path?q=1 and www.example.com or user@example.com.\n',
  explicitLinks: '[相对链接](../README.md:1) ![图片](image.png "caption") <https://example.com>\n',
  reference: '[text][ref]\n\n[ref]: https://example.com "title"\n',
  lists: '3. three\n4. four\n\n   continued\n\n> quote\n>\n> - nested\n',
  html: '<details><summary>Title</summary>\n\n**body**\n\n</details>\n\n<span title="中文">inline</span>\n',
  fences: '```mermaid\ngraph TD; A-->B\n```\n\n```plantuml\n@startuml\nAlice -> Bob\n@enduml\n```\n\n```d2\na -> b\n```\n',
  standalonePlantuml: '@startuml\nAlice -> Bob\n@enduml\n',
  unsafeHtml: '<script>alert(1)</script>\n\n[bad](javascript:alert%281%29)\n\n<img src=x onerror="alert(1)">\n',
  crlf: '# 中文 😀\r\n\r\none\r\ntwo\r\n\r\n## Next\r\n',
  gfmEdges: 'a ~one~ ~~two~~\n\n[!NOTE]\n\n- [X] upper\n',
}
const fixtureDir = resolve(outputDir, 'fixtures')
mkdirSync(fixtureDir, { recursive: true })
for (const [name, source] of Object.entries(fixtures)) writeFileSync(resolve(fixtureDir, `${name}.md`), source)
const mixed = Object.values(fixtures).join('\n\n')
const tableBlock = '## Requirement 中文 😀\n\n| ID | Status | Description |\n| :--- | ---: | --- |\n| REQ-001 | pass | 中文验收 **bold** and `code` |\n| REQ-002 | pending | [reference](../source.md:42) |\n\n'
const proseBlock = '## Background 中文\n\nA reusable parser should preserve **formatting**, *emphasis*, links and source anchors. SIMD accelerates scanning, not browser layout.\n第二行包含中文字符和 emoji 😀。\n\n'
const cases = [{ name: 'readme', source: readFileSync(resolve(root, 'README.md'), 'utf8') }]
const realFile = option('--file')
if (realFile) cases.push({ name: 'acceptance-review', source: readFileSync(resolve(realFile), 'utf8') })
const repeatToBytes = (block, bytes) => block.repeat(Math.ceil(bytes / Buffer.byteLength(block)))
cases.push(
  { name: 'mixed-2mib', source: repeatToBytes(mixed, 2 * 1024 * 1024) },
  { name: 'tables-5mib', source: repeatToBytes(tableBlock, 5 * 1024 * 1024) },
  { name: 'prose-5mib', source: repeatToBytes(proseBlock, 5 * 1024 * 1024) },
)
const binaryDir = resolve(root, 'target/markdown-parser-bench/bin')
const engines = [
  { name: 'marked', command: process.execPath, prefix: [fileURLToPath(import.meta.url), '--worker'] },
  { name: 'marked-core', command: process.execPath, prefix: [fileURLToPath(import.meta.url), '--worker'], core: true },
  { name: 'pulldown', command: resolve(binaryDir, 'default'), prefix: ['pulldown'] },
  { name: 'pulldown-simd', command: resolve(binaryDir, 'simd'), prefix: ['pulldown'] },
  { name: 'comrak', command: resolve(binaryDir, 'default'), prefix: ['comrak'] },
]
const execute = (command, parameters) => {
  const result = spawnSync(command, parameters, { encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 })
  if (result.error) throw result.error
  if (result.status !== 0) throw Error(`${command}: ${result.stderr}`)
  return result.stdout
}
const report = {
  date: new Date().toISOString(),
  environment: { cpu: cpus()[0].model, threads: cpus().length, platform: platform(), arch: arch(), node: process.version, rust: execute('rustc', ['--version']).trim() },
  versions: { marked: readFileSync(resolve(root, 'ui/vendor/marked.esm.js'), 'utf8').match(/marked v([\d.]+)/u)[1], pulldown: '0.13.5', comrak: '0.57.0' },
  methodology: { samplesPerRound: samples, rounds, warmupPerProcess: 5, timing: 'parse plus HTML only; source preloaded; first call separate; no sanitizer, outline, IPC or DOM', memory: 'whole-process Linux peak RSS KiB via /usr/bin/time, including runtime and allocator retention' },
  cases: [],
}
for (const corpus of cases) {
  const path = resolve(outputDir, `${corpus.name}.md`)
  writeFileSync(path, corpus.source)
  const entry = { name: corpus.name, inputBytes: Buffer.byteLength(corpus.source), sha256: createHash('sha256').update(corpus.source).digest('hex'), engines: {} }
  for (let round = 0; round < rounds; round++) {
    for (let index = 0; index < engines.length; index++) {
      const engine = engines[(index + round) % engines.length]
      const rssPath = resolve(outputDir, 'rss.txt')
      const parameters = engine.name.startsWith('marked') ? [...engine.prefix, path, String(samples), ...(engine.core ? ['--core'] : [])] : [...engine.prefix, path, 'bench', String(samples)]
      const measured = JSON.parse(execute('/usr/bin/time', ['-f', '%M', '-o', rssPath, engine.command, ...parameters]))
      const accumulated = entry.engines[engine.name] ||= { samplesMs: [], firstMs: [], peakRssKiB: [], outputBytes: measured.outputBytes }
      accumulated.samplesMs.push(...measured.samplesMs)
      accumulated.firstMs.push(measured.firstMs)
      accumulated.peakRssKiB.push(Number(readFileSync(rssPath, 'utf8').trim()))
      console.error(`${corpus.name} round ${round + 1} ${engine.name}: ${percentile(measured.samplesMs, 0.5).toFixed(2)} ms`)
    }
  }
  for (const measured of Object.values(entry.engines)) {
    measured.medianMs = percentile(measured.samplesMs, 0.5)
    measured.p95Ms = percentile(measured.samplesMs, 0.95)
    measured.medianFirstMs = percentile(measured.firstMs, 0.5)
    measured.maxRssMiB = Math.max(...measured.peakRssKiB) / 1024
    measured.mibPerSecond = entry.inputBytes / 1024 / 1024 / (measured.medianMs / 1000)
  }
  report.cases.push(entry)
  writeFileSync(resolve(outputDir, 'results.json'), JSON.stringify(report, null, 2))
}
const compatibility = []
for (const [name, source] of Object.entries(fixtures)) {
  const path = resolve(fixtureDir, `${name}.md`)
  const outputs = { marked: marked.parse(source) }
  for (const engine of engines.filter(engine => ['pulldown', 'comrak'].includes(engine.name))) {
    outputs[engine.name] = execute(engine.command, [...engine.prefix, path, 'render'])
  }
  compatibility.push({ name, source, outputs })
}
writeFileSync(resolve(outputDir, 'compatibility.json'), JSON.stringify(compatibility, null, 2))
console.log(JSON.stringify(report.cases.map(entry => ({ name: entry.name, bytes: entry.inputBytes, engines: Object.fromEntries(Object.entries(entry.engines).map(([name, result]) => [name, { medianMs: result.medianMs, p95Ms: result.p95Ms, firstMs: result.medianFirstMs, peakRssMiB: result.maxRssMiB }])) })), null, 2))
