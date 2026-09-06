// Generated from ui-src; run npm run build:ui. Do not edit.
const CODEX_CATALOG_FIELDS = Object.freeze([
    'cwd',
    'name',
    'updatedAt',
    'recencyAt',
    'section',
    'sectionEnteredAt',
]);
export function catalogListParams(kind, params = {}) {
    return kind === 'codex' ? { ...params, useStateDbOnly: true } : { ...params };
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
