// Generated from ui-src; run npm run build:ui. Do not edit.
export function cachedSession(cache, backend, id) {
    return id ? cache.get(`${backend}:${id}`) || null : null;
}
export function storeCachedSession(cache, backend, id, model, historyEpoch, now) {
    if (!id || !model || model.threadId !== id)
        return false;
    const key = `${backend}:${id}`;
    const existing = cache.get(key);
    cache.set(key, {
        ...(existing || {}), model, validatedAt: now,
        ...(backend === 'opencode' ? { historyEpoch } : {}),
    });
    return true;
}
export function validateCachedModel(cache, backend, model, openCodeHistoryEpoch, now) {
    for (const [key, cached] of cache) {
        if (key.startsWith(`${backend}:`) && cached.model === model) {
            cached.validatedAt = now;
            // A live delta cannot certify events missed before reconnect.
            if (backend === 'opencode' && cached.historyEpoch !== openCodeHistoryEpoch)
                return;
            if (backend === 'opencode')
                cached.historyEpoch = openCodeHistoryEpoch;
            return;
        }
    }
}
export function unvalidateCachedModel(cache, backend, model) {
    for (const [key, cached] of cache) {
        if (key.startsWith(`${backend}:`) && cached.model === model) {
            cached.validatedAt = 0;
            return;
        }
    }
}
export function cachedModelThreadId(cache, backend, model) {
    for (const [key, cached] of cache) {
        if (key.startsWith(`${backend}:`) && cached.model === model)
            return key.slice(backend.length + 1);
    }
    return null;
}
export function routeCodexNotification(message, context) {
    const { backend, selectedBackend, selectedId, selectedModel, cache, hiddenThreads, hiddenTurns } = context;
    const params = record(record(message).params);
    const turn = record(params.turn);
    const explicitId = params.threadId || record(params.thread).id || turn.threadId;
    if (explicitId && hiddenThreads.has(context.sessionKey(backend, explicitId)))
        return null;
    if (explicitId) {
        if (selectedBackend === backend && selectedId === explicitId)
            return selectedModel;
        return cache.get(`${backend}:${explicitId}`)?.model || null;
    }
    const turnId = params.turnId || turn.id;
    if (turnId && hiddenTurns.has(context.turnKey(backend, turnId)))
        return null;
    if (turnId) {
        for (const [key, cached] of cache) {
            if (!key.startsWith(`${backend}:`))
                continue;
            if (cached.model.activeTurnId === turnId || cached.model.turns.some((candidate) => candidate.id === turnId))
                return cached.model;
        }
    }
    return selectedBackend === backend ? selectedModel : null;
}
function record(value) {
    return isRecord(value) ? value : {};
}
function isRecord(value) {
    return !!value && typeof value === 'object';
}
