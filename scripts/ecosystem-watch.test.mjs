import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  codexMethodProjection,
  openCodeContractProjection,
  renderReport,
  reportNeedsReview,
  sha256,
  stableValue,
  stripNpmBinDirectories,
  synchronizeIssue,
} from './ecosystem-watch.mjs'

test('npm-injected ancestor bins cannot shadow installed upstream CLIs', () => {
  assert.equal(
    stripNpmBinDirectories('/project/node_modules/.bin:/home/user/node_modules/.bin:/usr/local/bin:/usr/bin', ':'),
    '/usr/local/bin:/usr/bin',
  )
  assert.equal(
    stripNpmBinDirectories('C:\\project\\node_modules\\.bin;C:\\Tools;C:\\Program Files\\node_modules\\.bin\\', ';'),
    'C:\\Tools',
  )
})

test('stable schema values ignore prose metadata without dropping named properties', () => {
  const schema = {
    title: 'Request prose title',
    description: 'Request prose description',
    type: 'object',
    properties: {
      title: { type: 'string', description: 'Field prose', example: 'A title' },
      description: { type: 'number' },
    },
  }
  const proseChanged = structuredClone(schema)
  proseChanged.title = 'Reworded title'
  proseChanged.description = 'Reworded description'
  proseChanged.properties.title.description = 'Reworded field prose'
  proseChanged.properties.title.example = 'A different example'
  assert.equal(sha256(schema), sha256(proseChanged))
  assert.deepEqual(Object.keys(stableValue(schema).properties), ['description', 'title'])

  const fieldChanged = structuredClone(schema)
  fieldChanged.properties.title.type = 'boolean'
  assert.notEqual(sha256(schema), sha256(fieldChanged))
})

test('Codex projection follows only required method references', () => {
  const document = {
    definitions: {
      ReadParams: { type: 'object', properties: { threadId: { type: 'string' } }, required: ['threadId'] },
      Unused: { type: 'object', properties: { changed: { type: 'boolean' } } },
      ReadRequest: {
        type: 'object',
        properties: {
          method: { type: 'string', enum: ['thread/read'] },
          params: { $ref: '#/definitions/ReadParams' },
        },
      },
    },
  }
  const result = codexMethodProjection({ clientRequests: document }, { clientRequests: ['thread/read', 'turn/start'] })
  assert.deepEqual(result.missing, ['clientRequests: turn/start'])
  assert.equal(result.projection.clientRequests['thread/read'][0].properties.params.properties.threadId.type, 'string')
  const before = sha256(result.projection)
  document.definitions.Unused.properties.changed.type = 'string'
  assert.equal(sha256(codexMethodProjection({ clientRequests: document }, { clientRequests: ['thread/read'] }).projection), before)
})

test('OpenCode projection validates required operations and events', () => {
  const specification = {
    paths: {
      '/session/{sessionID}': {
        get: { responses: { 200: { content: { 'application/json': { schema: { $ref: '#/components/schemas/Session' } } } } } },
      },
    },
    components: {
      schemas: {
        Session: { type: 'object', properties: { id: { type: 'string' } }, required: ['id'] },
        EventSessionIdle: {
          type: 'object',
          properties: { type: { type: 'string', enum: ['session.idle'] } },
          required: ['type'],
        },
      },
    },
  }
  const result = openCodeContractProjection(specification, {
    operations: [
      { method: 'GET', path: '/session/{sessionID}' },
      { method: 'POST', path: '/session/{sessionID}/abort' },
    ],
    events: ['session.idle', 'message.updated'],
  })
  assert.deepEqual(result.missing, ['POST /session/{sessionID}/abort', 'event: message.updated'])
  assert.equal(result.projection.operations['GET /session/{sessionID}'].responses[200].content['application/json'].schema.properties.id.type, 'string')
})

test('report distinguishes version drift, contract drift, and repository health', () => {
  const report = {
    checkedAt: '2026-09-02T00:00:00.000Z',
    upstreams: {
      codex: {
        status: 'compatible',
        acceptedVersion: '1.0.0',
        observedVersion: '1.1.0',
        versionChanged: true,
        acceptedContractSha256: 'a'.repeat(64),
        contractSha256: 'a'.repeat(64),
        contractChanged: false,
        missing: [],
        releaseUrl: 'https://example.com/codex',
      },
    },
  }
  assert.equal(reportNeedsReview(report), true)
  assert.equal(reportNeedsReview({ upstreams: { codex: { ...report.upstreams.codex, versionChanged: false } } }), false)
  assert.equal(reportNeedsReview({ upstreams: {} }, { tests: 'failure' }), true)
  const markdown = renderReport(report, { tests: 'success' })
  assert.match(markdown, /Decision requested/u)
  assert.match(markdown, /1\.0\.0 \| 1\.1\.0/u)
  assert.match(markdown, /tests: \*\*success\*\*/u)
  const healthyMarkdown = renderReport({
    ...report,
    upstreams: { codex: { ...report.upstreams.codex, observedVersion: '1.0.0', versionChanged: false } },
  }, { tests: 'success' })
  assert.match(healthyMarkdown, /No action is required/u)
  assert.doesNotMatch(healthyMarkdown, /Decision requested/u)
})

test('notification maintains one Issue and closes it after recovery', async (context) => {
  const originalFetch = globalThis.fetch
  const originalRepository = process.env.GITHUB_REPOSITORY
  const originalToken = process.env.GITHUB_TOKEN
  const requests = []
  globalThis.fetch = async (url, options = {}) => {
    requests.push({ url, options })
    if (String(url).includes('/issues?')) {
      return new Response(JSON.stringify([{
        number: 17,
        title: 'Weekly ecosystem watch: review required',
        body: '<!-- codex-thread-studio-ecosystem-watch -->',
      }]), { status: 200 })
    }
    return new Response(JSON.stringify({ number: 17 }), { status: 200 })
  }
  process.env.GITHUB_REPOSITORY = 'example/studio'
  process.env.GITHUB_TOKEN = 'test-token'
  context.after(() => {
    globalThis.fetch = originalFetch
    if (originalRepository == null) delete process.env.GITHUB_REPOSITORY
    else process.env.GITHUB_REPOSITORY = originalRepository
    if (originalToken == null) delete process.env.GITHUB_TOKEN
    else process.env.GITHUB_TOKEN = originalToken
  })

  const changed = {
    checkedAt: '2026-09-02T00:00:00.000Z',
    upstreams: {
      codex: {
        status: 'compatible',
        acceptedVersion: '1.0.0',
        observedVersion: '1.1.0',
        versionChanged: true,
        acceptedContractSha256: 'a'.repeat(64),
        contractSha256: 'a'.repeat(64),
        contractChanged: false,
        missing: [],
        releaseUrl: 'https://example.com/codex',
      },
    },
  }
  assert.deepEqual(await synchronizeIssue(changed), { action: 'updated', number: 17 })
  assert.equal(requests[1].options.method, 'PATCH')
  assert.equal(JSON.parse(requests[1].options.body).state, undefined)

  requests.length = 0
  const healthy = {
    ...changed,
    upstreams: {
      codex: { ...changed.upstreams.codex, observedVersion: '1.0.0', versionChanged: false },
    },
  }
  assert.deepEqual(await synchronizeIssue(healthy, {
    'Upstream compatibility job': 'success',
    'Project tests and security audits': 'success',
  }), { action: 'closed', number: 17 })
  const closeBody = JSON.parse(requests[1].options.body)
  assert.equal(closeBody.state, 'closed')
  assert.equal(closeBody.state_reason, 'completed')
})

test('the baseline covers literal RPC methods used by production UI modules', async () => {
  const files = [
    '../ui/app.js',
    '../ui/session-management.mjs',
    '../ui/session-map-controller.mjs',
    '../ui/thread-router-controller.mjs',
  ]
  const source = (await Promise.all(files.map((file) => readFile(new URL(file, import.meta.url), 'utf8')))).join('\n')
  const baseline = JSON.parse(await readFile(new URL('../.github/upstream-watch.json', import.meta.url), 'utf8'))
  const configured = new Set(baseline.contracts.codex.clientRequests)
  const used = new Set()
  for (const match of source.matchAll(/\b(?:rpc|request)\(\s*['"]([^'"]+)['"]/gu)) used.add(match[1])
  for (const match of source.matchAll(/\b(?:dispatchBackendRpc|dispatchRpc|requestCodexBackend)\([^,\n]+,\s*['"]([^'"]+)['"]/gu)) used.add(match[1])
  assert.deepEqual([...used].filter((method) => !configured.has(method)).sort(), [])
})
