// Generated from ui-src; run npm run build:ui. Do not edit.
export function hydrateOpenCodeHistoryMetadata(thread, model, historyComplete = true) {
    model.messageTurns = thread?.messageTurns || {};
    model.messageRoles = thread?.messageRoles || {};
    model.messageItems = thread?.messageItems || {};
    model.messageErrors = thread?.messageErrors || {};
    model.historyComplete = historyComplete !== false;
    model.status = thread?.status || model.status;
    model.activeTurnId = model.status === 'running' ? model.turns.at(-1)?.id || null : null;
}
/** Synchronous installation after the caller's selection/connection checks. */
export function installSelectedHistory(options) {
    const { model, thread } = options;
    const installedTail = options.opencode && options.historyAnchorTurnId
        ? options.mergeTail(model, thread, options.historyAnchorTurnId)
        : false;
    if (!installedTail)
        options.hydrate(model, thread);
    if (options.opencode && !installedTail)
        options.hydrateMetadata(thread, model, options.historyComplete);
    if (options.historyEvents?.length)
        options.replay(model, options.historyEvents, {
            messageSnapshots: options.messageSnapshots,
            statusAfterSequence: options.statusSequence,
            authoritativeStatus: options.statusSequence !== undefined && options.statusSequence >= 0 ? thread?.status : null,
        });
    options.ready({ complete: installedTail ? model.historyComplete !== false : options.historyComplete !== false });
    options.mergeMetadata(thread);
    options.cache(model);
}
