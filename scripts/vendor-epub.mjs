import { build } from 'esbuild'
import { mkdir, readFile, writeFile } from 'node:fs/promises'

const root = new URL('../', import.meta.url)
const vendor = new URL('ui/vendor/', root)
const licenses = new URL('licenses/', vendor)

await mkdir(vendor, { recursive: true })
await mkdir(licenses, { recursive: true })
await build({
  entryPoints: [new URL('ui/epub-vendor-entry.mjs', root).pathname],
  outfile: new URL('epub.mjs', vendor).pathname,
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: ['es2022'],
  minify: true,
  legalComments: 'none',
})

for (const [source, target] of [
  ['epubjs/license', 'epubjs-BSD-2-Clause.txt'],
  ['@xmldom/xmldom/LICENSE', 'xmldom-MIT.txt'],
  ['core-js/LICENSE', 'core-js-MIT.txt'],
  ['event-emitter/LICENSE', 'event-emitter-MIT.txt'],
  ['jszip/LICENSE.markdown', 'jszip-MIT-or-GPL-3.0.txt'],
  ['localforage/LICENSE', 'localforage-Apache-2.0.txt'],
  ['lodash/LICENSE', 'lodash-MIT.txt'],
  ['immediate/LICENSE.txt', 'immediate-MIT.txt'],
  ['lie/license.md', 'lie-MIT.txt'],
  ['pako/LICENSE', 'pako-MIT-Zlib.txt'],
  ['readable-stream/LICENSE', 'readable-stream-MIT.txt'],
  ['setimmediate/LICENSE.txt', 'setimmediate-MIT.txt'],
  ['d/LICENSE', 'd-MIT.txt'],
  ['es5-ext/LICENSE', 'es5-ext-MIT.txt'],
]) {
  const license = await readFile(new URL(`node_modules/${source}`, root), 'utf8')
  await writeFile(new URL(target, licenses), `${license.replace(/[ \t]+$/gmu, '').trimEnd()}\n`)
}

await writeFile(new URL('epubjs-small-dependencies-MIT.txt', licenses), `MIT License

The vendored EPUB.js bundle includes marks-pane by Fred Chasen and path-webpack by
Fred Chasen. Their npm packages declare the MIT license but do not ship a separate
license file. Copyright remains with their respective authors.

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
`)
