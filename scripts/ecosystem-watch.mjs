import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const DEFAULT_BASELINE = '.github/upstream-watch.json'
const ISSUE_TITLE = 'Weekly ecosystem watch: review required'
const ISSUE_MARKER = '<!-- codex-thread-studio-ecosystem-watch -->'
const HTTP_METHODS = new Set(['delete', 'get', 'patch', 'post', 'put'])
const IGNORED_SCHEMA_KEYS = new Set([
  '$schema',
  'description',
  'example',
  'examples',
  'externalDocs',
  'operationId',
  'summary',
  'tags',
  'title',
  'x-codeSamples',
])
const NAMED_SCHEMA_MAP_KEYS = new Set(['$defs', 'definitions', 'dependentSchemas', 'patternProperties', 'properties'])

export function stableValue(value, preserveEntryNames = false) {
  if (Array.isArray(value)) return value.map((entry) => stableValue(entry))
  if (!value || typeof value !== 'object') return value
  if (preserveEntryNames) {
    return Object.fromEntries(Object.keys(value)
      .sort()
      .map((key) => [key, stableValue(value[key])]))
  }
  return Object.fromEntries(Object.keys(value)
    .filter((key) => !IGNORED_SCHEMA_KEYS.has(key))
    .sort()
    .map((key) => [key, stableValue(value[key], NAMED_SCHEMA_MAP_KEYS.has(key))]))
}

export function stableJson(value) {
  return JSON.stringify(stableValue(value))
}

export function sha256(value) {
  return createHash('sha256').update(stableJson(value)).digest('hex')
}

function jsonPointer(root, reference) {
  if (!reference.startsWith('#/')) return null
  return reference.slice(2).split('/').reduce((value, part) => value?.[part.replaceAll('~1', '/').replaceAll('~0', '~')], root)
}

export function resolveLocalRefs(value, root, stack = new Set()) {
  if (Array.isArray(value)) return value.map((entry) => resolveLocalRefs(entry, root, stack))
  if (!value || typeof value !== 'object') return value
  if (typeof value.$ref === 'string' && value.$ref.startsWith('#/')) {
    if (stack.has(value.$ref)) return { $ref: value.$ref }
    const target = jsonPointer(root, value.$ref)
    if (target == null) return value
    const nextStack = new Set(stack).add(value.$ref)
    const siblings = Object.fromEntries(Object.entries(value).filter(([key]) => key !== '$ref'))
    return resolveLocalRefs({ ...target, ...siblings }, root, nextStack)
  }
  return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, resolveLocalRefs(entry, root, stack)]))
}

function visitObjects(value, visitor) {
  if (Array.isArray(value)) {
    for (const entry of value) visitObjects(entry, visitor)
    return
  }
  if (!value || typeof value !== 'object') return
  visitor(value)
  for (const entry of Object.values(value)) visitObjects(entry, visitor)
}

function uniqueSchemas(values) {
  const schemas = new Map()
  for (const value of values) schemas.set(stableJson(value), value)
  return [...schemas.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([, value]) => value)
}

export function codexMethodProjection(documents, requirements) {
  const projection = {}
  const missing = []
  for (const [documentName, methods] of Object.entries(requirements)) {
    const root = documents[documentName]
    if (!root) {
      missing.push(`${documentName}: schema document missing`)
      continue
    }
    const byMethod = new Map(methods.map((method) => [method, []]))
    visitObjects(root, (value) => {
      const method = value.properties?.method?.enum
      if (Array.isArray(method) && method.length === 1 && byMethod.has(method[0])) {
        byMethod.get(method[0]).push(resolveLocalRefs(value, root))
      }
    })
    projection[documentName] = {}
    for (const method of methods) {
      const schemas = uniqueSchemas(byMethod.get(method))
      if (!schemas.length) missing.push(`${documentName}: ${method}`)
      else projection[documentName][method] = schemas
    }
  }
  return { projection, missing }
}

export function openCodeContractProjection(specification, requirements) {
  const projection = { operations: {}, events: {} }
  const missing = []
  for (const operation of requirements.operations) {
    const method = operation.method.toLowerCase()
    const key = `${operation.method.toUpperCase()} ${operation.path}`
    const schema = specification.paths?.[operation.path]?.[method]
    if (!schema || !HTTP_METHODS.has(method)) missing.push(key)
    else projection.operations[key] = resolveLocalRefs(schema, specification)
  }

  const eventSchemas = new Map(requirements.events.map((event) => [event, []]))
  visitObjects(specification.components?.schemas || {}, (value) => {
    const type = value.properties?.type?.enum
    if (Array.isArray(type) && type.length === 1 && eventSchemas.has(type[0])) {
      eventSchemas.get(type[0]).push(resolveLocalRefs(value, specification))
    }
  })
  for (const event of requirements.events) {
    const schemas = uniqueSchemas(eventSchemas.get(event))
    if (!schemas.length) missing.push(`event: ${event}`)
    else projection.events[event] = schemas
  }
  return { projection, missing }
}

function parseCliVersion(output, label) {
  const matches = String(output || '').match(/\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?/gu)
  if (!matches?.length) throw new Error(`${label} did not report a semantic version`)
  return matches.at(-1)
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    timeout: 120_000,
    windowsHide: true,
    ...options,
  })
  if (result.status !== 0) {
    const detail = String(result.stderr || result.stdout || result.error?.message || '').trim().slice(0, 4_000)
    throw new Error(`${command} ${args.join(' ')} failed${result.status == null ? '' : ` (${result.status})`}: ${detail || 'no diagnostics'}`)
  }
  return result
}

function upstreamResult(baseline, version, projection, missing) {
  const contractSha256 = sha256(projection)
  return {
    status: missing.length ? 'incompatible' : 'compatible',
    acceptedVersion: baseline.acceptedVersion,
    observedVersion: version,
    versionChanged: version !== baseline.acceptedVersion,
    acceptedContractSha256: baseline.acceptedContractSha256,
    contractSha256,
    contractChanged: contractSha256 !== baseline.acceptedContractSha256,
    missing,
    releaseUrl: baseline.releaseUrl,
  }
}

function failedUpstreamResult(baseline, error) {
  return {
    status: 'probe-failed',
    acceptedVersion: baseline.acceptedVersion,
    observedVersion: null,
    versionChanged: false,
    acceptedContractSha256: baseline.acceptedContractSha256,
    contractSha256: null,
    contractChanged: false,
    missing: [],
    error: error?.message || String(error),
    releaseUrl: baseline.releaseUrl,
  }
}

export function stripNpmBinDirectories(pathValue, separator = delimiter) {
  return String(pathValue || '').split(separator)
    .filter((entry) => !/[\\/]node_modules[\\/]\.bin[\\/]?$/iu.test(entry))
    .join(separator)
}

async function isolatedEnvironment(outputDirectory, name) {
  const home = join(outputDirectory, `${name}-home`)
  await mkdir(home, { recursive: true })
  return {
    ...process.env,
    HOME: home,
    CODEX_HOME: join(home, '.codex'),
    XDG_CACHE_HOME: join(home, '.cache'),
    XDG_CONFIG_HOME: join(home, '.config'),
    XDG_DATA_HOME: join(home, '.local', 'share'),
    PATH: stripNpmBinDirectories(process.env.PATH),
  }
}

async function probeCodex(binary, baseline, requirements, outputDirectory) {
  try {
    const environment = await isolatedEnvironment(outputDirectory, 'codex')
    const versionResult = run(binary, ['--version'], { env: environment })
    const version = parseCliVersion(`${versionResult.stdout}\n${versionResult.stderr}`, 'Codex')
    const schemaDirectory = join(outputDirectory, 'codex-schema')
    await mkdir(schemaDirectory, { recursive: true })
    run(binary, ['app-server', 'generate-json-schema', '--experimental', '--out', schemaDirectory], { env: environment })
    const files = {
      clientRequests: 'ClientRequest.json',
      serverRequests: 'ServerRequest.json',
      serverNotifications: 'ServerNotification.json',
    }
    const documents = {}
    for (const [name, file] of Object.entries(files)) {
      documents[name] = JSON.parse(await readFile(join(schemaDirectory, file), 'utf8'))
    }
    const contract = codexMethodProjection(documents, requirements)
    await writeFile(join(outputDirectory, 'codex-contract.json'), `${JSON.stringify(stableValue(contract.projection), null, 2)}\n`)
    return upstreamResult(baseline, version, contract.projection, contract.missing)
  } catch (error) {
    return failedUpstreamResult(baseline, error)
  }
}

async function probeOpenCode(binary, baseline, requirements, outputDirectory) {
  try {
    const environment = await isolatedEnvironment(outputDirectory, 'opencode')
    const versionResult = run(binary, ['--version'], { env: environment })
    const version = parseCliVersion(`${versionResult.stdout}\n${versionResult.stderr}`, 'OpenCode')
    const generated = run(binary, ['generate', '--pure'], { env: environment })
    const specification = JSON.parse(generated.stdout)
    const contract = openCodeContractProjection(specification, requirements)
    await writeFile(join(outputDirectory, 'opencode-contract.json'), `${JSON.stringify(stableValue(contract.projection), null, 2)}\n`)
    return upstreamResult(baseline, version, contract.projection, contract.missing)
  } catch (error) {
    return failedUpstreamResult(baseline, error)
  }
}

export function reportNeedsReview(report, health = {}) {
  const unhealthy = Object.values(health).some((result) => result && !['success', 'skipped'].includes(result))
  return unhealthy || Object.values(report?.upstreams || {}).some((upstream) => upstream.status !== 'compatible'
    || upstream.versionChanged || upstream.contractChanged)
}

export async function probeUpstreams({ baselinePath, outputDirectory, codexBinary = 'codex', openCodeBinary = 'opencode' }) {
  const baseline = JSON.parse(await readFile(baselinePath, 'utf8'))
  await mkdir(outputDirectory, { recursive: true })
  const [codex, opencode] = await Promise.all([
    probeCodex(codexBinary, baseline.upstreams.codex, baseline.contracts.codex, outputDirectory),
    probeOpenCode(openCodeBinary, baseline.upstreams.opencode, baseline.contracts.opencode, outputDirectory),
  ])
  const report = {
    schemaVersion: 1,
    checkedAt: new Date().toISOString(),
    baseline: baselinePath,
    upstreams: { codex, opencode },
  }
  report.needsReview = reportNeedsReview(report)
  await writeFile(join(outputDirectory, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
  await writeFile(join(outputDirectory, 'report.md'), renderReport(report))
  return report
}

function statusText(upstream) {
  if (upstream.status === 'probe-failed') return 'Probe failed'
  if (upstream.status === 'incompatible') return 'Required contract missing'
  if (upstream.contractChanged) return 'Compatible; used contract changed'
  if (upstream.versionChanged) return 'Compatible; version changed'
  return 'No change'
}

function markdown(value) {
  return String(value ?? '—').replaceAll('|', '\\|').replaceAll('\n', ' ')
}

export function renderReport(report, health = {}) {
  const runUrl = process.env.GITHUB_SERVER_URL && process.env.GITHUB_REPOSITORY && process.env.GITHUB_RUN_ID
    ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
    : ''
  const needsReview = reportNeedsReview(report, health)
  const lines = [
    `${ISSUE_MARKER}`,
    '# Weekly ecosystem watch',
    '',
  ]
  if (needsReview) {
    lines.push('## Decision requested', '')
    lines.push('- [ ] Decide whether to update the installed Codex and/or OpenCode runtime.')
    lines.push('- [ ] If a used contract changed, decide whether the Studio adapter, fixtures, or documentation must change before upgrading.')
    lines.push('- [ ] After validation, record the accepted versions with `npm run monitor:upstreams -- accept` and commit the baseline.', '')
  } else {
    lines.push('## Result', '', 'No action is required; upstream contracts and repository health match the reviewed state.', '')
  }
  lines.push(
    '| Upstream | Reviewed | Observed | Studio-used contract | Result |',
    '| --- | --- | --- | --- | --- |',
  )
  for (const [key, upstream] of Object.entries(report?.upstreams || {})) {
    const name = key === 'codex' ? 'Codex CLI' : key === 'opencode' ? 'OpenCode' : key
    const nameCell = upstream.releaseUrl ? `[${name}](${upstream.releaseUrl})` : name
    const contract = upstream.contractSha256
      ? `${upstream.acceptedContractSha256?.slice(0, 12) || 'none'} → ${upstream.contractSha256.slice(0, 12)}`
      : 'not generated'
    lines.push(`| ${nameCell} | ${markdown(upstream.acceptedVersion)} | ${markdown(upstream.observedVersion)} | ${markdown(contract)} | ${markdown(statusText(upstream))} |`)
  }
  lines.push('')
  for (const [key, upstream] of Object.entries(report?.upstreams || {})) {
    if (!upstream.error && !upstream.missing?.length) continue
    lines.push(`### ${key === 'codex' ? 'Codex CLI' : 'OpenCode'} diagnostics`, '')
    if (upstream.error) lines.push(`- Probe error: ${upstream.error}`)
    for (const missing of upstream.missing || []) lines.push(`- Missing: \`${missing}\``)
    lines.push('')
  }
  if (Object.keys(health).length) {
    lines.push('## Repository health', '')
    for (const [name, result] of Object.entries(health)) lines.push(`- ${name}: **${result || 'unknown'}**`)
    lines.push('')
  }
  lines.push('## Interpretation', '')
  lines.push('- Version changed, contract unchanged: runtime upgrade is a candidate; still run a normal session smoke test.')
  lines.push('- Contract changed, requirements present: the used API shape changed but the required surface still exists; inspect the attached projections before deciding whether Studio code needs adjustment.')
  lines.push('- Required contract missing or probe failed: do not treat the new runtime as compatible until the adapter or the probe is fixed.')
  lines.push('')
  if (runUrl) lines.push(`Workflow run: ${runUrl}`, '')
  lines.push(`Checked: ${report?.checkedAt || new Date().toISOString()}`, '')
  return `${lines.join('\n')}\n`
}

async function githubRequest(path, options = {}) {
  const token = process.env.GITHUB_TOKEN
  const repository = process.env.GITHUB_REPOSITORY
  if (!token || !repository) throw new Error('GITHUB_TOKEN and GITHUB_REPOSITORY are required')
  const response = await fetch(`https://api.github.com/repos/${repository}${path}`, {
    ...options,
    headers: {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${token}`,
      'X-GitHub-Api-Version': '2022-11-28',
      ...(options.headers || {}),
    },
  })
  const text = await response.text()
  const value = text ? JSON.parse(text) : null
  if (!response.ok) throw new Error(`GitHub API ${response.status}: ${value?.message || text}`)
  return value
}

export async function synchronizeIssue(report, health = {}) {
  const issues = await githubRequest('/issues?state=open&per_page=100')
  const existing = issues.find((issue) => !issue.pull_request && (issue.title === ISSUE_TITLE || issue.body?.includes(ISSUE_MARKER)))
  const needsReview = reportNeedsReview(report, health)
  const body = renderReport(report, health)
  if (needsReview && existing) {
    await githubRequest(`/issues/${existing.number}`, { method: 'PATCH', body: JSON.stringify({ title: ISSUE_TITLE, body }) })
    return { action: 'updated', number: existing.number }
  }
  if (needsReview) {
    const created = await githubRequest('/issues', { method: 'POST', body: JSON.stringify({ title: ISSUE_TITLE, body }) })
    return { action: 'created', number: created.number }
  }
  if (existing) {
    await githubRequest(`/issues/${existing.number}`, { method: 'PATCH', body: JSON.stringify({ body, state: 'closed', state_reason: 'completed' }) })
    return { action: 'closed', number: existing.number }
  }
  return { action: 'none' }
}

function parseArguments(arguments_) {
  const command = arguments_[0] && !arguments_[0].startsWith('--') ? arguments_[0] : 'probe'
  if (!['accept', 'notify', 'probe'].includes(command)) throw new Error(`Unknown command: ${command}`)
  const values = {}
  for (let index = command === arguments_[0] ? 1 : 0; index < arguments_.length; index += 1) {
    const key = arguments_[index]
    if (!key.startsWith('--')) throw new Error(`Unexpected argument: ${key}`)
    const value = arguments_[index + 1]
    if (!value || value.startsWith('--')) throw new Error(`Missing value for ${key}`)
    values[key.slice(2)] = value
    index += 1
  }
  return { command, values }
}

async function acceptBaseline(options) {
  const report = await probeUpstreams(options)
  const failed = Object.entries(report.upstreams).filter(([, upstream]) => upstream.status !== 'compatible')
  if (failed.length) throw new Error(`Cannot accept an incompatible probe: ${failed.map(([name]) => name).join(', ')}`)
  const baseline = JSON.parse(await readFile(options.baselinePath, 'utf8'))
  const acceptedAt = new Date().toISOString()
  for (const [name, upstream] of Object.entries(report.upstreams)) {
    baseline.upstreams[name].acceptedVersion = upstream.observedVersion
    baseline.upstreams[name].acceptedContractSha256 = upstream.contractSha256
    baseline.upstreams[name].acceptedAt = acceptedAt
    upstream.acceptedVersion = upstream.observedVersion
    upstream.acceptedContractSha256 = upstream.contractSha256
    upstream.versionChanged = false
    upstream.contractChanged = false
  }
  await writeFile(options.baselinePath, `${JSON.stringify(baseline, null, 2)}\n`)
  report.acceptedAt = acceptedAt
  report.needsReview = false
  await writeFile(join(options.outputDirectory, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
  await writeFile(join(options.outputDirectory, 'report.md'), renderReport(report))
  return report
}

async function main() {
  const { command, values } = parseArguments(process.argv.slice(2))
  const baselinePath = resolve(values.baseline || DEFAULT_BASELINE)
  if (command === 'notify') {
    let report
    try {
      report = JSON.parse(await readFile(resolve(values.report || 'upstream-watch/report.json'), 'utf8'))
    } catch (error) {
      report = {
        schemaVersion: 1,
        checkedAt: new Date().toISOString(),
        upstreams: {
          monitor: failedUpstreamResult({ acceptedVersion: 'unknown', acceptedContractSha256: '', releaseUrl: '' }, error),
        },
      }
    }
    const health = {
      'Upstream compatibility job': values['upstream-result'],
      'Project tests and security audits': values['health-result'],
    }
    const result = await synchronizeIssue(report, health)
    console.log(`Ecosystem watch issue: ${result.action}${result.number ? ` #${result.number}` : ''}`)
    return
  }
  const outputDirectory = resolve(values.output || await mkdtemp(join(tmpdir(), 'codex-thread-studio-watch-')))
  const options = {
    baselinePath,
    outputDirectory,
    codexBinary: values['codex-bin'] || process.env.CODEX_THREAD_STUDIO_CODEX_BIN || 'codex',
    openCodeBinary: values['opencode-bin'] || process.env.CODEX_THREAD_STUDIO_OPENCODE_BIN || 'opencode',
  }
  const report = command === 'accept' ? await acceptBaseline(options) : await probeUpstreams(options)
  if (command === 'accept') console.log(`Accepted current upstream baseline at ${report.acceptedAt}`)
  console.log(renderReport(report))
  console.log(`Detailed report: ${join(outputDirectory, 'report.json')}`)
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : ''
if (invokedPath === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.stack || error.message || String(error))
    process.exitCode = 1
  })
}
