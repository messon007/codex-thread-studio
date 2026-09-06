// Generated from ui-src; run npm run build:ui. Do not edit.
import { markTranscriptModelChanged } from './model-revision.mjs';
export function createCodexViewModel() {
    return {
        threadId: null,
        turns: [],
        activeTurnId: null,
        status: 'disconnected',
        error: null,
        diff: '',
        usage: null,
        approvals: [],
        interactions: [],
    };
}
export function hydrateCodexThread(model, thread) {
    model.threadId = thread?.id || model.threadId;
    // RPC history objects are freshly decoded and are not reused by callers.
    // Taking ownership avoids cloning an entire large session a second time.
    model.turns = Array.isArray(thread?.turns) ? thread.turns : [];
    model.error = null;
    let active = null;
    for (let index = model.turns.length - 1; index >= 0; index -= 1) {
        if (model.turns[index]?.status !== 'inProgress')
            continue;
        active = model.turns[index];
        break;
    }
    model.activeTurnId = active?.id || null;
    model.status = model.activeTurnId ? 'running' : 'idle';
    markTranscriptModelChanged(model);
    return model;
}
export function beginOptimisticCodexTurn(model, { clientUserMessageId, input }) {
    const clientId = String(clientUserMessageId || '');
    const turnId = `studio-pending-${clientId}`;
    model.turns.push({
        id: turnId,
        status: 'inProgress',
        items: [{
                id: `studio-user-${clientId}`,
                type: 'userMessage',
                clientId,
                content: structuredCloneSafe(input || []),
                studioOptimistic: true,
            }],
        studioOptimistic: true,
    });
    model.activeTurnId = turnId;
    model.status = 'running';
    model.error = null;
    markTranscriptModelChanged(model);
    return turnId;
}
export function reconcileOptimisticCodexTurn(model, optimisticTurnId, incomingTurn) {
    const optimisticIndex = model.turns.findIndex((turn) => turn.id === optimisticTurnId);
    const optimistic = optimisticIndex >= 0 ? model.turns[optimisticIndex] : null;
    if (optimisticIndex >= 0)
        model.turns.splice(optimisticIndex, 1);
    const existing = model.turns.find((candidate) => candidate.id === incomingTurn?.id);
    const terminalStatus = ['completed', 'failed', 'cancelled', 'interrupted'].includes(existing?.status || '')
        ? existing?.status
        : null;
    const turn = upsertTurn(model, terminalStatus ? { ...(incomingTurn || {}), status: terminalStatus } : incomingTurn || {});
    const user = optimistic?.items?.find((item) => item.type === 'userMessage');
    if (user && !turn.items.some((item) => sameUserMessage(item, user)))
        turn.items.unshift(user);
    model.activeTurnId = terminalStatus ? null : turn.id;
    model.status = terminalStatus === 'failed' ? 'failed' : terminalStatus ? 'idle' : 'running';
    markTranscriptModelChanged(model);
    return turn;
}
export function rollbackOptimisticCodexTurn(model, optimisticTurnId) {
    const index = model.turns.findIndex((turn) => turn.id === optimisticTurnId);
    if (index >= 0)
        model.turns.splice(index, 1);
    const active = [...model.turns].reverse().find((turn) => turn?.status === 'inProgress');
    model.activeTurnId = active?.id || null;
    model.status = active ? 'running' : 'idle';
    markTranscriptModelChanged(model);
}
export function applyCodexNotification(model, message) {
    const handled = applyCodexNotificationInternal(model, message);
    if (handled)
        markTranscriptModelChanged(model);
    return handled;
}
function applyCodexNotificationInternal(model, message) {
    const method = message?.method;
    const params = message?.params || {};
    if (!method)
        return false;
    if (method === 'turn/started') {
        upsertTurn(model, params.turn || {});
        model.activeTurnId = params.turn?.id || model.activeTurnId;
        model.status = 'running';
        model.error = null;
        return true;
    }
    if (method === 'turn/completed') {
        upsertTurn(model, params.turn || {});
        if (model.activeTurnId === params.turn?.id)
            model.activeTurnId = null;
        model.status = params.turn?.status === 'failed' ? 'failed' : 'idle';
        model.error = params.turn?.error?.message || null;
        return true;
    }
    if (method === 'item/started' || method === 'item/completed') {
        const turn = ensureTurn(model, params.turnId);
        upsertItem(turn, params.item || {});
        return true;
    }
    if (method === 'item/agentMessage/delta') {
        const item = ensureItem(model, params.turnId, params.itemId, 'agentMessage');
        item.text = `${item.text || ''}${params.delta || ''}`;
        return true;
    }
    if (method === 'item/plan/delta') {
        const item = ensureItem(model, params.turnId, params.itemId, 'plan');
        item.text = `${item.text || ''}${params.delta || ''}`;
        return true;
    }
    if (method === 'item/reasoning/summaryTextDelta') {
        const item = ensureItem(model, params.turnId, params.itemId, 'reasoning');
        if (!Array.isArray(item.summary))
            item.summary = [];
        const index = typeof params.summaryIndex === 'number' && Number.isInteger(params.summaryIndex) ? params.summaryIndex : 0;
        item.summary[index] = `${item.summary[index] || ''}${params.delta || ''}`;
        return true;
    }
    if (method === 'item/reasoning/textDelta') {
        const item = ensureItem(model, params.turnId, params.itemId, 'reasoning');
        if (!Array.isArray(item.content))
            item.content = [];
        const index = typeof params.contentIndex === 'number' && Number.isInteger(params.contentIndex) ? params.contentIndex : 0;
        item.content[index] = `${item.content[index] || ''}${params.delta || ''}`;
        return true;
    }
    if (method === 'item/commandExecution/outputDelta') {
        const item = ensureItem(model, params.turnId, params.itemId, 'commandExecution');
        item.aggregatedOutput = `${item.aggregatedOutput || ''}${params.delta || ''}`;
        return true;
    }
    if (method === 'turn/plan/updated') {
        const turn = ensureTurn(model, params.turnId);
        upsertItem(turn, {
            id: `studio-plan-${params.turnId || 'active'}`,
            type: 'planUpdate',
            explanation: params.explanation || '',
            plan: Array.isArray(params.plan) ? params.plan : [],
        });
        return true;
    }
    if (method === 'turn/diff/updated') {
        model.diff = params.diff || '';
        return true;
    }
    if (method === 'thread/tokenUsage/updated') {
        model.usage = params.tokenUsage || params.usage || params;
        return true;
    }
    if (method === 'thread/status/changed' && (!model.threadId || params.threadId === model.threadId)) {
        model.status = normalizeThreadStatus(params.status);
        return true;
    }
    if (method === 'error') {
        model.error = params.error?.message || params.message || 'Codex turn failed';
        model.status = 'failed';
        return true;
    }
    if (message.id != null && method.includes('/requestApproval')) {
        const existing = model.approvals.findIndex((approval) => approval.id === message.id);
        const approval = { id: message.id, method, params };
        if (existing >= 0)
            model.approvals[existing] = approval;
        else
            model.approvals.push(approval);
        return true;
    }
    if (message.id != null && (method === 'item/tool/requestUserInput' || method === 'mcpServer/elicitation/request')) {
        const existing = model.interactions.findIndex((interaction) => String(interaction.id) === String(message.id));
        const interaction = { id: message.id, method, params: structuredCloneSafe(params) };
        if (existing >= 0)
            model.interactions[existing] = interaction;
        else
            model.interactions.push(interaction);
        return true;
    }
    if (method === 'serverRequest/resolved') {
        model.approvals = model.approvals.filter((approval) => String(approval.id) !== String(params.requestId));
        model.interactions = model.interactions.filter((interaction) => String(interaction.id) !== String(params.requestId));
        return true;
    }
    return false;
}
export function selectedThreadStatusChange(message, selectedThreadId) {
    if (message?.method !== 'thread/status/changed')
        return null;
    if (String(message.params?.threadId || '') !== String(selectedThreadId || ''))
        return null;
    return message.params?.status ?? null;
}
export function resolveCodexApproval(model, requestId) {
    model.approvals = model.approvals.filter((approval) => String(approval.id) !== String(requestId));
}
export function resolveCodexInteraction(model, requestId) {
    model.interactions = model.interactions.filter((interaction) => String(interaction.id) !== String(requestId));
}
export function textFromUserContent(content) {
    if (!Array.isArray(content))
        return '';
    return content
        .filter((item) => item?.type === 'text')
        .map((item) => item.text || '')
        .join('\n');
}
function normalizeThreadStatus(status) {
    if (status?.type === 'active')
        return 'running';
    if (status?.type === 'systemError')
        return 'failed';
    return status?.type || 'idle';
}
function upsertTurn(model, incoming) {
    const id = incoming?.id || model.activeTurnId || `pending-${model.turns.length}`;
    const index = model.turns.findIndex((turn) => turn.id === id);
    if (index < 0) {
        model.turns.push({ id, status: incoming.status || 'inProgress', items: [], ...structuredCloneSafe(incoming) });
        return model.turns[model.turns.length - 1];
    }
    const current = model.turns[index];
    // index was found above; no asynchronous mutation occurs in this reducer.
    const next = structuredCloneSafe(incoming);
    model.turns[index] = {
        ...current,
        ...next,
        // App Server turn notifications are lifecycle snapshots, while item/*
        // notifications are the canonical item stream. A completion snapshot may
        // therefore be empty or partial and must never erase an earlier user input.
        items: mergeTurnItems(current.items, next.items),
    };
    return model.turns[index];
}
function mergeTurnItems(current, incoming) {
    const merged = Array.isArray(current) ? structuredCloneSafe(current) : [];
    if (!Array.isArray(incoming))
        return merged;
    const turn = { items: merged };
    for (const item of incoming)
        upsertItem(turn, item);
    return turn.items;
}
function ensureTurn(model, turnId) {
    const id = turnId || model.activeTurnId || `pending-${model.turns.length}`;
    let turn = model.turns.find((candidate) => candidate.id === id);
    if (!turn) {
        turn = { id, status: 'inProgress', items: [] };
        model.turns.push(turn);
    }
    if (!Array.isArray(turn.items))
        turn.items = [];
    return turn;
}
function upsertItem(turn, incoming) {
    const id = incoming?.id || `anonymous-${turn.items.length}`;
    let index = turn.items.findIndex((item) => item.id === id);
    if (index < 0 && incoming?.type === 'userMessage') {
        index = turn.items.findIndex((item) => sameUserMessage(item, incoming));
    }
    if (index < 0) {
        turn.items.push({ ...structuredCloneSafe(incoming), id });
        return turn.items[turn.items.length - 1];
    }
    const next = { ...turn.items[index], ...structuredCloneSafe(incoming), id };
    delete next.studioOptimistic;
    turn.items[index] = next;
    return turn.items[index];
}
function sameUserMessage(left, right) {
    if (left?.type !== 'userMessage' || right?.type !== 'userMessage')
        return false;
    if (left.clientId && right.clientId)
        return left.clientId === right.clientId;
    return Boolean(left.studioOptimistic || right.studioOptimistic)
        && sameUserContent(left.content, right.content);
}
function sameUserContent(left, right) {
    const leftItems = Array.isArray(left) ? left : [];
    const rightItems = Array.isArray(right) ? right : [];
    if (leftItems.length !== rightItems.length)
        return false;
    return leftItems.every((item, index) => {
        const other = rightItems[index];
        if (item?.type !== other?.type)
            return false;
        if (item?.type === 'text')
            return (item.text || '') === (other.text || '');
        if (item?.type === 'image')
            return (item.url || '') === (other.url || '');
        if (item?.type === 'localImage')
            return (item.path || '') === (other.path || '');
        if (item?.type === 'skill')
            return (item.name || '') === (other.name || '') && (item.path || '') === (other.path || '');
        if (item?.type === 'file')
            return (item.path || '') === (other.path || '') && (item.root || '') === (other.root || '');
        return true;
    });
}
function ensureItem(model, turnId, itemId, type) {
    const turn = ensureTurn(model, turnId);
    let item = turn.items.find((candidate) => candidate.id === itemId);
    if (!item) {
        item = { id: itemId || `stream-${turn.items.length}`, type };
        turn.items.push(item);
    }
    return item;
}
function structuredCloneSafe(value) {
    if (typeof structuredClone === 'function')
        return structuredClone(value);
    return JSON.parse(JSON.stringify(value));
}
