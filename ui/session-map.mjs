// Generated from ui-src; run npm run build:ui. Do not edit.
function record(value) {
    return value !== null && typeof value === 'object' ? value : {};
}
function isMapState(value) { return typeof value === 'string' && MAP_STATES.includes(value); }
function isMapStructure(value) { return typeof value === 'string' && MAP_STRUCTURES.includes(value); }
function isAssistantState(value) { return typeof value === 'string' && SAFE_ASSISTANT_STATES.has(value); }
export const MAP_STRUCTURES = Object.freeze(['hierarchy', 'path', 'flow', 'blank']);
export const MAP_STATES = Object.freeze(['notStarted', 'active', 'visited', 'done', 'paused']);
export const SESSION_MAP_UPDATE_START = '<codex-thread-studio-session-map-update-v1>';
export const SESSION_MAP_UPDATE_END = '</codex-thread-studio-session-map-update-v1>';
export class SessionMapWorkerPool {
    workers;
    constructor() {
        this.workers = new Map();
    }
    enqueue(key, execute) {
        if (!key)
            return Promise.reject(new Error('Map worker requires a Session Map key'));
        let worker = this.workers.get(key);
        if (!worker) {
            worker = { key, threadId: null, generation: null, chain: Promise.resolve(), disposed: false };
            this.workers.set(key, worker);
        }
        const run = () => {
            if (worker.disposed)
                throw new Error('Session Map worker has been released');
            return execute(worker);
        };
        const result = worker.chain.then(run, run);
        worker.chain = result.catch(() => undefined);
        return result;
    }
    reconcileGeneration(worker, generation) {
        const changed = worker.generation != null && worker.generation !== generation;
        const previousThreadId = changed ? worker.threadId : null;
        if (changed)
            worker.threadId = null;
        worker.generation = generation;
        return previousThreadId;
    }
    dispose(key) {
        const worker = this.workers.get(key);
        if (!worker)
            return null;
        worker.disposed = true;
        this.workers.delete(key);
        return worker;
    }
}
export function sessionMapKey(backend, threadId) {
    return backend && threadId ? `${backend}:${threadId}` : '';
}
export function sessionMapEndpoint(backend, threadId, suffix = '') {
    const base = `/studio/session-map/${encodeURIComponent(backend)}/${encodeURIComponent(threadId)}`;
    return suffix ? `${base}/${suffix}` : base;
}
export function normalizeSessionMap(input) {
    const value = record(input);
    if (!value || !value.id || !value.backend || !value.threadId)
        return null;
    const itemIds = new Set();
    const items = (Array.isArray(value.items) ? value.items : []).map(record)
        .filter((item) => item?.id && item?.title && !itemIds.has(String(item.id)))
        .map((item) => {
        itemIds.add(String(item.id));
        return {
            id: String(item.id),
            parentId: item.parentId == null ? null : String(item.parentId),
            kind: String(item.kind || 'item'),
            title: String(item.title),
            summary: String(item.summary || ''),
            state: isMapState(item.state) ? item.state : 'notStarted',
            position: Number.isFinite(Number(item.position)) ? Number(item.position) : 0,
            archived: Boolean(item.archived),
            createdAt: Number(item.createdAt || 0),
            updatedAt: Number(item.updatedAt || 0),
        };
    });
    const visibleIds = new Set(items.filter((item) => !item.archived).map((item) => item.id));
    return {
        id: String(value.id),
        backend: String(value.backend),
        threadId: String(value.threadId),
        goal: String(value.goal || ''),
        definitionOfDone: String(value.definitionOfDone || ''),
        structure: isMapStructure(value.structure) ? value.structure : 'hierarchy',
        revision: Number(value.revision || 0),
        currentItemId: visibleIds.has(String(value.currentItemId)) ? String(value.currentItemId) : null,
        lastSyncedTurnId: value.lastSyncedTurnId == null ? null : String(value.lastSyncedTurnId),
        items,
        relations: Array.isArray(value.relations) ? value.relations : [],
        createdAt: Number(value.createdAt || 0),
        updatedAt: Number(value.updatedAt || 0),
    };
}
export function visibleMapItems(map) {
    return (map?.items || []).filter((item) => !item.archived);
}
function trailingMarkerPrefixLength(value, marker) {
    const limit = Math.min(value.length, marker.length - 1);
    for (let length = limit; length > 0; length -= 1) {
        if (marker.startsWith(value.slice(-length)))
            return length;
    }
    return 0;
}
export function sessionMapVisibleText(value) {
    const text = String(value || '');
    const markerIndex = text.lastIndexOf(SESSION_MAP_UPDATE_START);
    if (markerIndex >= 0)
        return text.slice(0, markerIndex).trimEnd();
    const partialLength = trailingMarkerPrefixLength(text, SESSION_MAP_UPDATE_START);
    return partialLength ? text.slice(0, -partialLength).trimEnd() : text;
}
export function parseSessionMapUpdate(value) {
    const text = String(value || '');
    const markerIndex = text.lastIndexOf(SESSION_MAP_UPDATE_START);
    if (markerIndex < 0)
        return { found: false, visibleText: sessionMapVisibleText(text), update: null };
    const payloadStart = markerIndex + SESSION_MAP_UPDATE_START.length;
    const markerEnd = text.indexOf(SESSION_MAP_UPDATE_END, payloadStart);
    if (markerEnd < 0)
        throw new Error('The Map update block is incomplete');
    if (text.slice(markerEnd + SESSION_MAP_UPDATE_END.length).trim()) {
        throw new Error('The Map update block must be at the end of the response');
    }
    let update;
    try {
        update = JSON.parse(text.slice(payloadStart, markerEnd).trim());
    }
    catch {
        throw new Error('The Map update block is not valid JSON');
    }
    const payload = record(update);
    if (!update || typeof payload.baseRevision !== 'number' || !Number.isInteger(payload.baseRevision) || !Array.isArray(payload.operations)) {
        throw new Error('The Map update block is missing baseRevision or operations');
    }
    return { found: true, visibleText: text.slice(0, markerIndex).trimEnd(), update: { ...payload, baseRevision: payload.baseRevision, operations: payload.operations } };
}
export function mapChildren(map, parentId = null) {
    return visibleMapItems(map)
        .filter((item) => item.parentId === parentId)
        .sort((left, right) => left.position - right.position || left.title.localeCompare(right.title));
}
export function mapItemTrail(map, itemId = map?.currentItemId) {
    if (!map || !itemId)
        return [];
    const byId = new Map(visibleMapItems(map).map((item) => [item.id, item]));
    const trail = [];
    const visited = new Set();
    let item = byId.get(itemId);
    while (item && !visited.has(item.id)) {
        visited.add(item.id);
        trail.unshift(item);
        item = item.parentId ? byId.get(item.parentId) : null;
    }
    return trail;
}
export function mapProgress(map) {
    const items = visibleMapItems(map);
    const done = items.filter((item) => item.state === 'done').length;
    const explored = items.filter((item) => ['visited', 'done'].includes(item.state)).length;
    return { total: items.length, done, explored };
}
export function flattenSessionMap(map) {
    const rows = [];
    const visited = new Set();
    const walk = (parentId, depth) => {
        for (const item of mapChildren(map, parentId)) {
            if (visited.has(item.id))
                continue;
            visited.add(item.id);
            rows.push({ item, depth });
            walk(item.id, depth + 1);
        }
    };
    walk(null, 0);
    for (const item of visibleMapItems(map)) {
        if (!visited.has(item.id))
            rows.push({ item, depth: 0 });
    }
    return rows;
}
export function boundedMapContext(map, { maxItems = 80 } = {}) {
    if (!map)
        return '';
    const visible = visibleMapItems(map);
    const currentTrail = new Set(mapItemTrail(map).map((item) => item.id));
    const ranked = [...visible].sort((left, right) => {
        const leftRank = left.id === map.currentItemId ? 0 : currentTrail.has(left.id) ? 1 : left.state === 'active' ? 2 : left.state === 'notStarted' ? 3 : 4;
        const rightRank = right.id === map.currentItemId ? 0 : currentTrail.has(right.id) ? 1 : right.state === 'active' ? 2 : right.state === 'notStarted' ? 3 : 4;
        return leftRank - rightRank || left.position - right.position;
    });
    const selected = new Set(ranked.slice(0, maxItems).map((item) => item.id));
    for (const item of visible) {
        if (item.parentId && selected.has(item.id))
            selected.add(item.parentId);
    }
    const items = visible
        .filter((item) => selected.has(item.id))
        .map(({ id, parentId, kind, title, summary, state, position }) => ({ id, parentId, kind, title, summary, state, position }));
    return JSON.stringify({
        revision: map.revision,
        goal: map.goal,
        definitionOfDone: map.definitionOfDone,
        structure: map.structure,
        currentItemId: map.currentItemId,
        items,
        omittedItemCount: Math.max(0, visible.length - items.length),
    });
}
export function shouldBootstrapSessionMap(map) {
    return Boolean(map && !map.lastSyncedTurnId && visibleMapItems(map).length === 0);
}
export function bootstrapMapInput(map, interactions = []) {
    const recent = [];
    let remaining = 48_000;
    for (const interaction of (Array.isArray(interactions) ? interactions : []).slice(-12).reverse()) {
        if (remaining <= 0)
            break;
        const user = String(interaction?.user || '').slice(0, Math.min(8_000, remaining));
        remaining -= user.length;
        const assistant = String(interaction?.assistant || '').slice(0, Math.min(12_000, remaining));
        remaining -= assistant.length;
        if (user || assistant)
            recent.unshift({ user, assistant });
    }
    recent.forEach((interaction, index) => { interaction.turn = index + 1; });
    return `Create or extend a compact initial navigation structure for this optional Session Map.

Current map (authoritative JSON):
${boundedMapContext(map)}

Recent conversation interactions (oldest to newest):
${JSON.stringify(recent)}

Return only safe incremental operations. Create a useful outline of normally 4–12 items based on the goal and conversation. Preserve existing items. Add parents before their children, use unique stable item IDs, and use setCurrent last to identify the subject currently being discussed. Prefer specific chapters, components, packages, questions, or steps over vague categories. Never mark anything done, change the goal, archive, move, reorder, or delete content.`;
}
export function assistantOperationSchema() {
    const itemId = { type: 'string', minLength: 1, maxLength: 128 };
    const nullableItemId = { anyOf: [itemId, { type: 'null' }] };
    const nullableTitle = { anyOf: [{ type: 'string', minLength: 1, maxLength: 200 }, { type: 'null' }] };
    const nullableKind = { anyOf: [{ type: 'string', minLength: 1, maxLength: 40 }, { type: 'null' }] };
    const nullableSummary = { anyOf: [{ type: 'string', maxLength: 1000 }, { type: 'null' }] };
    const states = ['notStarted', 'active', 'visited'];
    const operation = {
        type: 'object',
        properties: {
            op: { type: 'string', enum: ['addItem', 'updateItem', 'setCurrent', 'setState'] },
            itemId: nullableItemId,
            parentId: nullableItemId,
            afterItemId: nullableItemId,
            title: nullableTitle,
            kind: nullableKind,
            summary: nullableSummary,
            state: { anyOf: [{ type: 'string', enum: states }, { type: 'null' }] },
        },
        required: ['op', 'itemId', 'parentId', 'afterItemId', 'title', 'kind', 'summary', 'state'],
        additionalProperties: false,
    };
    return {
        type: 'object',
        properties: {
            operations: { type: 'array', maxItems: 40, items: operation },
        },
        required: ['operations'],
        additionalProperties: false,
    };
}
export function safeAssistantOperations(input) {
    const value = record(input);
    const operations = Array.isArray(value?.operations) ? value.operations : [];
    return operations.map(normalizeAssistantOperation).filter((operation) => operation !== null);
}
export function structuredWorkerText(turn) {
    const text = (turn?.items || [])
        .filter((item) => item?.type === 'agentMessage' && item.text)
        .map((item) => String(item.text))
        .join('\n')
        .trim();
    if (text)
        return text;
    const error = structuredWorkerError(turn?.error);
    if (error)
        throw new Error(`Structured AI task failed: ${error}`);
    throw new Error('The structured AI task returned no result');
}
function normalizeAssistantOperation(value) {
    const operation = record(value);
    if (!operation || typeof operation !== 'object')
        return null;
    const itemId = boundedText(operation.itemId, 128);
    if (operation.op === 'addItem') {
        const title = boundedText(operation.title, 200);
        if (!itemId || !title || !isAssistantState(operation.state))
            return null;
        return {
            op: 'addItem',
            itemId,
            parentId: boundedText(operation.parentId, 128) || null,
            afterItemId: boundedText(operation.afterItemId, 128) || null,
            title,
            kind: boundedText(operation.kind, 40) || 'item',
            summary: boundedText(operation.summary, 1000),
            state: operation.state,
        };
    }
    if (operation.op === 'updateItem') {
        if (!itemId)
            return null;
        const normalized = {
            op: 'updateItem',
            itemId,
            title: nullableBoundedText(operation.title, 200),
            kind: nullableBoundedText(operation.kind, 40),
            summary: nullableBoundedText(operation.summary, 1000),
        };
        return [normalized.title, normalized.kind, normalized.summary].some((part) => part !== null) ? normalized : null;
    }
    if (operation.op === 'setCurrent') {
        if (operation.itemId != null && !itemId)
            return null;
        return { op: 'setCurrent', itemId: itemId || null };
    }
    if (operation.op === 'setState' && itemId && isAssistantState(operation.state)) {
        return { op: 'setState', itemId, state: operation.state };
    }
    return null;
}
const SAFE_ASSISTANT_STATES = new Set(['notStarted', 'active', 'visited']);
function boundedText(value, max) {
    return typeof value === 'string' ? value.trim().slice(0, max) : '';
}
function nullableBoundedText(value, max) {
    return value == null ? null : boundedText(value, max) || null;
}
function structuredWorkerError(value) {
    const error = record(value);
    const message = String(error?.message || '').trim();
    if (!message)
        return '';
    try {
        const parsed = record(JSON.parse(message));
        return String(record(parsed.error).message || message).trim().slice(0, 2000);
    }
    catch {
        return message.slice(0, 2000);
    }
}
export function sessionMapTurnConfiguration(map) {
    if (!map)
        return {};
    const context = boundedMapContext(map);
    const rules = `This thread has an optional Session Map maintained by the client. Use it to preserve the user's goal and current navigation position. Do not claim that an item is complete merely because it was discussed. Current Session Map JSON: ${context}

When update_session_map is available, use it for safe incremental operations. If and only if that tool succeeds, do not emit the fallback block below.

When update_session_map is unavailable, append exactly one machine-readable block at the absolute end of every final answer, after the complete user-facing Markdown response:
${SESSION_MAP_UPDATE_START}
{"baseRevision":${map.revision},"operations":[]}
${SESSION_MAP_UPDATE_END}

Replace operations with safe incremental operations when the Map should change; otherwise keep the empty array. The block is private client metadata: never explain or refer to it in the user-facing answer. Allowed operations are addItem, updateItem, setCurrent, and setState with only notStarted, active, or visited. Add parents before children. Never change the goal, mark completion, archive, move, reorder, delete, or modify relations.`;
    return {
        developerInstructions: rules,
        dynamicTools: [{
                type: 'function',
                name: 'update_session_map',
                description: `${rules} The client validates operations transactionally and may reject unsafe changes.`,
                inputSchema: assistantOperationSchema(),
            }],
    };
}
