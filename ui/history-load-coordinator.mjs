// Generated from ui-src; run npm run build:ui. Do not edit.
/** Start immediately, share only compatible flights, and never delete a newer one. */
export function coordinateHistoryLoad(loads, key, backend, historyEpoch, start) {
    const activeLoad = loads.get(key);
    if (activeLoad && (backend !== 'opencode' || activeLoad.historyEpoch === historyEpoch))
        return activeLoad;
    let load;
    load = start().finally(() => {
        if (loads.get(key) === load)
            loads.delete(key);
    });
    if (backend === 'opencode')
        load.historyEpoch = historyEpoch;
    loads.set(key, load);
    return load;
}
