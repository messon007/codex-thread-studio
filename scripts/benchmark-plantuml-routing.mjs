import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { performance } from 'node:perf_hooks'
import { Marked } from '../ui/vendor/marked.esm.js'
import { markdownParserForSource, plantumlMarkdownExtension } from '../ui/diagram-source.mjs'

const block = '## Background 中文\n\nA reusable parser should preserve **formatting**, *emphasis*, links and source anchors. SIMD accelerates scanning, not browser layout.\n第二行包含中文字符和 emoji 😀。\n\n'
const paths = process.argv.slice(2)
const corpora = paths.length
  ? paths.map(path => ({ name: path, source: readFileSync(path, 'utf8') }))
  : [{ name: 'prose-5mib', source: block.repeat(Math.ceil(5 * 1024 * 1024 / Buffer.byteLength(block))) }]
const legacy = new Marked({ async: false, breaks: true, gfm: true, extensions: [plantumlMarkdownExtension()] })
const results = []
for (const { name, source } of corpora) {
  const expected = legacy.parse(source)
  const modes = [
    { name: 'always-on', samples: 3, render: () => legacy.parse(source) },
    { name: 'routed-chat', samples: 15, render: () => markdownParserForSource(source).parse(source) },
    { name: 'routed-document', samples: 15, render: () => {
      const parser = markdownParserForSource(source)
      return parser.parser(parser.lexer(source))
    } },
  ]
  const measurements = {}
  for (const mode of modes) {
    assert.equal(mode.render(), expected)
    const samplesMs = []
    for (let index = 0; index < mode.samples; index++) {
      const start = performance.now()
      const output = mode.render()
      samplesMs.push(performance.now() - start)
      assert.equal(output, expected)
    }
    const sorted = [...samplesMs].sort((left, right) => left - right)
    measurements[mode.name] = { medianMs: sorted[Math.floor(sorted.length / 2)], samplesMs }
  }
  results.push({ name, inputBytes: Buffer.byteLength(source), sha256: createHash('sha256').update(source).digest('hex'), identicalHtml: true, measurements })
}
console.log(JSON.stringify({ node: process.version, scope: 'parse plus HTML only; two untimed baseline parses and one untimed routed parse per mode; no sanitizer or DOM', results }, null, 2))
