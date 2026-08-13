import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'

test('installed Codex schema still exposes Studio structured interactions', async (context) => {
  const directory = await mkdtemp(join(tmpdir(), 'studio-app-server-schema-'))
  try {
    const result = spawnSync('codex', ['app-server', 'generate-json-schema', '--experimental', '--out', directory], { encoding: 'utf8' })
    if (result.error?.code === 'ENOENT') return context.skip('codex is not installed')
    assert.equal(result.status, 0, result.stderr)
    const requests = JSON.parse(await readFile(join(directory, 'ServerRequest.json'), 'utf8'))
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
