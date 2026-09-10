// Generated from ui-src; run npm run build:ui. Do not edit.
/** Only the controller session's latest routed turn can require attention. */
export function routerAttentionEntries(dispatches, controllers, controller, latestTurnKey) {
    const entry = dispatches.get(latestTurnKey);
    return entry?.decision?.targetSessionKey
        && controllers.get(latestTurnKey) === controller
        && entry.unread
        && ['completed', 'failed'].includes(entry.status || '')
        ? [{ key: latestTurnKey, entry }]
        : [];
}
export function responseIsVisible(rect, viewport) {
    const overlap = Math.min(rect.bottom, viewport.bottom) - Math.max(rect.top, viewport.top);
    return rect.height > 0 && overlap >= Math.min(48, rect.height);
}
