// DOM-dependent EPUB CFI compatibility, without the browser ZIP/book loader.
import { build } from 'esbuild'
import { mkdir, copyFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
const root = new URL('../', import.meta.url)
const output = new URL('native/gnome/vendor/epub/', root)
await mkdir(output, { recursive: true })
await build({
  stdin: { contents: "export { default as CFI } from 'epubjs/src/epubcfi.js'", resolveDir: fileURLToPath(root) },
  outfile: fileURLToPath(new URL('cfi.js', output)), bundle: true,
  format: 'iife', globalName: 'StudioEpubCFI', platform: 'browser',
  target: ['es2022'], minify: true, legalComments: 'none',
  plugins: [{ name: 'browser-dom', setup(builder) {
    builder.onResolve({ filter: /^@xmldom\/xmldom$/ }, () => ({ path: 'browser-dom', namespace: 'native-epub' }))
    builder.onLoad({ filter: /.*/, namespace: 'native-epub' }, () => ({ contents: 'export const DOMParser = globalThis.DOMParser', loader: 'js' }))
  } }],
})
await copyFile(new URL('node_modules/epubjs/license', root), new URL('LICENSE', output))
