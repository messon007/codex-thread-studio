// PDF destinations stay renderer-private. Listing labels does no page lookup;
// compatibility callers resolve at most the requested 256 entries.
export function createPdfOutline({ document: current, ensure, bounded, pause }) {
  let loaded = null
  const locations = new Map()
  function items() {
    ensure()
    if (!loaded) loaded = (async () => {
      const roots = await bounded(current().getOutline(), 'PDF outline')
      ensure()
      if ((roots?.length || 0) > 2000) throw Error('PDF outline exceeds 2,000 entries')
      const queue = (roots || []).map(item => ({ item, level: 1 })).reverse(), rows = []
      while (queue.length) {
        ensure()
        if (rows.length >= 2000 || queue.length > 2000) throw Error('PDF outline exceeds 2,000 entries')
        const { item, level } = queue.pop()
        rows.push({ id: rows.length, level: Math.min(12, level), label: String(item.title || '').slice(0, 1024), destination: item.dest })
        if ((item.items?.length || 0) + queue.length + rows.length > 2000) throw Error('PDF outline exceeds 2,000 entries')
        for (const child of (item.items || []).toReversed()) queue.push({ item: child, level: level + 1 })
        if (rows.length % 256 === 0) await pause()
      }
      ensure()
      return rows
    })().catch(error => { loaded = null; throw error })
    return loaded
  }
  async function location(id, signal) {
    ensure(); signal?.throwIfAborted()
    const rows = await items()
    ensure(); signal?.throwIfAborted()
    if (!Number.isSafeInteger(id) || id < 0 || id >= rows.length) throw Error('Invalid PDF outline identity')
    if (locations.has(id)) return locations.get(id)
    let destination = rows[id].destination
    if (typeof destination === 'string') destination = await bounded(current().getDestination(destination), 'PDF outline destination')
    ensure(); signal?.throwIfAborted()
    let page = null
    if (Array.isArray(destination) && destination[0] != null) page = typeof destination[0] === 'number'
      ? destination[0] + 1 : (await bounded(current().getPageIndex(destination[0]), 'PDF outline page')) + 1
    ensure(); signal?.throwIfAborted()
    const value = Number.isSafeInteger(page) && page > 0 && page <= current().numPages
      ? { kind: 'pdf', pages: [{ page, rects: [{ x: 0, y: 0, width: 1, height: 1 }] }] } : null
    locations.set(id, value)
    return value
  }
  return {
    location,
    async page({ offset = 0, resolve = true, signal } = {}) {
      if (!Number.isSafeInteger(offset) || offset < 0 || typeof resolve !== 'boolean') throw Error('Invalid PDF outline request')
      ensure(); signal?.throwIfAborted()
      const rows = await items(), result = []
      ensure(); signal?.throwIfAborted()
      for (const row of rows.slice(offset, offset + 256)) {
        const item = { id: row.id, level: row.level, label: row.label, navigable: row.destination != null }
        if (resolve) item.location = await location(row.id, signal)
        result.push(item)
      }
      return { offset, total: rows.length, items: result }
    },
    dispose() { loaded = null; locations.clear() }
  }
}
