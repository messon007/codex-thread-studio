// Reproduce missing clean-package imports; fixtures stay in an owned directory.
import assert from 'node:assert/strict'
import { build } from 'esbuild'
import { mkdtemp, mkdir, realpath, writeFile, rm } from 'node:fs/promises'
import { join } from 'node:path'
import { verifyWebImports } from '../../scripts/web-imports.mjs'
const directory = await mkdtemp(join(await realpath('target'), 'web-import-check-'))
const project = join(directory, 'project'),
  web = join(project, 'web')
try {
  await mkdir(join(web, 'vendor'), { recursive: true })
  const entry = join(web, 'view.mjs')
  await writeFile(entry, "import DOMPurify from './vendor/purify.mjs'; export { DOMPurify }\n")
  await assert.rejects(verifyWebImports(project, build), /Missing Web module/)
  await writeFile(join(web, 'vendor/purify.mjs'), 'export default {}\n')
  assert.deepEqual(await verifyWebImports(project, build), { modules: 1 })
  await writeFile(join(directory, 'outside.mjs'), 'export const outside = 1\n')
  await writeFile(entry, "import '../../outside.mjs'\n")
  await assert.rejects(verifyWebImports(project, build), /escapes the delivery root/)
  await writeFile(entry, "import 'node:fs'\n")
  await assert.rejects(verifyWebImports(project, build), /Node builtin/)
  await mkdir(join(project, 'assets/diagrams'), { recursive: true })
  await writeFile(join(project, 'assets/diagrams/engine.mjs'), 'export const engine = true\n')
  await writeFile(entry, "import './engine.mjs'\n")
  await assert.rejects(verifyWebImports(project, build), /Missing Web module/)
  assert.deepEqual(
    await verifyWebImports(project, build, { 'web/engine.mjs': 'assets/diagrams/engine.mjs' }),
    { modules: 1 }
  )
  console.log(
    JSON.stringify({
      ok: true,
      checks: 6,
      cleanPackageImports: true,
      explicitRouteAlias: true,
      rootBoundary: true
    })
  )
} finally {
  await rm(directory, { recursive: true, force: true })
}
