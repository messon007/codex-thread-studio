// Generated from ui-src; run npm run build:ui. Do not edit.
const CODEX_CATALOG_FIELDS = Object.freeze([
    'cwd',
    'name',
    'updatedAt',
    'recencyAt',
    'section',
    'sectionEnteredAt',
]);
export const CODEX_SUBAGENT_SOURCE_KINDS = Object.freeze([
    'subAgent',
    'subAgentReview',
    'subAgentCompact',
    'subAgentThreadSpawn',
    'subAgentOther',
]);
export function catalogListParams(kind, params = {}) {
    return kind === 'codex' ? { ...params, useStateDbOnly: true } : { ...params };
}
export function subagentCatalogListParams(ancestorThreadId, cursor = null, limit = 100) {
    const ancestor = typeof ancestorThreadId === 'string' ? ancestorThreadId.trim() : '';
    if (!ancestor)
        throw new Error('A root thread ID is required to list subagents.');
    const pageSize = Number.isFinite(limit) ? Math.min(Math.max(Math.trunc(limit), 1), 100) : 100;
    return {
        ancestorThreadId: ancestor,
        cursor: typeof cursor === 'string' && cursor ? cursor : null,
        limit: pageSize,
        sourceKinds: [...CODEX_SUBAGENT_SOURCE_KINDS],
    };
}
export function orderAgentThreadTree(root, descendants = []) {
    const rootId = String(root?.id || '');
    if (!rootId)
        return [];
    const byId = new Map([[rootId, root]]);
    for (const thread of descendants) {
        const id = String(thread?.id || '');
        if (id && id !== rootId)
            byId.set(id, thread);
    }
    const children = new Map();
    const orphans = [];
    for (const [id, thread] of byId) {
        if (id === rootId)
            continue;
        const parentId = String(thread.parentThreadId || '');
        if (!parentId || parentId === id || !byId.has(parentId)) {
            orphans.push(thread);
            continue;
        }
        const siblings = children.get(parentId) || [];
        siblings.push(thread);
        children.set(parentId, siblings);
    }
    const compare = (left, right) => {
        const leftCreated = Number(left.createdAt || 0);
        const rightCreated = Number(right.createdAt || 0);
        return leftCreated - rightCreated || String(left.id || '').localeCompare(String(right.id || ''));
    };
    for (const siblings of children.values())
        siblings.sort(compare);
    orphans.sort(compare);
    const result = [];
    const visited = new Set();
    const append = (thread, depth) => {
        const id = String(thread.id || '');
        if (!id || visited.has(id))
            return;
        visited.add(id);
        result.push({ thread, depth });
        for (const child of children.get(id) || [])
            append(child, depth + 1);
    };
    append(root, 0);
    for (const orphan of orphans)
        append(orphan, 1);
    for (const [id, thread] of byId)
        if (!visited.has(id))
            append(thread, 1);
    return result;
}
export function preserveSelectedSubagent(catalog, previous, selectedId) {
    const listed = Array.isArray(catalog) ? [...catalog] : [];
    const selected = typeof selectedId === 'string' ? selectedId : '';
    if (!selected || listed.some((thread) => thread.id === selected))
        return listed;
    const retained = (Array.isArray(previous) ? previous : []).find((thread) => thread.id === selected && thread.parentThreadId);
    return retained ? [{ ...retained, turns: undefined }, ...listed] : listed;
}
export function shouldRecoverCodexCatalog(catalog, preferredId) {
    const threads = Array.isArray(catalog) ? catalog : [];
    if (!threads.length)
        return true;
    const preferred = typeof preferredId === 'string' ? preferredId : '';
    return Boolean(preferred) && !threads.some((thread) => String(thread?.id || '') === preferred);
}
export function reconcileStartedThreadCatalog(catalog, startedThreads = []) {
    const listed = Array.isArray(catalog) ? [...catalog] : [];
    const listedIds = new Set(listed.map((thread) => String(thread?.id || '')).filter(Boolean));
    const retainedById = new Map();
    const confirmedIds = [];
    for (const thread of Array.isArray(startedThreads) ? startedThreads : []) {
        const id = String(thread?.id || '');
        if (!id)
            continue;
        if (listedIds.has(id)) {
            confirmedIds.push(id);
            continue;
        }
        retainedById.set(id, { ...thread, turns: undefined });
    }
    const retained = [...retainedById.values()].reverse();
    return {
        threads: [...retained, ...listed],
        retainedIds: retained.map((thread) => thread.id),
        confirmedIds: [...new Set(confirmedIds)],
    };
}
export function mergeCatalogMetadata(kind, current, incoming) {
    const merged = { ...(current || {}), ...(incoming || {}) };
    if (kind !== 'codex' || !current)
        return merged;
    for (const field of CODEX_CATALOG_FIELDS) {
        if (Object.hasOwn(current, field))
            merged[field] = current[field];
    }
    return merged;
}
export function turnStartParams(kind, thread, params = {}) {
    if (kind !== 'codex' || !thread?.cwd)
        return { ...params };
    return { ...params, cwd: thread.cwd };
}
