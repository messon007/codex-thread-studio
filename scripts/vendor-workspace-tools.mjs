import { build } from 'esbuild'
import { copyFile, mkdir } from 'node:fs/promises'

await mkdir(new URL('../ui/vendor/', import.meta.url), { recursive: true })
await mkdir(new URL('../ui/vendor/licenses/', import.meta.url), { recursive: true })
for (const [entry, output] of [
  ['workspace-editor-vendor-entry.mjs', 'workspace-editor.mjs'],
  ['workspace-terminal-vendor-entry.mjs', 'workspace-terminal.mjs'],
]) {
  await build({
    entryPoints: [new URL(`../ui/${entry}`, import.meta.url).pathname],
    outfile: new URL(`../ui/vendor/${output}`, import.meta.url).pathname,
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: ['es2022'],
    minify: true,
    legalComments: 'none',
  })
}
await copyFile(
  new URL('../node_modules/@xterm/xterm/css/xterm.css', import.meta.url),
  new URL('../ui/vendor/xterm.css', import.meta.url),
)
for (const [source, target] of [
  ['@xterm/xterm/LICENSE', 'xterm-MIT.txt'],
  ['@xterm/addon-fit/LICENSE', 'xterm-addon-fit-MIT.txt'],
  ['codemirror/LICENSE', 'codemirror-MIT.txt'],
  ['@lezer/common/LICENSE', 'lezer-MIT.txt'],
  ['@marijn/find-cluster-break/LICENSE', 'find-cluster-break-MIT.txt'],
  ['style-mod/LICENSE', 'style-mod-MIT.txt'],
  ['w3c-keyname/LICENSE', 'w3c-keyname-MIT.txt'],
  ['crelt/LICENSE', 'crelt-MIT.txt'],
]) {
  await copyFile(
    new URL(`../node_modules/${source}`, import.meta.url),
    new URL(`../ui/vendor/licenses/${target}`, import.meta.url),
  )
}
