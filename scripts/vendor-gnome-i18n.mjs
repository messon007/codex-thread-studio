// Reuse Studio's interface catalog without a JavaScript runtime in the native app.
import { translationEntries } from '../ui/i18n.mjs'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
const root = new URL('../', import.meta.url)
const extra = JSON.parse(await readFile(new URL('native/gnome/locales/zh-CN.json', root), 'utf8'))
const shared = translationEntries['zh-CN']
for (const [key, value] of Object.entries(extra)) {
  if (typeof value !== 'string' || !value.length || Object.hasOwn(shared, key)) {
    throw new Error(`Invalid or duplicate native translation: ${key}`)
  }
}
const entries = Object.entries({ ...shared, ...extra }).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
const content = JSON.stringify(Object.fromEntries(entries), null, 2) + '\n'
const directory = new URL('native/gnome/vendor/i18n/', root)
const output = new URL('zh-CN.json', directory)
if (process.argv.includes('--check')) {
  if (await readFile(output, 'utf8') !== content) throw new Error('Native catalog is stale; run node scripts/vendor-gnome-i18n.mjs')
} else {
  await mkdir(directory, { recursive: true })
  await writeFile(output, content)
}
