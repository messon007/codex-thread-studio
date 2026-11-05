import { copyFile, mkdir, readdir, writeFile } from 'node:fs/promises'

// Explicit PDF.js resource groups. Scripting/QuickJS is deliberately absent.
export async function buildPdfAssets(root) {
  const resources = [
    ['build/pdf.min.mjs', 'pdf.mjs'], ['build/pdf.worker.min.mjs', 'pdf.worker.mjs'],
    ['web/pdf_viewer.css', 'pdf-viewer.css'], ['LICENSE', 'LICENSE'],
  ]
  for (const [group, allowed] of [
    ['cmaps', /\.bcmap$|^LICENSE$/], ['standard_fonts', /\.(?:pfb|ttf)$|^LICENSE/],
    ['wasm', /^(?:(?:openjpeg|jbig2|qcms_bg)\.wasm|(?:openjpeg|jbig2)_nowasm_fallback\.js|LICENSE.*)$/],
  ]) {
    for (const name of (await readdir(new URL(`node_modules/pdfjs-dist/${group}/`, root))).sort()) {
      if (allowed.test(name)) resources.push([`${group}/${name}`, `${group}/${name}`])
    }
  }
  let source = '// Generated from the exact pdfjs-dist package lock by scripts/pdf-assets.mjs.\n'
  source += 'pub(crate) fn pdf_asset(path: &str) -> Option<(&\'static str, &\'static [u8])> {\n Some(match path {\n'
  for (const [from, to] of resources) {
    const target = new URL(`web/vendor/pdf/${to}`, root)
    await mkdir(new URL('./', target), { recursive: true }); await copyFile(new URL(`node_modules/pdfjs-dist/${from}`, root), target)
    const mime = to.endsWith('.mjs') || to.endsWith('.js') ? 'text/javascript' : to.endsWith('.css') ? 'text/css' : to.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream'
    source += `${JSON.stringify(to)} => (${JSON.stringify(mime)}, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), ${JSON.stringify('/web/vendor/pdf/' + to)}))),\n`
  }
  source += '_ => return None,\n })\n}\n'
  await writeFile(new URL('web/vendor/pdf-assets.rs', root), source)
}
