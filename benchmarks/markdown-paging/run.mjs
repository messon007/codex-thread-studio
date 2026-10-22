import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { cpus } from 'node:os'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
const output = resolve(process.argv[2] || 'target/markdown-paging-results')
await mkdir(output, { recursive: true })
const binary = resolve(root, 'target/markdown-paging-build/release/studio-markdown-paging-bench')
const rounds = Number(process.env.ROUNDS || 3)
assert.ok(Number.isInteger(rounds) && rounds > 0)
const corpora = []
for (const [index, path] of process.argv.slice(3).entries()) {
  corpora.push({ name: `real-${index + 1}`, source: await readFile(path, 'utf8') })
}
const block = '## Requirement 中文 😀\n\nEvidence and **acceptance** context with [reference][ref].\n\n| Field | Value |\n| :--- | ---: |\n' + Array.from({ length: 12 }, (_, index) => `| evidence-${index} | 中文输入与输出结果，保留上下文并检查 **formatting** and \`value\` |\n`).join('') + '\n'
corpora.push({ name: 'report-30mib', source: '# Report\n\n' + block.repeat(Math.ceil(30 * 1024 * 1024 / Buffer.byteLength(block))) + '\n[ref]: https://example.com\n' })
if (corpora[0]) corpora.push({ name: 'long-tables-30mib', source: corpora[0].source.repeat(Math.ceil(30 * 1024 * 1024 / Buffer.byteLength(corpora[0].source))) })
const invoke = arguments_ => {
  const child = spawnSync(binary, arguments_, { encoding: 'utf8', maxBuffer: 1024 * 1024, timeout: 120_000 })
  if (child.error) throw child.error
  assert.equal(child.status, 0, child.stderr)
  return JSON.parse(child.stdout)
}
const report = { date: new Date().toISOString(), cpu: cpus()[0].model, node: process.version, rounds, corpora: [] }
for (const corpus of corpora) {
  const path = resolve(output, `${corpus.name}.md`)
  await writeFile(path, corpus.source)
  const entry = { name: corpus.name, sourceBytes: Buffer.byteLength(corpus.source), sha256: createHash('sha256').update(corpus.source).digest('hex'), samples: [] }
  for (const pageBytes of [32768, 65536, 131072]) {
    for (let round = 0; round < rounds; round++) {
      const directory = resolve(output, `${corpus.name}-${pageBytes}-${round}`)
      const native = invoke([path, directory, String(pageBytes), '200'])
      const cache = invoke(['cache', directory])
      assert.equal(native.sourceBytes, entry.sourceBytes)
      assert.equal(native.firstPages, 3)
      entry.samples.push({ pageBytes, round, directory, artifactsRetained: round === 0, native, cache })
      console.error(`${corpus.name}: ${pageBytes} round ${round + 1}: first ${native.firstReadyMs.toFixed(1)} ms, peak ${(native.peakRssKiB / 1024).toFixed(1)} MiB`)
      if (round > 0) await rm(directory, { recursive: true })
    }
  }
  report.corpora.push(entry)
  await writeFile(resolve(output, 'results.json'), JSON.stringify(report, null, 2))
}
console.log(JSON.stringify({ output, corpora: report.corpora.map(entry => ({ name: entry.name, sourceBytes: entry.sourceBytes, sha256: entry.sha256 })) }, null, 2))
