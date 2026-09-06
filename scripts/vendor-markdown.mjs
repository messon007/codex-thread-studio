import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const vendor = resolve(root, 'ui/vendor')
const licenses = resolve(vendor, 'licenses')

await mkdir(vendor, { recursive: true })
await mkdir(licenses, { recursive: true })
await Promise.all([
  copyFile(resolve(root, 'node_modules/marked/lib/marked.esm.js'), resolve(vendor, 'marked.esm.js')),
  copyFile(resolve(root, 'node_modules/dompurify/dist/purify.es.mjs'), resolve(vendor, 'purify.es.mjs')),
  copyFile(resolve(root, 'node_modules/github-markdown-css/github-markdown.css'), resolve(vendor, 'github-markdown.css')),
  copyFile(resolve(root, 'node_modules/mermaid/dist/mermaid.min.js'), resolve(vendor, 'mermaid.min.js')),
  copyFile(resolve(root, 'node_modules/marked/LICENSE'), resolve(licenses, 'marked-MIT.md')),
  copyFile(resolve(root, 'node_modules/dompurify/LICENSE'), resolve(licenses, 'dompurify-Apache-2.0.txt')),
  copyFile(resolve(root, 'node_modules/dompurify/LICENSE-MPL'), resolve(licenses, 'dompurify-MPL-2.0.txt')),
  copyFile(resolve(root, 'node_modules/github-markdown-css/license'), resolve(licenses, 'github-markdown-css-MIT.txt')),
  copyFile(resolve(root, 'node_modules/mermaid/LICENSE'), resolve(licenses, 'mermaid-MIT.txt')),
])

await Promise.all([
  normalizeText(resolve(vendor, 'github-markdown.css')),
  normalizeText(resolve(licenses, 'dompurify-MPL-2.0.txt')),
])

console.log('Vendored Marked, DOMPurify, Mermaid, and GitHub Markdown CSS into ui/vendor')

async function normalizeText(path) {
  const source = await readFile(path, 'utf8')
  const normalized = `${source.replace(/[ \t]+$/gm, '').trimEnd()}\n`
  await writeFile(path, normalized)
}
