// Public host only: no reader implementation, child DOM, or diagnostic globals.
import { createReaderClient } from '/host-client.mjs'
export async function mountSearchPane(url, title = 'Search pane') {
  const frame = document.createElement('iframe')
  frame.title = title
  frame.src = url
  frame.style.cssText = 'width:740px;height:720px;border:0;display:inline-block'
  document.body.append(frame)
  const events = []
  const client = createReaderClient(frame, { onEvent: (type, value) => {
    if (type === 'state') { events.push(value); if (events.length > 100) events.shift() }
  } })
  const initial = await client.ready()
  await client.request('settings', { toolbar: 'host', notes: 'host', selectionMenu: 'host' })
  return { frame, client, initial, events }
}
export async function embeddedSearchContract(url, { keepMounted = false } = {}) {
  const panes = [], checks = []
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const reject = async (promise, code) => {
    const error = await promise.then(() => null, error => error)
    check(error && (!code || error.code === code), `Expected rejection ${code || ''}, got ${error?.code || 'success'}`)
  }
  try {
    panes.push(await mountSearchPane(url, 'Search A'), await mountSearchPane(url, 'Search B'))
    const [a, b] = panes.map(pane => pane.client)
    const ready = panes[0].initial
    check(ready.capabilities.search.version === 1 && ready.capabilities.search.activation === true, 'Public search capability absent')
    check(ready.capabilities.search.windowLimit === 100 && ready.capabilities.search.pagination === 'source' && ready.capabilities.search.resultLimit === null, 'Source paging capability is inaccurate')
    check(ready.capabilities.queries === false, 'Search expanded query authority')
    check((await a.request('state')).presentation.toolbar === 'host', 'Host does not own toolbar')
    const first = await a.request('search', { query: 'needle' })
    check(first.version === 1 && first.total === 250 && first.available === 250 && first.totalExact === true, 'Search did not return complete source count')
    check(first.offset === 0 && first.limit === 100 && first.matches.length === 100 && !first.complete, 'First source window is unbounded or falsely complete')
    check(first.matches.every((match, i) => match.index === i && match.occurrence === i && match.sourceLocation.kind === 'source'), 'Same-line identity/byte ranges lost')
    check(first.matches[0].location.kind === 'line', 'Legacy typed line location lost')
    check((await a.request('state')).search.current === null, 'Reading search results navigated without activation')
    const second = await a.request('search', { query: 'needle', offset: 100 })
    check(second.searchId === first.searchId && second.total === 250 && second.matches.length === 100 && second.matches[0].index === 100 && second.matches[0].occurrence === 100, 'Second window lost query identity or absolute same-line ordinal')
    const final = await a.request('search', { query: 'needle', offset: 200 })
    check(final.searchId === first.searchId && final.matches.length === 50 && final.complete && final.matches.at(-1).index === 249, 'Last window inaccessible')
    const end = await a.request('search', { query: 'needle', offset: 250 })
    check(end.matches.length === 0 && end.complete && end.total === 250, 'End window changed count')
    checks.push('exact-count-and-bounded-windows')
    for (const index of [0, 1, 99, 100, 129, 130, 249, 0, 249]) {
      const state = await a.request('searchActivate', { searchId: first.searchId, index })
      check(state.search.current === index && state.search.total === 250 && state.search.active.index === index, `Activation ${index} lost current/total`)
      check(state.search.active.sourceLocation.startByte < state.search.active.sourceLocation.endByte, 'Activated byte range is invalid')
    }
    checks.push('activation-page-boundaries-and-wrap')
    const peer = await b.request('search', { query: 'Paragraph' })
    await b.request('searchActivate', { searchId: peer.searchId, index: 2 })
    check(peer.total === 120 && peer.searchId !== first.searchId, 'Independent pane shared query state')
    await reject(b.request('searchActivate', { searchId: first.searchId, index: 0 }), 'SEARCH_ID')
    await a.request('searchClear')
    check((await a.request('state')).search.current === null && (await b.request('state')).search.current === 2, 'Clearing one pane changed another')
    await reject(a.request('searchActivate', { searchId: first.searchId, index: 0 }), 'SEARCH_ID')
    checks.push('independent-pane-ownership')
    const selected = await a.request('search', { query: 'needle' })
    await a.request('searchActivate', { searchId: selected.searchId, index: 101 })
    await a.request('setView', { view: 'source' })
    let state = await a.request('searchActivate', { searchId: selected.searchId, index: 102 })
    check(state.view === 'source' && state.search.current === 102, 'Source activation lost search identity')
    await a.request('setView', { view: 'preview' })
    state = await a.request('searchActivate', { searchId: selected.searchId, index: 103 })
    check(state.view === 'preview' && state.search.current === 103, 'Preview activation lost search identity')
    checks.push('preview-source')
    for (const params of [{ query: 'needle', offset: null }, { query: 'needle', offset: -1 }, { query: 'needle', offset: 0.5 }, { query: 'needle', offset: Number.MAX_SAFE_INTEGER + 1 }, { query: 'needle', offset: '100' }, { query: 'needle', page: 1 }, { query: 'needle', limit: 1000 }, { query: '界'.repeat(86) }]) await reject(a.request('search', params))
    await reject(a.request('search', { query: 'needle', offset: 251 }), 'RANGE')
    await reject(a.request('search', { query: 'needle', revision: 'stale' }), 'REVISION')
    await reject(a.request('search', { query: 'needle', documentId: 'foreign' }), 'REVISION')
    await reject(a.request('searchActivate', { searchId: selected.searchId, index: 0, location: { kind: 'line', line: 1 } }))
    await reject(a.request('searchActivate', { searchId: selected.searchId, index: 250 }), 'RANGE')
    await reject(a.request('searchActivate', { searchId: selected.searchId, index: 0, revision: 'stale' }), 'REVISION')
    checks.push('parameter-and-revision-validation')
    const empty = await a.request('search', { query: '' })
    state = await a.request('state')
    check(state.search.current === null && state.search.query === '' && state.search.total === 0, 'Empty query failed to clear')
    const none = await a.request('search', { query: 'not-in-the-document' })
    check(none.total === 0 && none.totalExact && none.matches.length === 0, 'Zero-match query is not exact')
    const controller = new AbortController(); controller.abort()
    await reject(a.request('search', { query: 'needle' }, { signal: controller.signal }))
    check(ready.capabilities.search.matchCase === true && ready.capabilities.search.defaultMatchCase === true, 'Match-case capability/default missing')
    const strictCase = await a.request('search', { query: 'NEEDLE', matchCase: true })
    check(strictCase.total === 0 && strictCase.matchCase === true, 'Strict search ignored case')
    const foldedCase = await a.request('search', { query: 'NEEDLE', matchCase: false, offset: 100 })
    check(foldedCase.total === 250 && foldedCase.matches.length === 100 && foldedCase.matchCase === false, 'Insensitive search lost count/window')
    state = await a.request('searchActivate', { searchId: foldedCase.searchId, index: 129 })
    check(state.search.current === 129 && state.search.matchCase === false, 'Activation lost resolved match case')
    const foldedEnd = await a.request('search', { query: 'NEEDLE', matchCase: false, offset: 200 })
    check(foldedEnd.searchId === foldedCase.searchId && foldedEnd.matches.length === 50, 'Paging did not retain case identity')
    const defaultCase = await a.request('search', { query: 'NEEDLE' })
    check(defaultCase.total === 0 && defaultCase.matchCase === true && defaultCase.searchId !== foldedCase.searchId, 'Omitted option did not resolve advertised default')
    await reject(a.request('searchActivate', { searchId: foldedCase.searchId, index: 0 }), 'SEARCH_ID')
    for (const matchCase of [null, 'false', 0, {}]) await reject(a.request('search', { query: 'needle', matchCase }), 'PARAMS')
    check((await b.request('state')).search.current === 2, 'Changing match case affected another pane')
    await a.request('searchClear')
    check((await a.request('state')).search.matchCase === null, 'Clear retained an owned match-case option')
    checks.push('configurable-case-defaults-paging-identity')
    const oldId = selected.searchId
    await a.dispose(); panes[0].frame.remove()
    panes[0] = await mountSearchPane(url, 'Search A remounted')
    check((await panes[0].client.request('state')).search.current === null, 'Remount inherited search selection')
    await reject(panes[0].client.request('searchActivate', { searchId: oldId, index: 0 }), 'SEARCH_ID')
    check((await b.request('state')).search.current === 2, 'Other pane lost state on disposal/remount')
    checks.push('clear-zero-abort-disposal-remount')
    if (keepMounted) window.searchTestHost = { panes, mount: mountSearchPane }
    return { assertions, checks, publicProtocolOnly: true }
  } finally {
    if (!keepMounted) for (const pane of panes) { await pane.client.dispose().catch(() => {}); pane.frame.remove() }
  }
}
