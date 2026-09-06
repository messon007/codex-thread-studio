// Generated from ui-src; run npm run build:ui. Do not edit.
export function reduceOpenCodeEvent(model, event, selectedSessionId, helpers) {
    const { normalizeOpenCodeStatus, errorText, ensureTurn, structuredOutputItem, upsertItem, upsertOpenCodeUserPart, openCodePartToItem, rememberMessageItem, forgetMessageItem, refreshTurnMessageError } = helpers;
    const payload = event?.payload || event;
    const type = payload?.type;
    const properties = payload?.properties || {};
    const sessionId = properties.sessionID || properties.sessionId || properties.info?.sessionID || properties.part?.sessionID;
    if (sessionId && selectedSessionId && sessionId !== selectedSessionId)
        return { handled: false, sessionId, type };
    if (type === 'session.status') {
        model.status = normalizeOpenCodeStatus(properties.status);
        if (model.status === 'running') {
            model.activeTurnId ||= model.turns.at(-1)?.id || null;
            return { handled: true, kind: 'metadata', sessionId, type };
        }
        const active = model.turns.find((turn) => turn.id === model.activeTurnId) || model.turns.at(-1);
        const turnId = model.activeTurnId || active?.id || null;
        model.activeTurnId = null;
        if (active?.status === 'inProgress')
            active.status = model.status === 'failed' ? 'failed' : 'completed';
        return { handled: true, kind: turnId ? 'full' : 'metadata', sessionId, type, turnId };
    }
    if (type === 'session.idle') {
        model.status = 'idle';
        const active = model.turns.at(-1);
        const turnId = model.activeTurnId || active?.id || null;
        model.activeTurnId = null;
        if (active?.status === 'inProgress')
            active.status = 'completed';
        return { handled: true, kind: 'full', sessionId, type, turnId };
    }
    if (type === 'session.error') {
        model.error = errorText(properties.error || properties);
        model.status = 'failed';
        model.activeTurnId = null;
        return { handled: true, kind: 'full', sessionId, type };
    }
    if (type === 'message.updated') {
        const info = properties.info || {};
        let turn = null;
        model.messageRoles ||= {};
        if (info.id)
            model.messageRoles[info.id] = info.role;
        if (info.role === 'user') {
            turn = ensureTurn(model, info.id);
            model.messageTurns ||= {};
            model.messageTurns[String(info.id)] = turn.id;
            model.activeTurnId = turn.id;
            model.status = 'running';
        }
        else if (info.id) {
            turn = ensureTurn(model, info.parentID || model.activeTurnId);
            model.messageTurns ||= {};
            model.messageTurns[String(info.id)] = turn.id;
            if (info.structured !== undefined) {
                const item = structuredOutputItem(info);
                upsertItem(turn, item);
                model.messageItems ||= {};
                rememberMessageItem(model.messageItems, info.id, item.id);
            }
            if (info.error) {
                const message = errorText(info.error);
                turn.status = 'failed';
                turn.error = { message };
                model.messageErrors ||= {};
                model.messageErrors[info.id] = { turnId: turn.id, message };
            }
        }
        return { handled: true, kind: 'full', sessionId, type, turnId: turn?.id || null };
    }
    if (type === 'message.part.updated') {
        const part = properties.part || {};
        const turn = ensureTurn(model, model.messageTurns?.[String(part.messageID)] || model.activeTurnId);
        const role = model.messageRoles?.[String(part.messageID)] || 'assistant';
        if (role === 'user')
            upsertOpenCodeUserPart(turn, part);
        else
            upsertItem(turn, openCodePartToItem(part, role));
        model.messageTurns ||= {};
        if (part.messageID)
            model.messageTurns[String(part.messageID)] = turn.id;
        model.messageItems ||= {};
        rememberMessageItem(model.messageItems, part.messageID, role === 'user' ? part.messageID : part.id);
        return { handled: true, kind: 'full', sessionId, type, turnId: turn.id };
    }
    if (type === 'message.part.delta') {
        const turn = ensureTurn(model, model.messageTurns?.[String(properties.messageID)] || model.activeTurnId);
        let item = turn.items.find((candidate) => candidate.id === properties.partID);
        if (!item) {
            item = { id: properties.partID, type: properties.field === 'text' ? 'agentMessage' : 'unknown' };
            turn.items.push(item);
        }
        const field = properties.field || 'text';
        if (item.type === 'reasoning') {
            item.content = [`${item.content?.[0] || ''}${properties.delta || ''}`];
        }
        else {
            item[field] = `${item[field] || ''}${properties.delta || ''}`;
        }
        model.messageTurns ||= {};
        if (properties.messageID)
            model.messageTurns[String(properties.messageID)] = turn.id;
        model.messageItems ||= {};
        rememberMessageItem(model.messageItems, properties.messageID, item.id);
        return { handled: true, kind: 'stream', sessionId, type, turnId: turn.id, itemId: item.id };
    }
    if (type === 'message.part.removed') {
        const messageId = String(properties.messageID || '');
        const itemId = String(properties.partID || '');
        const turn = model.turns.find((candidate) => String(candidate.id || '') === String(model.messageTurns?.[messageId] || ''))
            || model.turns.find((candidate) => (candidate.items || []).some((item) => String(item.id || '') === itemId));
        const role = model.messageRoles?.[messageId];
        if (turn && itemId && role === 'user') {
            const item = (turn.items || []).find((candidate) => candidate.type === 'userMessage');
            if (item?.studioUserParts) {
                delete item.studioUserParts[itemId];
                item.content = Object.values(item.studioUserParts).flat();
            }
        }
        else if (turn && itemId) {
            turn.items = (turn.items || []).filter((item) => String(item.id || '') !== itemId);
        }
        forgetMessageItem(model.messageItems, messageId, itemId);
        return { handled: true, kind: 'full', sessionId, type, turnId: turn?.id || null };
    }
    if (type === 'message.removed') {
        const messageId = String(properties.messageID || '');
        const turnId = String(model.messageTurns?.[messageId] || model.messageErrors?.[messageId]?.turnId || '');
        const role = model.messageRoles?.[messageId];
        const itemIds = new Set(model.messageItems?.[messageId] || []);
        const turn = model.turns.find((candidate) => String(candidate.id || '') === turnId)
            || model.turns.find((candidate) => (candidate.items || []).some((item) => itemIds.has(String(item.id || ''))));
        if (turn && (role === 'user' || String(turn.id || '') === messageId)) {
            model.turns = model.turns.filter((candidate) => candidate !== turn);
            for (const [id, mappedTurnId] of Object.entries(model.messageTurns || {})) {
                if (String(mappedTurnId || '') === turnId) {
                    delete model.messageTurns?.[id];
                    delete model.messageRoles?.[id];
                    delete model.messageItems?.[id];
                    delete model.messageErrors?.[id];
                }
            }
            if (model.activeTurnId === turn.id)
                model.activeTurnId = null;
        }
        else if (turn && itemIds.size) {
            turn.items = (turn.items || []).filter((item) => !itemIds.has(String(item.id || '')));
            if (!turn.items.length) {
                model.turns = model.turns.filter((candidate) => candidate !== turn);
                if (model.activeTurnId === turn.id)
                    model.activeTurnId = null;
            }
        }
        delete model.messageTurns?.[messageId];
        delete model.messageRoles?.[messageId];
        delete model.messageItems?.[messageId];
        delete model.messageErrors?.[messageId];
        if (turn && model.turns.includes(turn))
            refreshTurnMessageError(model, turn);
        return { handled: true, kind: 'full', sessionId, type, turnId: turnId || null };
    }
    if (type === 'permission.asked') {
        const id = properties.id || properties.requestID || properties.permissionID;
        if (id && !model.approvals.some((approval) => String(approval.id) === String(id))) {
            model.approvals.push({ id, method: 'opencode/permission', params: properties });
        }
        return { handled: true, kind: 'full', sessionId, type };
    }
    if (type === 'permission.replied') {
        const id = properties.id || properties.requestID || properties.permissionID;
        model.approvals = model.approvals.filter((approval) => String(approval.id) !== String(id));
        return { handled: true, kind: 'full', sessionId, type };
    }
    if (type === 'session.diff') {
        model.diff = typeof properties.diff === 'string' ? properties.diff : JSON.stringify(properties.diff || properties, null, 2);
        return { handled: true, kind: 'metadata', sessionId, type };
    }
    return { handled: false, sessionId, type };
}
