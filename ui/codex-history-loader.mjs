// Generated from ui-src; run npm run build:ui. Do not edit.
import { DEFAULT_TURN_TAIL_PAGE_SIZE, collectCodexTurnTail, isHistoryPaginationCompatibilityError } from './thread-history-tail.mjs';
export function createCodexHistoryLoader(services) {
    const { dispatchBackendRpc, requestCodexResume, historyTailCapability, rememberHistoryTailCapability, threadForRef } = services;
    async function readFullCodexHistory(backend, id, resumed = null) {
        const read = await dispatchBackendRpc(backend, 'thread/read', { threadId: id, includeTurns: true });
        if (!resumed)
            return read;
        return {
            ...resumed,
            ...read,
            thread: { ...(resumed.thread || {}), ...(read.thread || {}) },
        };
    }
    async function loadCodexHistoryForSelection(backend, id, cached, { environmentRoot = '', environmentRevision = '', } = {}) {
        const canReusePrefix = cached?.model?.threadId === id
            && Array.isArray(cached.model.turns)
            && cached.model.turns.length > 0;
        if (!canReusePrefix || historyTailCapability(backend, id) === false) {
            return {
                result: await requestCodexResume(backend, id, { environmentRoot, environmentRevision }),
                historyMode: 'full',
                tail: null,
            };
        }
        let resumed = null;
        try {
            resumed = await requestCodexResume(backend, id, {
                environmentRoot,
                environmentRevision,
                incremental: true,
            });
            if (Array.isArray(resumed?.thread?.turns) && resumed.thread.turns.length > 0) {
                rememberHistoryTailCapability(backend, id, false);
                return { result: resumed, historyMode: 'legacy-full', tail: null };
            }
            let initialPage = resumed?.initialTurnsPage;
            if (!Array.isArray(initialPage?.data)) {
                initialPage = await dispatchBackendRpc(backend, 'thread/turns/list', {
                    threadId: id,
                    limit: DEFAULT_TURN_TAIL_PAGE_SIZE,
                    sortDirection: 'desc',
                    itemsView: 'full',
                });
            }
            const tail = await collectCodexTurnTail({
                cachedTurns: cached.model.turns,
                initialPage,
                fetchPage: (cursor) => dispatchBackendRpc(backend, 'thread/turns/list', {
                    threadId: id,
                    cursor,
                    limit: DEFAULT_TURN_TAIL_PAGE_SIZE,
                    sortDirection: 'desc',
                    itemsView: 'full',
                }),
            });
            rememberHistoryTailCapability(backend, id, true);
            if (tail.matched) {
                return {
                    result: {
                        ...resumed,
                        thread: { ...(resumed?.thread || {}), id, turns: tail.turns },
                    },
                    historyMode: 'tail',
                    tail,
                };
            }
            return {
                result: await readFullCodexHistory(backend, id, resumed),
                historyMode: 'full-fallback',
                tail,
            };
        }
        catch (error) {
            if (!isHistoryPaginationCompatibilityError(error))
                throw error;
            rememberHistoryTailCapability(backend, id, false);
            return {
                result: resumed
                    ? await readFullCodexHistory(backend, id, resumed)
                    : await requestCodexResume(backend, id, { environmentRoot, environmentRevision }),
                historyMode: 'compatibility-fallback',
                tail: null,
            };
        }
    }
    async function loadCodexHistoryForBackground(backend, id, cached) {
        const canReusePrefix = cached?.model?.threadId === id
            && Array.isArray(cached.model.turns)
            && cached.model.turns.length > 0;
        if (canReusePrefix && historyTailCapability(backend, id) !== false) {
            try {
                const initialPage = await dispatchBackendRpc(backend, 'thread/turns/list', {
                    threadId: id,
                    limit: DEFAULT_TURN_TAIL_PAGE_SIZE,
                    sortDirection: 'desc',
                    itemsView: 'full',
                });
                const tail = await collectCodexTurnTail({
                    cachedTurns: cached.model.turns,
                    initialPage,
                    fetchPage: (cursor) => dispatchBackendRpc(backend, 'thread/turns/list', {
                        threadId: id,
                        cursor,
                        limit: DEFAULT_TURN_TAIL_PAGE_SIZE,
                        sortDirection: 'desc',
                        itemsView: 'full',
                    }),
                });
                rememberHistoryTailCapability(backend, id, true);
                if (tail.matched) {
                    const metadata = threadForRef({ backend, id }) || {};
                    return {
                        result: { thread: { ...metadata, id, turns: tail.turns } },
                        historyMode: 'tail',
                        tail,
                    };
                }
            }
            catch (error) {
                if (!isHistoryPaginationCompatibilityError(error))
                    throw error;
                rememberHistoryTailCapability(backend, id, false);
            }
        }
        const result = await dispatchBackendRpc(backend, 'thread/read', { threadId: id, includeTurns: true });
        return { result, historyMode: 'full', tail: null };
    }
    return { loadCodexHistoryForSelection, loadCodexHistoryForBackground };
}
