// Generated from ui-src; run npm run build:ui. Do not edit.
import { DEFAULT_TURN_TAIL_PAGE_SIZE, collectCodexTurnTail, collectCodexTurnHistory, isHistoryPaginationCompatibilityError } from './thread-history-tail.mjs';
export function createCodexHistoryLoader(services) {
    const { dispatchBackendRpc, requestCodexResume, historyTailCapability, rememberHistoryTailCapability, threadForRef } = services;
    async function readLegacyHistory(backend, id, resumed = null) {
        const read = await dispatchBackendRpc(backend, 'thread/read', { threadId: id, includeTurns: true });
        return { ...resumed, ...read, thread: { ...(resumed?.thread || {}), ...(read.thread || {}) } };
    }
    function fetchPage(backend, id, cursor) {
        return dispatchBackendRpc(backend, 'thread/turns/list', {
            threadId: id, limit: DEFAULT_TURN_TAIL_PAGE_SIZE, sortDirection: 'desc', itemsView: 'full',
            ...(cursor ? { cursor } : {}),
        });
    }
    async function pagedHistory(backend, id, cached, metadata, refreshFullMetadata = false) {
        const initialPage = Array.isArray(metadata.initialTurnsPage?.data)
            ? metadata.initialTurnsPage : await fetchPage(backend, id);
        const canReusePrefix = cached?.model?.threadId === id && cached.model.turns.length > 0;
        if (canReusePrefix) {
            const tail = await collectCodexTurnTail({
                cachedTurns: cached.model.turns, initialPage, fetchPage: cursor => fetchPage(backend, id, cursor),
            });
            if (tail.matched) {
                rememberHistoryTailCapability(backend, id, true);
                return { result: { ...metadata, thread: { ...metadata.thread, id, turns: tail.turns } }, historyMode: 'tail', tail };
            }
        }
        // Cold opens and unmatched caches must not fall back to a giant thread/read.
        if (refreshFullMetadata)
            metadata = await dispatchBackendRpc(backend, 'thread/read', { threadId: id, includeTurns: false });
        const history = await collectCodexTurnHistory({ initialPage, fetchPage: cursor => fetchPage(backend, id, cursor) });
        rememberHistoryTailCapability(backend, id, true);
        return {
            result: { ...metadata, thread: { ...metadata.thread, id, turns: history.turns } },
            historyMode: 'paged-full', tail: history,
        };
    }
    async function loadCodexHistoryForSelection(backend, id, cached, { environmentRoot = '', environmentRevision = '', } = {}) {
        if (historyTailCapability(backend, id) === false) {
            return { result: await requestCodexResume(backend, id, { environmentRoot, environmentRevision }), historyMode: 'full', tail: null };
        }
        let resumed = null;
        try {
            resumed = await requestCodexResume(backend, id, { environmentRoot, environmentRevision, incremental: true });
            if (Array.isArray(resumed?.thread?.turns) && resumed.thread.turns.length > 0) {
                rememberHistoryTailCapability(backend, id, false);
                return { result: resumed, historyMode: 'legacy-full', tail: null };
            }
            return await pagedHistory(backend, id, cached, resumed);
        }
        catch (error) {
            if (!isHistoryPaginationCompatibilityError(error))
                throw error;
            rememberHistoryTailCapability(backend, id, false);
            return {
                result: resumed ? await readLegacyHistory(backend, id, resumed)
                    : await requestCodexResume(backend, id, { environmentRoot, environmentRevision }),
                historyMode: 'compatibility-fallback', tail: null,
            };
        }
    }
    async function loadCodexHistoryForBackground(backend, id, cached) {
        if (historyTailCapability(backend, id) !== false) {
            try {
                const canReusePrefix = cached?.model?.threadId === id && cached.model.turns.length > 0;
                const metadata = canReusePrefix ? { thread: threadForRef({ backend, id }) || {} }
                    : await dispatchBackendRpc(backend, 'thread/read', { threadId: id, includeTurns: false });
                return await pagedHistory(backend, id, cached, metadata, canReusePrefix);
            }
            catch (error) {
                if (!isHistoryPaginationCompatibilityError(error))
                    throw error;
                rememberHistoryTailCapability(backend, id, false);
            }
        }
        return { result: await readLegacyHistory(backend, id), historyMode: 'full', tail: null };
    }
    return { loadCodexHistoryForSelection, loadCodexHistoryForBackground };
}
