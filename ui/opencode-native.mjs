// Generated from ui-src; run npm run build:ui. Do not edit.
import { safeImageUrl } from './composer-images.mjs';
import { markTranscriptModelChanged } from './model-revision.mjs';
import { reduceOpenCodeEvent } from './opencode-event-reducer.mjs';
const openCodeReducerHelpers = {
    normalizeOpenCodeStatus, errorText, ensureTurn, structuredOutputItem, upsertItem,
    upsertOpenCodeUserPart, openCodePartToItem, rememberMessageItem,
    forgetMessageItem, refreshTurnMessageError,
};
export async function collectOpenCodeRootSessions(fetchPage, requestedPageSize = 100) {
    const pageSize = Math.max(1, Math.min(100, Number(requestedPageSize) || 100));
    const sessions = [];
    const seenIds = new Set();
    const seenCursors = new Set();
    let cursor = null;
    for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
        const page = await fetchPage({
            limit: pageSize,
            archived: false,
            roots: true,
            ...(cursor == null ? {} : { cursor }),
        });
        if (!Array.isArray(page) || page.length === 0)
            break;
        let added = 0;
        for (const session of page) {
            if (!session?.id || seenIds.has(session.id))
                continue;
            seenIds.add(session.id);
            sessions.push(session);
            added += 1;
        }
        if (page.length < pageSize || added === 0)
            break;
        const nextCursor = Number(page.at(-1)?.time?.updated);
        if (!Number.isFinite(nextCursor) || seenCursors.has(nextCursor))
            break;
        seenCursors.add(nextCursor);
        cursor = nextCursor;
    }
    return sessions;
}
export async function collectOpenCodeMessageHistory(fetchPage, requestedPageSize = 500) {
    const pageSize = Math.max(1, Math.min(1_000, Number(requestedPageSize) || 500));
    const pages = [];
    const seenCursors = new Set();
    let before = null;
    let complete = false;
    for (let pageNumber = 0; pageNumber < 20; pageNumber += 1) {
        const page = await fetchPage({
            limit: pageSize,
            ...(before == null ? {} : { before }),
        });
        const messages = Array.isArray(page) ? page : page?.messages;
        if (!Array.isArray(messages))
            break;
        pages.unshift(messages);
        const nextCursor = String((page && !Array.isArray(page) ? page.cursor : '') || '');
        if (nextCursor && !seenCursors.has(nextCursor)) {
            seenCursors.add(nextCursor);
            before = nextCursor;
            continue;
        }
        if (!nextCursor) {
            complete = before != null || messages.length < pageSize;
        }
        break;
    }
    // Long reads start with the newest page. Refresh it after older pages finish so
    // messages completed during pagination replace their earlier snapshot.
    if (complete && pages.length > 1) {
        const latest = await fetchPage({ limit: pageSize });
        const messages = Array.isArray(latest) ? latest : latest?.messages;
        if (Array.isArray(messages))
            pages.push(messages);
    }
    return { messages: mergeOpenCodeMessagePages(pages), complete };
}
export async function collectOpenCodeMessageTail(fetchPage, anchorTurnIds, requestedPageSize = 80) {
    const pageSize = Math.max(1, Math.min(1_000, Number(requestedPageSize) || 80));
    const anchors = new Set((Array.isArray(anchorTurnIds) ? anchorTurnIds : [])
        .map((id) => String(id || ''))
        .filter(Boolean));
    if (!anchors.size)
        return { messages: [], complete: false, matched: false, anchorTurnId: '' };
    const pages = [];
    const seenCursors = new Set();
    let before = null;
    let complete = false;
    for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
        const page = await fetchPage({
            limit: pageSize,
            ...(before == null ? {} : { before }),
        });
        const messages = Array.isArray(page) ? page : page?.messages;
        if (!Array.isArray(messages))
            break;
        pages.unshift(messages);
        let merged = mergeOpenCodeMessagePages(pages);
        let tail = sliceOpenCodeMessageTail(merged, anchors);
        if (tail.matched) {
            // Older-page reads can overlap a still-streaming newest page. Refresh only
            // that page so the authoritative tail wins without downloading history.
            if (pages.length > 1) {
                const latest = await fetchPage({ limit: pageSize });
                const latestMessages = Array.isArray(latest) ? latest : latest?.messages;
                if (Array.isArray(latestMessages)) {
                    pages.push(latestMessages);
                    merged = mergeOpenCodeMessagePages(pages);
                    tail = sliceOpenCodeMessageTail(merged, anchors);
                }
            }
            return {
                messages: tail.messages,
                complete: false,
                matched: true,
                anchorTurnId: tail.anchorTurnId,
            };
        }
        const nextCursor = String((page && !Array.isArray(page) ? page.cursor : '') || '');
        if (nextCursor && !seenCursors.has(nextCursor)) {
            seenCursors.add(nextCursor);
            before = nextCursor;
            continue;
        }
        if (!nextCursor)
            complete = before != null || messages.length < pageSize;
        break;
    }
    return {
        messages: mergeOpenCodeMessagePages(pages),
        complete,
        matched: false,
        anchorTurnId: '',
    };
}
export async function fetchOpenCodeDirectoryStatuses(directories, fetchStatus, requestedConcurrency = 6) {
    const queue = [...new Set((Array.isArray(directories) ? directories : []).filter(Boolean))];
    if (!queue.length)
        return {};
    const concurrency = Math.max(1, Math.min(queue.length, Number(requestedConcurrency) || 6));
    const results = new Array(queue.length);
    let nextIndex = 0;
    const worker = async () => {
        while (nextIndex < queue.length) {
            const index = nextIndex;
            nextIndex += 1;
            try {
                results[index] = await fetchStatus(queue[index]);
            }
            catch {
                results[index] = {};
            }
        }
    };
    await Promise.all(Array.from({ length: concurrency }, worker));
    return Object.assign({}, ...results);
}
export function mergeOpenCodeMessagePages(pages) {
    const messages = [];
    const positions = new Map();
    for (const page of pages) {
        for (const message of page) {
            const id = String(message?.info?.id || '');
            if (!id) {
                messages.push(message);
                continue;
            }
            const position = positions.get(id);
            if (position == null) {
                positions.set(id, messages.length);
                messages.push(message);
            }
            else {
                messages[position] = message;
            }
        }
    }
    return messages;
}
export function mergeOpenCodeThreadTail(model, thread, anchorTurnId) {
    const anchor = String(anchorTurnId || '');
    const cachedIndex = model?.turns?.findIndex((turn) => String(turn?.id || '') === anchor) ?? -1;
    const incomingIndex = thread?.turns?.findIndex((turn) => String(turn?.id || '') === anchor) ?? -1;
    if (!model || cachedIndex < 0 || incomingIndex < 0)
        return false;
    const removedTurnIds = new Set(model.turns.slice(cachedIndex)
        .map((turn) => String(turn?.id || ''))
        .filter(Boolean));
    const preservedMessageTurns = Object.fromEntries(Object.entries(model.messageTurns || {})
        .filter(([, turnId]) => !removedTurnIds.has(String(turnId || ''))));
    const preservedMessageIds = new Set(Object.keys(preservedMessageTurns));
    const preserveMessageMetadata = (values) => Object.fromEntries(Object.entries(values || {})
        .filter(([messageId]) => preservedMessageIds.has(messageId)));
    const preservedErrors = Object.fromEntries(Object.entries(model.messageErrors || {})
        .filter(([messageId, value]) => preservedMessageIds.has(messageId)
        || !removedTurnIds.has(String(value?.turnId || ''))));
    model.turns = [
        ...model.turns.slice(0, cachedIndex),
        ...thread.turns.slice(incomingIndex),
    ];
    model.threadId = thread.id || model.threadId;
    model.messageTurns = { ...preservedMessageTurns, ...(thread.messageTurns || {}) };
    model.messageRoles = { ...preserveMessageMetadata(model.messageRoles), ...(thread.messageRoles || {}) };
    model.messageItems = { ...preserveMessageMetadata(model.messageItems), ...(thread.messageItems || {}) };
    model.messageErrors = { ...preservedErrors, ...(thread.messageErrors || {}) };
    model.error = null;
    model.status = thread.status || model.status;
    model.activeTurnId = model.status === 'running' ? model.turns.at(-1)?.id || null : null;
    markTranscriptModelChanged(model);
    return true;
}
export function sliceOpenCodeMessageTail(messages, anchorTurnIds) {
    const values = Array.isArray(messages) ? messages : [];
    const anchors = anchorTurnIds instanceof Set
        ? anchorTurnIds
        : new Set((Array.isArray(anchorTurnIds) ? anchorTurnIds : [])
            .map((id) => String(id || ''))
            .filter(Boolean));
    const anchorIndex = newestOpenCodeAnchorIndex(values, anchors);
    if (anchorIndex < 0)
        return { messages: values, matched: false, anchorTurnId: '' };
    return {
        messages: values.slice(anchorIndex),
        matched: true,
        anchorTurnId: String(values[anchorIndex]?.info?.id || ''),
    };
}
export function normalizeOpenCodeSessions(sessions, statuses = {}) {
    return (Array.isArray(sessions) ? sessions : []).map((session) => ({
        id: session.id,
        name: session.title || '',
        preview: session.title || '',
        cwd: session.directory || '',
        parentThreadId: session.parentID || null,
        status: normalizeOpenCodeStatus(statuses?.[session.id] || 'idle'),
        source: 'opencode',
        model: session.modelID || session.model?.modelID || (session.model?.providerID && session.model?.id ? `${session.model.providerID}/${session.model.id}` : ''),
        updatedAt: session.time?.updated || session.time?.created || 0,
        native: session,
    }));
}
function newestOpenCodeAnchorIndex(messages, anchors) {
    for (let index = messages.length - 1; index >= 0; index -= 1) {
        const info = messages[index]?.info || {};
        if (info.role === 'user' && anchors.has(String(info.id || '')))
            return index;
    }
    return -1;
}
export function openCodeThreadFromHistory(session, messages, status) {
    const turns = [];
    const turnById = new Map();
    const messageTurns = {};
    const messageRoles = {};
    const messageItems = {};
    const messageErrors = {};
    let current = null;
    for (const message of Array.isArray(messages) ? messages : []) {
        const info = message?.info || {};
        if (info.id)
            messageRoles[info.id] = info.role;
        if (info.role === 'user') {
            const studioUserParts = {};
            for (const part of message.parts || []) {
                const additions = userContentFromPart(part);
                if (additions.length)
                    studioUserParts[part.id || `${part.type}-${Object.keys(studioUserParts).length}`] = additions;
            }
            const itemId = info.id || `user-item-${turns.length}`;
            current = {
                id: info.id || `user-${turns.length}`,
                status: 'completed',
                items: [{
                        id: itemId,
                        type: 'userMessage',
                        content: Object.values(studioUserParts).flat(),
                        studioUserParts,
                    }],
            };
            turns.push(current);
            turnById.set(current.id, current);
            if (info.id) {
                messageTurns[info.id] = current.id;
                rememberMessageItem(messageItems, info.id, itemId);
            }
            continue;
        }
        const turnId = info.parentID || current?.id || info.id || `assistant-${turns.length}`;
        current = turnById.get(turnId) || current;
        if (!current) {
            current = { id: turnId, status: 'completed', items: [] };
            turns.push(current);
            turnById.set(turnId, current);
        }
        if (info.id)
            messageTurns[info.id] = current.id;
        for (const part of message.parts || []) {
            const item = openCodePartToItem(part, info.role);
            upsertItem(current, item);
            if (info.id && item?.id)
                rememberMessageItem(messageItems, info.id, item.id);
        }
        if (info.structured !== undefined) {
            const item = structuredOutputItem(info);
            upsertItem(current, item);
            if (info.id)
                rememberMessageItem(messageItems, info.id, item.id);
        }
        if (info.error) {
            current.status = 'failed';
            const message = errorText(info.error);
            current.error = { message };
            if (info.id)
                messageErrors[info.id] = { turnId: current.id, message };
        }
    }
    const normalizedStatus = normalizeOpenCodeStatus(status);
    const active = normalizedStatus === 'running' ? turns.at(-1) : null;
    if (active)
        active.status = 'inProgress';
    return {
        id: session?.id,
        name: session?.title || '',
        cwd: session?.directory || '',
        parentThreadId: session?.parentID || null,
        source: 'opencode',
        status: normalizedStatus,
        turns,
        messageTurns,
        messageRoles,
        messageItems,
        messageErrors,
    };
}
export function applyOpenCodeEvent(model, event, selectedSessionId) {
    const result = reduceOpenCodeEvent(model, event, selectedSessionId, openCodeReducerHelpers);
    if (result.handled)
        markTranscriptModelChanged(model);
    return result;
}
export function replayOpenCodeEventsAfterHistory(model, bufferedEvents, selectedSessionId, { messageSnapshots = {}, statusAfterSequence = -1, authoritativeStatus = null, } = {}) {
    const entries = (bufferedEvents || []).map((entry) => entry?.event
        ? entry
        : { event: entry, sequence: Number.POSITIVE_INFINITY });
    const ambiguousDeltas = new Map();
    const snapshotForMessage = (messageId) => messageSnapshots?.[String(messageId || '')]
        || { afterSequence: -1, ambiguousThroughSequence: -1 };
    const flushAmbiguousDeltas = (matches = () => true) => {
        for (const [key, event] of ambiguousDeltas) {
            if (!matches(event.properties || {}))
                continue;
            mergeAmbiguousOpenCodeDelta(model, event, selectedSessionId);
            ambiguousDeltas.delete(key);
        }
    };
    const discardAmbiguousDeltas = (matches) => {
        for (const [key, event] of ambiguousDeltas) {
            if (matches(event.properties || {}))
                ambiguousDeltas.delete(key);
        }
    };
    let statusApplied = authoritativeStatus == null;
    const applyStatusSnapshot = () => {
        if (statusApplied)
            return;
        if (authoritativeStatus === null)
            return;
        applyOpenCodeStatusSnapshot(model, authoritativeStatus);
        statusApplied = true;
    };
    for (const { event, sequence } of entries) {
        if (sequence > statusAfterSequence)
            applyStatusSnapshot();
        const payload = event?.payload || event;
        if (payload?.type === 'message.part.delta') {
            const properties = payload.properties || {};
            const key = `${properties.messageID || ''}\u0000${properties.partID || ''}\u0000${properties.field || 'text'}`;
            const snapshot = snapshotForMessage(properties.messageID);
            if (sequence <= snapshot.ambiguousThroughSequence) {
                const current = ambiguousDeltas.get(key);
                ambiguousDeltas.set(key, {
                    type: 'message.part.delta',
                    properties: {
                        ...properties,
                        delta: `${current?.properties?.delta || ''}${properties.delta || ''}`,
                    },
                });
            }
            else {
                flushAmbiguousDeltas((delta) => String(delta.messageID || '') === String(properties.messageID || '')
                    && String(delta.partID || '') === String(properties.partID || ''));
                applyOpenCodeEvent(model, event, selectedSessionId);
            }
            continue;
        }
        const snapshotMessageId = payload?.type === 'message.updated'
            ? payload.properties?.info?.id
            : ['message.part.updated', 'message.part.removed', 'message.removed'].includes(payload?.type || '')
                ? payload.properties?.part?.messageID
                    || payload.properties?.messageID
                : null;
        const snapshot = snapshotForMessage(snapshotMessageId);
        if (snapshotMessageId && sequence <= snapshot.afterSequence) {
            if (payload?.type === 'message.part.updated') {
                const part = payload.properties?.part || {};
                discardAmbiguousDeltas((delta) => String(delta.messageID || '') === String(part.messageID || '')
                    && String(delta.partID || '') === String(part.id || ''));
            }
            else if (payload?.type === 'message.part.removed') {
                discardAmbiguousDeltas((delta) => String(delta.messageID || '') === String(payload.properties?.messageID || '')
                    && String(delta.partID || '') === String(payload.properties?.partID || ''));
            }
            else if (payload?.type === 'message.removed') {
                discardAmbiguousDeltas((delta) => String(delta.messageID || '') === String(payload.properties?.messageID || ''));
            }
            continue;
        }
        if (payload?.type === 'session.error' && sequence <= statusAfterSequence) {
            model.error = errorText(payload.properties?.error || payload.properties);
            continue;
        }
        if (['session.status', 'session.idle'].includes(payload?.type || '') && sequence <= statusAfterSequence)
            continue;
        if (['message.part.updated', 'message.part.removed'].includes(payload?.type || '')) {
            const part = payload.type === 'message.part.updated' ? payload.properties?.part || {} : payload.properties || {};
            flushAmbiguousDeltas((delta) => String(delta.messageID || '') === String(part.messageID || '')
                && String(delta.partID || '') === String(part.id || part.partID || ''));
        }
        else if (payload?.type === 'message.removed') {
            flushAmbiguousDeltas((delta) => String(delta.messageID || '') === String(payload.properties?.messageID || ''));
        }
        applyOpenCodeEvent(model, event, selectedSessionId);
    }
    flushAmbiguousDeltas();
    applyStatusSnapshot();
    return model;
}
function applyOpenCodeStatusSnapshot(model, status) {
    model.status = status;
    if (status === 'running') {
        const active = model.turns.find((turn) => turn.id === model.activeTurnId) || model.turns.at(-1);
        model.activeTurnId = active?.id || null;
        if (active)
            active.status = 'inProgress';
        return;
    }
    const active = model.turns.find((turn) => turn.id === model.activeTurnId) || model.turns.at(-1);
    model.activeTurnId = null;
    if (active?.status === 'inProgress')
        active.status = status === 'failed' ? 'failed' : 'completed';
}
function mergeAmbiguousOpenCodeDelta(model, event, selectedSessionId) {
    const properties = event.properties || {};
    const sessionId = properties.sessionID || properties.sessionId;
    if (sessionId && selectedSessionId && sessionId !== selectedSessionId)
        return;
    const messageId = String(properties.messageID || '');
    const itemId = String(properties.partID || '');
    const turn = ensureTurn(model, model.messageTurns?.[messageId] || model.activeTurnId);
    const item = turn.items.find((candidate) => String(candidate.id || '') === itemId);
    if (!item) {
        applyOpenCodeEvent(model, event, selectedSessionId);
        return;
    }
    const field = String(properties.field || 'text');
    if (item.type === 'reasoning')
        item.content = [mergeOpenCodeStreamText(item.content?.[0], properties.delta)];
    else {
        item[field] = mergeOpenCodeStreamText(item[field], properties.delta);
        const firstContent = item.content?.[0];
        if (field === 'text' && Array.isArray(item.content) && isRecord(firstContent) && firstContent.type === 'text') {
            item.content[0] = { ...firstContent, text: item.text };
        }
    }
    model.messageItems ||= {};
    rememberMessageItem(model.messageItems, messageId, itemId);
}
export function normalizeOpenCodeStatus(status) {
    const type = record(status).type || status;
    if (type === 'busy' || type === 'active' || type === 'retry')
        return 'running';
    if (type === 'error')
        return 'failed';
    return type === 'idle' ? 'idle' : 'notLoaded';
}
export function openCodeModelList(providerResult) {
    const providers = providerResult?.providers || providerResult?.all || [];
    const connected = new Set(providerResult?.connected || []);
    return providers.filter((provider) => !connected.size || connected.has(provider.id)).flatMap((provider) => Object.entries(provider.models || {}).map(([id, model]) => ({
        id: `${provider.id}/${id}`,
        model: `${provider.id}/${id}`,
        displayName: `${model?.name || id} · ${provider.name || provider.id}`,
        isDefault: providerResult?.default?.[provider.id] === id || providerResult?.default?.[provider.id] === `${provider.id}/${id}`,
        supportedReasoningEfforts: [],
    })));
}
export function splitOpenCodeModel(value) {
    const [providerID, ...model] = String(value || '').split('/');
    return providerID && model.length ? { providerID, modelID: model.join('/') } : null;
}
export function openCodeCommandUserId(result) {
    const info = result?.info || {};
    return String(info.role === 'user' ? info.id || '' : info.parentID || '');
}
export function openCodeCommandTurn(result, input = []) {
    const info = result?.info || {};
    const userId = openCodeCommandUserId(result);
    if (!userId)
        return null;
    const items = [{ id: userId, type: 'userMessage', content: input }];
    for (const part of result?.parts || []) {
        const item = openCodePartToItem(part, 'assistant');
        if (item)
            items.push(item);
    }
    if (info.structured != null)
        items.push(structuredOutputItem(info));
    return {
        id: userId,
        status: info.error ? 'failed' : 'completed',
        items,
        ...(info.error ? { error: { message: errorText(info.error) } } : {}),
    };
}
export function selectOpenCodeStartedUserMessage(messages, { baselineIds = [], expectedText = '' } = {}) {
    const baseline = new Set([...baselineIds].map((id) => String(id || '')).filter(Boolean));
    const expected = String(expectedText || '').trim();
    const candidates = [...(Array.isArray(messages) ? messages : [])].reverse()
        .filter((message) => message?.info?.role === 'user'
        && message.info.id
        && !baseline.has(String(message.info.id)));
    if (!expected)
        return candidates[0] || null;
    return candidates.find((candidate) => (candidate.parts || [])
        .filter((part) => part?.type === 'text')
        .map((part) => part.text || '').join('\n').trim() === expected) || null;
}
export function createOpenCodeLoopGuard({ terminalLimit = 3 } = {}) {
    const sessions = new Map();
    const limit = Math.max(3, Math.floor(Number(terminalLimit) || 3));
    function clear(sessionId) {
        sessions.delete(String(sessionId || ''));
    }
    function observe(event) {
        const payload = event?.payload || event || {};
        const properties = payload.properties || {};
        const sessionId = String(properties.sessionID || properties.info?.sessionID || properties.part?.sessionID || '');
        if (!sessionId)
            return null;
        const sessionStatus = properties.status || properties;
        if (payload.type === 'session.idle'
            || payload.type === 'session.deleted'
            || (payload.type === 'session.status' && record(sessionStatus).type === 'idle')) {
            clear(sessionId);
            return null;
        }
        let session = sessions.get(sessionId);
        if (!session) {
            session = { parents: new Map(), messages: new Map() };
            sessions.set(sessionId, session);
        }
        if (payload.type === 'message.part.updated' || payload.type === 'message.part.delta') {
            const part = properties.part || properties;
            const messageId = String(part.messageID || properties.messageID || '');
            if (!messageId)
                return null;
            const message = session.messages.get(messageId) || { hasContent: false, hasTool: false };
            if (part.type === 'tool')
                message.hasTool = true;
            if (part.type === 'file')
                message.hasContent = true;
            const content = part.text ?? properties.delta;
            if (['text', 'reasoning', 'file'].includes(part.type || '') && String(content || '').trim())
                message.hasContent = true;
            if (payload.type === 'message.part.delta' && String(properties.delta || '').trim())
                message.hasContent = true;
            session.messages.set(messageId, message);
            return null;
        }
        if (payload.type !== 'message.updated')
            return null;
        const info = properties.info || {};
        if (info.role !== 'assistant' || info.error || !info.parentID)
            return null;
        if (!info.finish || ['tool-calls', 'unknown'].includes(info.finish))
            return null;
        const message = session.messages.get(String(info.id || ''));
        if (message?.hasTool || message?.hasContent || info.structured != null || Number(info.tokens?.output || 0) > 0)
            return null;
        const parentId = String(info.parentID);
        const messageId = String(info.id || '');
        let state = session.parents.get(parentId);
        if (!state) {
            state = { ids: new Set(), lastSignaledSize: 0 };
            session.parents.set(parentId, state);
        }
        if (messageId)
            state.ids.add(messageId);
        if (state.ids.size < limit || state.ids.size === state.lastSignaledSize)
            return null;
        state.lastSignaledSize = state.ids.size;
        return { sessionId, parentId, assistantIds: [...state.ids] };
    }
    return { observe, clear };
}
function userContentFromPart(part) {
    if (part?.type === 'text')
        return [{ type: 'text', text: part.text || '' }];
    if (part?.type === 'file' && String(part.mime || '').startsWith('image/') && safeImageUrl(part.url)) {
        return [{ type: 'image', url: part.url }];
    }
    if (part?.type === 'file')
        return [{ type: 'text', text: `@${part.filename || part.url || part.path || 'file'}` }];
    return [];
}
function upsertOpenCodeUserPart(turn, part) {
    const additions = userContentFromPart(part);
    if (!additions.length)
        return;
    const id = part.messageID || turn.id || 'user-message';
    const item = turn.items.find((candidate) => candidate.type === 'userMessage') || { id, type: 'userMessage', content: [] };
    item.studioUserParts ||= {};
    item.studioUserParts[part.id || `${part.type}-${Object.keys(item.studioUserParts).length}`] = additions;
    item.content = Object.values(item.studioUserParts).flat();
    upsertItem(turn, item);
}
function structuredOutputItem(info) {
    return {
        id: `${info.id || 'assistant'}-structured`,
        type: 'agentMessage',
        text: JSON.stringify(info.structured),
    };
}
function openCodePartToItem(part, role) {
    const id = part?.id || `${part?.messageID || 'message'}-${part?.type || 'part'}`;
    if (part?.type === 'text')
        return { id, type: role === 'user' ? 'userMessage' : 'agentMessage', text: part.text || '', content: [{ type: 'text', text: part.text || '' }] };
    if (part?.type === 'reasoning')
        return { id, type: 'reasoning', content: [part.text || ''] };
    if (part?.type === 'tool') {
        const state = part.state || {};
        if (['bash', 'shell'].includes(part.tool || '')) {
            return { id, type: 'commandExecution', command: state.input?.command || state.title || '', aggregatedOutput: state.output || state.error || '', status: toolStatus(state.status) };
        }
        return { id, type: 'mcpToolCall', server: 'OpenCode', tool: part.tool || 'tool', arguments: state.input || {}, result: state.output || '', error: state.error || '', status: toolStatus(state.status) };
    }
    if (part?.type === 'patch')
        return { id, type: 'fileChange', changes: [{ kind: 'patch', path: (part.files || []).join(', '), diff: part.hash || '' }], status: 'completed' };
    if (part?.type === 'compaction')
        return { id, type: 'contextCompaction' };
    if (part?.type === 'step-start' || part?.type === 'step-finish')
        return null;
    return { ...part, id, type: part?.type || 'unknown' };
}
function toolStatus(status) {
    const statuses = { pending: 'inProgress', running: 'inProgress', completed: 'completed', error: 'failed' };
    return statuses[String(status)] || status || 'inProgress';
}
function ensureTurn(model, id) {
    const turnId = id || model.activeTurnId || `opencode-${model.turns.length}`;
    let turn = model.turns.find((candidate) => candidate.id === turnId);
    if (!turn) {
        turn = { id: turnId, status: 'inProgress', items: [] };
        model.turns.push(turn);
    }
    if (!Array.isArray(turn.items))
        turn.items = [];
    return turn;
}
function upsertItem(turn, item) {
    if (!item)
        return;
    const index = turn.items.findIndex((candidate) => candidate.id === item.id);
    if (index < 0)
        turn.items.push(item);
    else
        turn.items[index] = { ...turn.items[index], ...item };
}
function rememberMessageItem(messageItems, messageId, itemId) {
    const messageKey = String(messageId || '');
    const itemKey = String(itemId || '');
    if (!messageKey || !itemKey)
        return;
    const items = messageItems[messageKey] ||= [];
    if (!items.some((id) => String(id || '') === itemKey))
        items.push(itemKey);
}
function forgetMessageItem(messageItems, messageId, itemId) {
    const messageKey = String(messageId || '');
    if (!messageItems?.[messageKey])
        return;
    const itemKey = String(itemId || '');
    messageItems[messageKey] = messageItems[messageKey].filter((id) => String(id || '') !== itemKey);
    if (!messageItems[messageKey].length)
        delete messageItems[messageKey];
}
function refreshTurnMessageError(model, turn) {
    let error = null;
    for (const [messageId, entry] of Object.entries(model.messageErrors || {})) {
        const turnId = entry?.turnId || model.messageTurns?.[messageId];
        if (String(turnId || '') === String(turn.id || ''))
            error = entry;
    }
    if (error) {
        turn.error = { message: error.message };
        turn.status = 'failed';
        return;
    }
    delete turn.error;
    if (turn.status === 'failed') {
        turn.status = model.status === 'running' && model.activeTurnId === turn.id ? 'inProgress' : 'completed';
    }
}
function mergeOpenCodeStreamText(historyValue, liveValue) {
    const history = String(historyValue || '');
    const live = String(liveValue || '');
    if (!history || live.startsWith(history))
        return live || history;
    if (!live || history.endsWith(live))
        return history;
    const limit = Math.min(history.length, live.length);
    for (let overlap = limit; overlap > 0; overlap -= 1) {
        if (history.endsWith(live.slice(0, overlap)))
            return history + live.slice(overlap);
    }
    return history + live;
}
function errorText(error) {
    if (typeof error === 'string')
        return error;
    return error?.data?.message || error?.message || error?.name || JSON.stringify(error || {});
}
function record(value) {
    return isRecord(value) ? value : {};
}
function isRecord(value) {
    return !!value && typeof value === 'object';
}
