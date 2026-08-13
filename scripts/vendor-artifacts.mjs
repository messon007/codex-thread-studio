import { build } from 'esbuild'
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises'

const root = new URL('../', import.meta.url)
const vendor = new URL('ui/vendor/', root)
const licenses = new URL('licenses/', vendor)

await mkdir(vendor, { recursive: true })
await mkdir(licenses, { recursive: true })

for (const name of ['pdf.min.mjs', 'pdf.worker.min.mjs']) {
  await copyFile(new URL(`node_modules/pdfjs-dist/build/${name}`, root), new URL(name, vendor))
}

await build({
  entryPoints: [new URL('ui/artifact-table-vendor-entry.mjs', root).pathname],
  outfile: new URL('artifact-table.mjs', vendor).pathname,
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: ['es2022'],
  minify: true,
  legalComments: 'none',
})

for (const [source, target] of [
  ['pdfjs-dist/LICENSE', 'pdfjs-Apache-2.0.txt'],
  ['exceljs/LICENSE', 'exceljs-MIT.txt'],
]) {
  const value = await readFile(new URL(`node_modules/${source}`, root), 'utf8')
  await writeFile(new URL(target, licenses), `${value.trimEnd()}\n`)
}
