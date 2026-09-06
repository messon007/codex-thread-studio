import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, join } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'

test('schema lookup ignores npm-injected node_modules bin directories', () => {
  const npmBin = join(tmpdir(), 'workspace', 'node_modules', '.bin')
  const externalBin = join(tmpdir(), 'tools', 'bin')
  assert.equal(withoutNpmBinDirectories([npmBin, externalBin].join(delimiter)), externalBin)
})

test('installed Codex schema still exposes Studio structured interactions', async (context) => {
  const directory = await mkdtemp(join(tmpdir(), 'studio-app-server-schema-'))
  try {
    const requestSchema = join(directory, 'ServerRequest.json')
    const configuredBinary = process.env.CODEX_THREAD_STUDIO_CODEX_BIN?.trim()
    const codexBinary = configuredBinary || 'codex'
    const environment = configuredBinary
      ? { ...process.env }
      : { ...process.env, PATH: withoutNpmBinDirectories(process.env.PATH) }
    let result
    const diagnostics = []
    for (let attempt = 0; attempt < 3; attempt += 1) {
      result = spawnSync(codexBinary, ['app-server', 'generate-json-schema', '--experimental', '--out', directory], {
        encoding: 'utf8',
        env: environment,
        timeout: 30_000,
        windowsHide: true,
      })
      if (result.error?.code === 'ENOENT') {
        if (!configuredBinary) return context.skip('codex is not installed')
        assert.fail(`Configured Codex binary was not found: ${configuredBinary}`)
      }
      if (result.status === 0 && existsSync(requestSchema)) break
      diagnostics.push(await schemaAttemptDiagnostic(attempt + 1, result, directory))
      if (attempt < 2) await delay(150 * (2 ** attempt))
    }
    assert.equal(result.status, 0, [
      `Codex schema command failed for ${codexBinary}`,
      ...diagnostics,
    ].join('\n'))
    assert.ok(existsSync(requestSchema), [
      `Codex exited successfully without writing ${requestSchema}`,
      ...diagnostics,
    ].join('\n'))
    const requests = JSON.parse(await readFile(requestSchema, 'utf8'))
    const methods = requests.oneOf.map((entry) => entry.properties?.method?.enum?.[0]).filter(Boolean)
    assert.ok(methods.includes('item/tool/requestUserInput'))
    assert.ok(methods.includes('mcpServer/elicitation/request'))
    const initialize = JSON.parse(await readFile(join(directory, 'v1', 'InitializeParams.json'), 'utf8'))
    const capabilities = initialize.definitions.InitializeCapabilities.properties
    assert.equal(capabilities.experimentalApi.type, 'boolean')
    assert.ok(capabilities.extensions)
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
})

function withoutNpmBinDirectories(path = '') {
  return path
    .split(delimiter)
    .filter((entry) => !/[\\/]node_modules[\\/]\.bin[\\/]?$/i.test(entry))
    .join(delimiter)
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds))
}

async function schemaAttemptDiagnostic(attempt, result, directory) {
  const files = await readdir(directory).catch(() => [])
  const error = result.error ? `${result.error.code || result.error.name}: ${result.error.message}` : '(none)'
  const stderr = String(result.stderr || '').trim().slice(0, 2_000) || '(empty)'
  const stdout = String(result.stdout || '').trim().slice(0, 2_000) || '(empty)'
  return `attempt ${attempt}: status=${result.status} signal=${result.signal || 'none'} error=${error} files=${files.join(', ') || '(none)'} stdout=${stdout} stderr=${stderr}`
}
