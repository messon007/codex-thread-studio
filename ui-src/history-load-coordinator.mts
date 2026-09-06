export type HistoryFlight<T> = Promise<T> & { historyEpoch?: number | null }

/** Start immediately, share only compatible flights, and never delete a newer one. */
export function coordinateHistoryLoad<T>(
  loads: Map<string, HistoryFlight<T>>, key: string, backend: string,
  historyEpoch: number | null, start: () => Promise<T>,
): HistoryFlight<T> {
  const activeLoad = loads.get(key)
  if (activeLoad && (backend !== 'opencode' || activeLoad.historyEpoch === historyEpoch)) return activeLoad
  let load: HistoryFlight<T>
  load = start().finally(() => {
    if (loads.get(key) === load) loads.delete(key)
  })
  if (backend === 'opencode') load.historyEpoch = historyEpoch
  loads.set(key, load)
  return load
}
