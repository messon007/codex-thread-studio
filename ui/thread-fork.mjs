// Generated from ui-src; run npm run build:ui. Do not edit.
export function threadForkParams(threadId, lastTurnId = null) {
    const params = { threadId: String(threadId || '') };
    if (lastTurnId)
        params.lastTurnId = String(lastTurnId);
    return params;
}
export function openCodeForkBody(params = {}) {
    return params.lastTurnId ? { messageID: String(params.lastTurnId) } : {};
}
export function isTurnForkable(turn) {
    return Boolean(turn?.id
        && !turn.studioOptimistic
        && turn.status
        && turn.status !== 'inProgress'
        && turn.status !== 'unknown');
}
