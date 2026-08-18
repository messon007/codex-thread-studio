const builtinDescriptors = [
  {
    id: 'codex',
    name: 'Codex',
    tag: 'CX',
    nativeLabel: 'CODEX NATIVE',
    binary: 'codex',
    infoPath: '/studio/codex',
    socketPath: '/ws/codex',
    protocol: 'Codex App Server v2',
    transport: 'stdio JSONL',
    kind: 'codex',
    adapter: 'codex-app-server',
  },
  {
    id: 'opencode',
    name: 'OpenCode',
    tag: 'OC',
    nativeLabel: 'OPENCODE NATIVE',
    binary: 'opencode',
    infoPath: '/studio/opencode',
    protocol: 'OpenCode Server API',
    transport: 'HTTP + SSE',
    kind: 'opencode',
    socketPath: '',
    adapter: 'opencode-server',
  },
]

const descriptors = new Map()

export const BACKEND_IDS = []
export const CODEX_BACKENDS = []

export function installBackendRegistry(entries = builtinDescriptors) {
  const next = normalizeDescriptors(entries)
  descriptors.clear()
  for (const descriptor of next) descriptors.set(descriptor.id, descriptor)
  BACKEND_IDS.splice(0, BACKEND_IDS.length, ...next.map(({ id }) => id))
  CODEX_BACKENDS.splice(0, CODEX_BACKENDS.length, ...next.filter(({ kind }) => kind === 'codex').map(({ id }) => id))
  return backendDescriptors()
}

export function backendDescriptors() {
  return BACKEND_IDS.map((id) => descriptors.get(id))
}

export function backendDescriptor(backend) {
  return descriptors.get(backend) || descriptors.get('codex')
}

export function isSupportedBackend(backend) {
  return descriptors.has(backend)
}

export function isCodexBackend(backend) {
  return descriptors.has(backend) && descriptors.get(backend)?.kind === 'codex'
}

export function defaultTurnOptions(backend) {
  return backend === 'codex' ? { effort: 'high' } : {}
}

export function emptyBackendCatalogs() {
  return Object.fromEntries(BACKEND_IDS.map((backend) => [backend, []]))
}

export function emptyBackendSelections() {
  return Object.fromEntries(BACKEND_IDS.map((backend) => [backend, null]))
}

export function backendSearchAliases(backend) {
  const descriptor = backendDescriptor(backend)
  return `${descriptor.tag.toLowerCase()} ${descriptor.name.toLowerCase()} ${backend}`
}

function normalizeDescriptors(entries) {
  const values = Array.isArray(entries) ? entries : []
  const normalized = []
  const ids = new Set()
  for (const entry of values) {
    const descriptor = normalizeDescriptor(entry)
    if (!descriptor || ids.has(descriptor.id)) continue
    ids.add(descriptor.id)
    normalized.push(descriptor)
  }
  for (const builtin of builtinDescriptors) {
    if (ids.has(builtin.id)) continue
    const descriptor = normalizeDescriptor(builtin)
    ids.add(descriptor.id)
    normalized.push(descriptor)
  }
  const codex = normalized.find((entry) => entry.id === 'codex')
  const opencode = normalized.find((entry) => entry.id === 'opencode')
  const configured = normalized.filter((entry) => !['codex', 'opencode'].includes(entry.id))
  return [codex, ...configured, opencode].filter(Boolean)
}

function normalizeDescriptor(value) {
  const id = String(value?.id || '').trim()
  const kind = value?.kind === 'opencode' ? 'opencode' : value?.kind === 'codex' ? 'codex' : ''
  if (!/^[a-z][a-z0-9_-]{0,63}$/u.test(id) || !kind) return null
  const name = String(value?.name || id).trim().slice(0, 64) || id
  const tag = String(value?.tag || name.slice(0, 2)).trim().toUpperCase().replace(/[^A-Z0-9]/gu, '').slice(0, 4) || 'AI'
  return Object.freeze({
    id,
    name,
    tag,
    nativeLabel: String(value?.nativeLabel || `${name.toUpperCase()} NATIVE`).slice(0, 96),
    binary: String(value?.binary || id).slice(0, 4096),
    infoPath: String(value?.infoPath || (kind === 'codex' ? `/studio/backend/${id}` : '/studio/opencode')),
    socketPath: String(value?.socketPath || (kind === 'codex' ? `/ws/codex/${id}` : '')),
    protocol: String(value?.protocol || (kind === 'codex' ? 'Codex App Server v2' : 'OpenCode Server API')),
    transport: String(value?.transport || (kind === 'codex' ? 'stdio JSONL' : 'HTTP + SSE')),
    kind,
    adapter: String(value?.adapter || (kind === 'codex' ? 'codex-app-server' : 'opencode-server')),
  })
}

installBackendRegistry()
