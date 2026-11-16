import type { HistoryTurn, TurnPage, TurnTailOptions } from './session-types.mjs'

export const DEFAULT_TURN_TAIL_PAGE_SIZE = 4
export const MAX_TURN_TAIL_PAGES = 100

// Full history is still displayed, but never requested as one unbounded RPC.
// A finite cursor walk also protects against a broken server producing endless pages.
export async function collectCodexTurnHistory<T extends HistoryTurn>({
  initialPage, fetchPage, maxPages = 10_000, maxBytes = 256 * 1024 * 1024,
}: Omit<TurnTailOptions<T>, 'cachedTurns'> & { maxBytes?: number }) {
  const turns: T[] = []
  const ids = new Set<string>()
  const cursors = new Set<string>()
  let page = initialPage
  let pageCount = 0
  let bytes = 0
  const encoder = new TextEncoder()
  while (true) {
    if (!page || !Array.isArray(page.data)) throw new Error('Malformed Codex history page')
    bytes += encoder.encode(JSON.stringify(page)).byteLength
    if (bytes > maxBytes) throw new Error('Codex history pagination exceeded its byte budget')
    pageCount++
    for (const turn of page.data) {
      const id = String(turn?.id || '')
      if (!id) throw new Error('Codex history turn has no identity')
      // Descending pages can overlap while a running turn is being updated.
      // Retain the first (newest) authoritative copy.
      if (!ids.has(id)) { ids.add(id); turns.push(turn) }
    }
    const cursor = stringOrNull(page.nextCursor)
    if (!cursor) return { turns: turns.reverse(), pageCount, fetchedTurnCount: turns.length, appendedTurnCount: turns.length }
    if (cursors.has(cursor)) throw new Error('Codex history pagination repeated a cursor')
    if (pageCount >= maxPages) throw new Error('Codex history pagination exceeded its page budget')
    if (!fetchPage) throw new Error('Codex history pagination cannot continue')
    cursors.add(cursor)
    page = await fetchPage(cursor)
  }
}

export async function collectCodexTurnTail<T extends HistoryTurn>({
  cachedTurns,
  initialPage,
  fetchPage,
  maxPages = MAX_TURN_TAIL_PAGES,
}: TurnTailOptions<T>) {
  const existing: T[] = Array.isArray(cachedTurns) ? [...cachedTurns] : []
  const knownIndexes = new Map<string, number>()
  existing.forEach((turn, index) => {
    const id = String(turn?.id || '')
    if (id) knownIndexes.set(id, index)
  })

  const receivedDescending: T[] = []
  const receivedIds = new Set<string>()
  const seenCursors = new Set<string>()
  let page = normalizePage(initialPage)
  let pageCount = 0

  while (page && pageCount < Math.max(1, Number(maxPages) || MAX_TURN_TAIL_PAGES)) {
    pageCount += 1
    for (const turn of page.data) {
      const id = String(turn?.id || '')
      if (!id || receivedIds.has(id)) continue
      receivedIds.add(id)
      receivedDescending.push(turn)
      const cachedIndex = knownIndexes.get(id)
      if (cachedIndex == null) continue
      const tail = receivedDescending.slice().reverse()
      return {
        matched: true,
        anchorTurnId: id,
        turns: [...existing.slice(0, cachedIndex), ...tail],
        pageCount,
        fetchedTurnCount: receivedDescending.length,
        appendedTurnCount: tail.filter((candidate) => !knownIndexes.has(String(candidate?.id || ''))).length,
      }
    }

    const cursor = stringOrNull(page.nextCursor)
    if (!cursor || seenCursors.has(cursor) || typeof fetchPage !== 'function') break
    seenCursors.add(cursor)
    page = normalizePage(await fetchPage(cursor))
  }

  if (!existing.length && !receivedDescending.length && !page?.nextCursor) {
    return {
      matched: true,
      anchorTurnId: '',
      turns: [],
      pageCount,
      fetchedTurnCount: 0,
      appendedTurnCount: 0,
    }
  }

  return {
    matched: false,
    anchorTurnId: '',
    turns: existing,
    pageCount,
    fetchedTurnCount: receivedDescending.length,
    appendedTurnCount: 0,
  }
}

export function isHistoryPaginationCompatibilityError(error: unknown) {
  const message = String((error && typeof error === 'object' && 'message' in error ? error.message : undefined) || error || '')
  return /(?:method (?:not found|unknown)|unknown (?:rpc )?method|unsupported(?: method)?|not supported|invalid params|unknown field|excludeTurns|initialTurnsPage|thread\/turns\/list[^\n]*(?:not found|unknown|unsupported|not supported))/iu.test(message)
}

function normalizePage<T extends HistoryTurn>(value: TurnPage<T> | null | undefined) {
  if (!value || !Array.isArray(value.data)) return null
  return {
    data: value.data,
    nextCursor: stringOrNull(value.nextCursor),
    backwardsCursor: stringOrNull(value.backwardsCursor),
  }
}

function stringOrNull(value: unknown) {
  const text = String(value || '')
  return text || null
}
