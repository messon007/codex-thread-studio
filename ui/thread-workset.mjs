// Generated from ui-src; run npm run build:ui. Do not edit.
import { catalogTimestamp, threadCatalogKey } from './thread-catalog.mjs';
export function addLoadedThread(loadedThreads, backend, id) {
    if (!id)
        return false;
    const key = threadCatalogKey(backend, id);
    if (loadedThreads.has(key))
        return false;
    loadedThreads.add(key);
    return true;
}
export function preserveCatalogActivity(threads, previousThreads = []) {
    const previousById = new Map((previousThreads || []).map((thread) => [thread.id, thread]));
    return (threads || []).map((thread) => {
        const previous = previousById.get(thread.id);
        if (catalogTimestamp(previous?.activityAt) <= catalogTimestamp(thread?.activityAt))
            return thread;
        return { ...thread, activityAt: previous.activityAt };
    });
}
export function updateLoadedCatalogTimestamp(catalogs, loadedThreads, backend, id, timestamp) {
    if (!loadedThreads.has(threadCatalogKey(backend, id)))
        return false;
    return Boolean(updateCatalogThreadActivity(catalogs, backend, id, { timestamp }));
}
export function updateCatalogThreadActivity(catalogs, backend, id, options = {}) {
    const thread = (catalogs[backend] || []).find((candidate) => candidate.id === id);
    if (!thread)
        return null;
    const hasStatus = Object.hasOwn(options, 'status');
    const hasTimestamp = Object.hasOwn(options, 'timestamp');
    if (!hasStatus && !hasTimestamp)
        return null;
    const change = {
        backend,
        id,
        thread,
        hadStatus: Object.hasOwn(thread, 'status'),
        previousStatus: thread.status,
        hadActivityAt: Object.hasOwn(thread, 'activityAt'),
        previousActivityAt: thread.activityAt,
        hasStatus,
        appliedStatus: options.status,
        hasTimestamp,
        appliedTimestamp: options.timestamp,
        statusChanged: hasStatus && !Object.is(thread.status, options.status),
        timestampChanged: hasTimestamp && !Object.is(thread.activityAt, options.timestamp),
    };
    if (hasStatus)
        thread.status = options.status;
    if (hasTimestamp)
        thread.activityAt = options.timestamp;
    return change;
}
export function restoreCatalogThreadActivity(catalogs, change) {
    if (!change?.backend || !change?.id)
        return null;
    const thread = (catalogs[change.backend] || []).find((candidate) => candidate.id === change.id);
    if (!thread)
        return null;
    if (change.hasStatus && !Object.is(thread.status, change.appliedStatus))
        return null;
    if (change.hasTimestamp && !Object.is(thread.activityAt, change.appliedTimestamp))
        return null;
    if (change.hasStatus) {
        if (change.hadStatus)
            thread.status = change.previousStatus;
        else
            delete thread.status;
    }
    if (change.hasTimestamp) {
        if (change.hadActivityAt)
            thread.activityAt = change.previousActivityAt;
        else
            delete thread.activityAt;
    }
    return {
        backend: change.backend,
        id: change.id,
        thread,
        statusChanged: change.statusChanged,
        timestampChanged: change.timestampChanged,
    };
}
