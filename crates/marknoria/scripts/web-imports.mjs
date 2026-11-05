// Validate owned browser modules without bundling duplicate runtime libraries.
import { readdir, realpath, stat } from 'node:fs/promises'
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { isBuiltin } from 'node:module'

export async function verifyWebImports(root, build, aliases = {}) {
  const project = await realpath(root instanceof URL ? fileURLToPath(root) : root)
  async function checked(path) {
    let canonical
    try {
      canonical = await realpath(path)
    } catch {
      throw Error(`Missing Web module: ${relative(project, path)}`)
    }
    const local = relative(project, canonical)
    if (local === '..' || local.startsWith('..' + sep) || isAbsolute(local))
      throw Error('Web module escapes the delivery root')
    if (!(await stat(canonical)).isFile()) throw Error(`Web import is not a file: ${local}`)
    return canonical
  }
  const web = join(project, 'web')
  const entries = await Promise.all(
    (await readdir(web))
      .filter((name) => /\.(?:mjs|js)$/.test(name))
      .sort()
      .map((name) => checked(join(web, name)))
  )
  if (!entries.length) throw Error('No owned Web modules to verify')
  await build({
    entryPoints: entries,
    bundle: true,
    splitting: true,
    format: 'esm',
    platform: 'browser',
    write: false,
    outdir: join(project, 'unused-import-check'),
    logLevel: 'silent',
    plugins: [
      {
        name: 'delivery-imports',
        setup(parser) {
          parser.onResolve({ filter: /.*/ }, async (reference) => {
            if (reference.kind === 'entry-point') return
            if (isBuiltin(reference.path)) throw Error('Browser module imports a Node builtin')
            if (reference.path.startsWith('.')) {
              const requested = resolve(dirname(reference.importer), reference.path)
              const key = relative(project, requested).split(sep).join('/')
              const alias = Object.hasOwn(aliases, key) ? aliases[key] : null
              await checked(alias ? resolve(project, alias) : requested)
            }
            // Generated bundles already have their own locked build. Do not rebundle
            // them for each entry or resolve host-provided bare package imports here.
            return { path: reference.path, external: true }
          })
        }
      }
    ]
  })
  return { modules: entries.length }
}
