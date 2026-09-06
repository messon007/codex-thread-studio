import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

for (const script of ['vendor-workspace-tools.mjs', 'vendor-epub.mjs', 'vendor-artifacts.mjs']) {
  test(`${script} uses native filesystem paths for esbuild on Windows`, () => {
    const source = readFileSync(new URL(script, import.meta.url), 'utf8')
    assert.match(source, /import \{ fileURLToPath \} from 'node:url'/u)
    assert.match(source, /entryPoints: \[fileURLToPath\(/u)
    assert.match(source, /outfile: fileURLToPath\(/u)
    assert.doesNotMatch(source, /\.pathname/u)
  })
}
