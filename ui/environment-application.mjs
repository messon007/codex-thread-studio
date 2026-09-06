// Generated from ui-src; run npm run build:ui. Do not edit.
export function createEnvironmentApplication(effects) {
    return async function applyEnvironment(options) {
        const { backend, threadId, generation, root, profileRevision, force, includeThread, excludeTurns, initialTurnsPage } = options;
        if (!effects.supports(backend) || !threadId)
            return null;
        const key = `${backend}\u0000${threadId}\u0000${generation}\u0000${root}\u0000${profileRevision}`;
        if (!force && effects.applied.has(key))
            return null;
        const active = effects.requests.get(key);
        if (!force && active)
            return active;
        const request = (async () => {
            const apply = (incremental) => effects.apply({
                root, threadId, backend, includeThread, excludeTurns: incremental,
                ...(incremental && initialTurnsPage ? { initialTurnsPage } : {}),
            });
            let result;
            try {
                result = await apply(excludeTurns);
            }
            catch (error) {
                if (!excludeTurns || !effects.compatibilityError(error))
                    throw error;
                effects.disableTail(backend, threadId);
                result = await apply(false);
            }
            effects.applied.add(key);
            return result;
        })().finally(() => effects.requests.delete(key));
        effects.requests.set(key, request);
        return request;
    };
}
