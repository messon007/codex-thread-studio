// Generated from ui-src; run npm run build:ui. Do not edit.
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
];
const descriptors = new Map();
export const BACKEND_IDS = [];
export const CODEX_BACKENDS = [];
export function installBackendRegistry(entries = builtinDescriptors) {
    const next = normalizeDescriptors(entries);
    descriptors.clear();
    for (const descriptor of next)
        descriptors.set(descriptor.id, descriptor);
    BACKEND_IDS.splice(0, BACKEND_IDS.length, ...next.map(({ id }) => id));
    CODEX_BACKENDS.splice(0, CODEX_BACKENDS.length, ...next.filter(({ kind }) => kind === 'codex').map(({ id }) => id));
    return backendDescriptors();
}
export function backendDescriptors() {
    return BACKEND_IDS.map((id) => descriptors.get(id));
}
export function backendDescriptor(backend) {
    const descriptor = descriptors.get(backend) || descriptors.get('codex');
    if (!descriptor)
        throw new Error('Backend registry has no Codex descriptor');
    return descriptor;
}
export function isSupportedBackend(backend) {
    return descriptors.has(backend);
}
export function isCodexBackend(backend) {
    return descriptors.has(backend) && descriptors.get(backend)?.kind === 'codex';
}
export function defaultTurnOptions(backend) {
    return backend === 'codex' ? { effort: 'high' } : {};
}
export function emptyBackendCatalogs() {
    return Object.fromEntries(BACKEND_IDS.map((backend) => [backend, []]));
}
export function emptyBackendSelections() {
    return Object.fromEntries(BACKEND_IDS.map((backend) => [backend, null]));
}
export function backendSearchAliases(backend) {
    const descriptor = backendDescriptor(backend);
    return `${descriptor.tag.toLowerCase()} ${descriptor.name.toLowerCase()} ${backend}`;
}
function normalizeDescriptors(entries) {
    const values = Array.isArray(entries) ? entries : [];
    const normalized = [];
    const ids = new Set();
    for (const entry of values) {
        const descriptor = normalizeDescriptor(entry);
        if (!descriptor || ids.has(descriptor.id))
            continue;
        ids.add(descriptor.id);
        normalized.push(descriptor);
    }
    for (const builtin of builtinDescriptors) {
        if (ids.has(builtin.id))
            continue;
        const descriptor = normalizeDescriptor(builtin);
        if (!descriptor)
            throw new Error('Invalid built-in backend descriptor');
        ids.add(descriptor.id);
        normalized.push(descriptor);
    }
    const codex = normalized.find((entry) => entry.id === 'codex');
    const opencode = normalized.find((entry) => entry.id === 'opencode');
    const configured = normalized.filter((entry) => !['codex', 'opencode'].includes(entry.id));
    return [codex, ...configured, opencode].filter((entry) => Boolean(entry));
}
function normalizeDescriptor(value) {
    const record = isRecord(value) ? value : {};
    const id = String(record.id || '').trim();
    const kind = record.kind === 'opencode' ? 'opencode' : record.kind === 'codex' ? 'codex' : '';
    if (!/^[a-z][a-z0-9_-]{0,63}$/u.test(id) || !kind)
        return null;
    const name = String(record.name || id).trim().slice(0, 64) || id;
    const tag = String(record.tag || name.slice(0, 2)).trim().toUpperCase().replace(/[^A-Z0-9]/gu, '').slice(0, 4) || 'AI';
    return Object.freeze({
        id,
        name,
        tag,
        nativeLabel: String(record.nativeLabel || `${name.toUpperCase()} NATIVE`).slice(0, 96),
        binary: String(record.binary || id).slice(0, 4096),
        infoPath: String(record.infoPath || (kind === 'codex' ? `/studio/backend/${id}` : '/studio/opencode')),
        socketPath: String(record.socketPath || (kind === 'codex' ? `/ws/codex/${id}` : '')),
        protocol: String(record.protocol || (kind === 'codex' ? 'Codex App Server v2' : 'OpenCode Server API')),
        transport: String(record.transport || (kind === 'codex' ? 'stdio JSONL' : 'HTTP + SSE')),
        kind,
        adapter: String(record.adapter || (kind === 'codex' ? 'codex-app-server' : 'opencode-server')),
    });
}
function isRecord(value) {
    return !!value && typeof value === 'object';
}
installBackendRegistry();
