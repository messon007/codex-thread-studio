import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { ownedD2Resource } from './d2-resource.mjs'
import { buildPdfAssets } from './pdf-assets.mjs'

const root = new URL('../', import.meta.url)
const args = process.argv.slice(2)
if (args.some(arg => !['--shared-only', '--web-only'].includes(arg)) || new Set(args).size > 1) {
  throw Error('Usage: node scripts/build.mjs [--shared-only | --web-only]')
}

async function sharedAssets() {
  await mkdir(new URL('assets/vendor/', root), { recursive: true })
  for (const [source, destination] of [
    ['dompurify/dist/purify.es.mjs', 'purify.mjs'],
    ['dompurify/LICENSE', 'purify-LICENSE.txt'],
    ['mermaid/dist/mermaid.min.js', 'mermaid.min.js'],
    ['mermaid/LICENSE', 'mermaid-MIT.txt'],
    ['@plantuml/core/plantuml.js', 'plantuml.js'],
    ['@plantuml/core/viz-global.js', 'plantuml-viz-global.js'],
    ['@plantuml/core/LICENSE', 'plantuml-MIT.txt'],
    ['@d2lang/d2/LICENSE.txt', 'd2-MPL-2.0.txt'],
    ['@d2lang/d2/THIRD_PARTY_NOTICES.txt', 'd2-THIRD_PARTY_NOTICES.txt'],
  ]) await copyFile(new URL(`node_modules/${source}`, root), new URL(`assets/vendor/${destination}`, root))
  await writeFile(new URL('assets/vendor/d2.js', root), ownedD2Resource(await readFile(new URL('node_modules/@d2lang/d2/dist/browser/index.js', root), 'utf8')))
}

async function webAssets() {
  const { build } = await import('esbuild')
  await mkdir(new URL('web/vendor/', root), { recursive: true })
  // Browser modules must also work from the exported filesystem, without the
  // HTTP server's shared-asset alias or a stale development bundle.
  await copyFile(new URL('node_modules/dompurify/dist/purify.es.mjs', root), new URL('web/vendor/purify.mjs', root))
  await copyFile(new URL('node_modules/dompurify/LICENSE', root), new URL('web/vendor/purify-LICENSE.txt', root))
  await buildPdfAssets(root)
  await build({ stdin: { contents: "export { default as ePub } from 'epubjs'; export { replaceBase } from 'epubjs/src/utils/replacements.js'", resolveDir: fileURLToPath(root) }, bundle: true, format: 'esm', minify: true, outfile: fileURLToPath(new URL('web/vendor/epub.mjs', root)) })
  await copyFile(new URL('node_modules/epubjs/license', root), new URL('web/vendor/epub-BSD-2-Clause.txt', root))
  await build({ entryPoints: [fileURLToPath(new URL('web/code-view.mjs', root))], bundle: true, format: 'esm', minify: true, outfile: fileURLToPath(new URL('web/vendor/editor.mjs', root)) })
  await build({
    stdin: { contents: "export * from '@tanstack/virtual-core'", resolveDir: fileURLToPath(root) },
    bundle: true,
    format: 'esm',
    minify: true,
    outfile: fileURLToPath(new URL('web/vendor/virtual-core.mjs', root)),
  })
  await copyFile(new URL('node_modules/@tanstack/virtual-core/LICENSE', root), new URL('web/vendor/virtual-MIT.txt', root))
  await copyFile(new URL('node_modules/@codemirror/legacy-modes/LICENSE', root), new URL('web/vendor/codemirror-legacy-MIT.txt', root))
  const { verifyWebImports } = await import('./web-imports.mjs')
  // The private full-reader controller loads the shared engine through this
  // deliberate HTTP route alias; ordinary components need no such mapping.
  const verified = await verifyWebImports(root, build, {
    'web/engine.mjs': 'assets/diagrams/engine.mjs'
  })
  console.log(`Web source imports verified: ${verified.modules} modules`)
}

if (!args.includes('--shared-only')) await webAssets()
if (!args.includes('--web-only')) await sharedAssets()
