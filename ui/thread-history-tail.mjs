// Generated from ui-src; run npm run build:ui. Do not edit.
export const DEFAULT_TURN_TAIL_PAGE_SIZE = 4;
export const MAX_TURN_TAIL_PAGES = 100;
export async function collectCodexTurnTail({ cachedTurns, initialPage, fetchPage, maxPages = MAX_TURN_TAIL_PAGES, }) {
    const existing = Array.isArray(cachedTurns) ? [...cachedTurns] : [];
    const knownIndexes = new Map();
    existing.forEach((turn, index) => {
        const id = String(turn?.id || '');
        if (id)
            knownIndexes.set(id, index);
    });
    const receivedDescending = [];
    const receivedIds = new Set();
    const seenCursors = new Set();
    let page = normalizePage(initialPage);
    let pageCount = 0;
    while (page && pageCount < Math.max(1, Number(maxPages) || MAX_TURN_TAIL_PAGES)) {
        pageCount += 1;
        for (const turn of page.data) {
            const id = String(turn?.id || '');
            if (!id || receivedIds.has(id))
                continue;
            receivedIds.add(id);
            receivedDescending.push(turn);
            const cachedIndex = knownIndexes.get(id);
            if (cachedIndex == null)
                continue;
            const tail = receivedDescending.slice().reverse();
            return {
                matched: true,
                anchorTurnId: id,
                turns: [...existing.slice(0, cachedIndex), ...tail],
                pageCount,
                fetchedTurnCount: receivedDescending.length,
                appendedTurnCount: tail.filter((candidate) => !knownIndexes.has(String(candidate?.id || ''))).length,
            };
        }
        const cursor = stringOrNull(page.nextCursor);
        if (!cursor || seenCursors.has(cursor) || typeof fetchPage !== 'function')
            break;
        seenCursors.add(cursor);
        page = normalizePage(await fetchPage(cursor));
    }
    if (!existing.length && !receivedDescending.length && !page?.nextCursor) {
        return {
            matched: true,
            anchorTurnId: '',
            turns: [],
            pageCount,
            fetchedTurnCount: 0,
            appendedTurnCount: 0,
        };
    }
    return {
        matched: false,
        anchorTurnId: '',
        turns: existing,
        pageCount,
        fetchedTurnCount: receivedDescending.length,
        appendedTurnCount: 0,
    };
}
export function isHistoryPaginationCompatibilityError(error) {
    const message = String((error && typeof error === 'object' && 'message' in error ? error.message : undefined) || error || '');
    return /(?:method (?:not found|unknown)|unknown (?:rpc )?method|unsupported(?: method)?|not supported|invalid params|unknown field|excludeTurns|initialTurnsPage|thread\/turns\/list[^\n]*(?:not found|unknown|unsupported|not supported))/iu.test(message);
}
function normalizePage(value) {
    if (!value || !Array.isArray(value.data))
        return null;
    return {
        data: value.data,
        nextCursor: stringOrNull(value.nextCursor),
        backwardsCursor: stringOrNull(value.backwardsCursor),
    };
}
function stringOrNull(value) {
    const text = String(value || '');
    return text || null;
}
