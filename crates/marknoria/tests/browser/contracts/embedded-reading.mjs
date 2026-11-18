import { createReaderClient } from '/host-client.mjs'

const encoder = new TextEncoder()
async function segment(scope, text, quote, last = false) {
  const startUtf16 = last ? text.lastIndexOf(quote) : text.indexOf(quote)
  if (startUtf16 < 0) throw Error('Missing passage fixture')
  const hash = await crypto.subtle.digest('SHA-256', encoder.encode(text))
  const textHash = Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('')
  return { scope, startUtf16, endUtf16: startUtf16 + quote.length, textHash }
}
function fragment(html) {
  const template = document.createElement('template')
  template.innerHTML = html // Projections of generated test documents.
  return template.content
}

export async function embeddedReadingContract(urls, { keepMounted = false } = {}) {
  let assertions = 0
  const panes = []
  const check = (condition, message) => {
    if (!condition) throw Error(message)
    assertions++
  }
  const expectError = async (promise, code) => {
    const actual = await promise.then(() => null, error => typeof error.code === 'string' ? error.code : error.name)
    check(actual === code, `Expected ${code}, received ${actual}`)
  }
  async function mount(name, request) {
    const frame = document.createElement('iframe')
    const url = new URL(urls[name])
    url.searchParams.set('notes', 'host')
    url.searchParams.set('selectionMenu', 'host')
    if (request) url.searchParams.set('sourceFlow', request)
    frame.src = url
    frame.title = name
    frame.style.cssText = 'position:fixed;inset:18px auto auto 18px;width:640px;height:520px;border:0'
    document.body.append(frame)
    const client = createReaderClient(frame)
    const pane = { frame, client, command: (method, params, options) => client.request(method, params, options) }
    panes.push(pane)
    pane.owner = await client.ready()
    return pane
  }
  const makeAnchor = ({ owner }, location, segments) => ({
    version: 1, documentId: owner.documentId, revision: owner.revision, location,
    ...(segments ? { textRange: { version: 1, segments } } : {}),
  })
  const requestPassage = ({ command }, anchor, options) => command('navigatePassage', { anchor }, options)
  try {
    const small = await mount('small'), { command } = small
    const { capabilities, presentation, document: metadata } = small.owner
    check(presentation.notes === 'host' && !capabilities.notes, 'Fixture must use host-owned comments without Notes storage')
    check(capabilities.sourceReading?.version === 1 && capabilities.sourceReading.continuous && metadata.sourceLines > 200, 'Embedded Source did not apply the continuous default')
    check(capabilities.passageNavigation?.version === 1 && capabilities.passageNavigation.focusHandoff, 'Missing saved passage capability')
    const pages = (await command('projectionWindow', { start: 0, count: 5 })).pages
    const first = pages[0], next = pages[1]
    const quote = 'repeat 中文😀'
    const plain = fragment(first.html).textContent
    const later = fragment(next.html).textContent.trim().slice(0, 12)
    const primary = await segment({ kind: 'projection', page: first.page }, plain, quote, true)
    const secondary = await segment({ kind: 'projection', page: next.page }, fragment(next.html).textContent, later)
    const prose = makeAnchor(small, { kind: 'line', line: 3 }, [primary, secondary])
    await command('navigate', { location: { kind: 'line', line: metadata.sourceLines } })
    const hostDialog = document.createElement('dialog')
    hostDialog.textContent = 'Host comment'
    document.body.append(hostDialog)
    hostDialog.showModal()
    const saved = await requestPassage(small, prose)
    check(saved.precision === 'renderedText' && saved.view === 'preview' && saved.revision === small.owner.revision, 'Wrong passage completion')
    hostDialog.close()
    hostDialog.remove()
    small.frame.focus({ preventScroll: true })
    check((await command('focusPassage', { focusToken: saved.focusToken })).focused, 'Host could not transfer focus after success')
    // Observe the selection after both windows process queued focus/layout work.
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    const selected = await command('selection', { textOnly: true })
    check(selected?.quote === quote && selected?.textRange?.segments?.length === 1, 'Did not select the first saved segment')
    check(selected.textRange.segments[0].startUtf16 === primary.startUtf16, 'Resolved the wrong repeated quotation')
    await expectError(command('focusPassage', { focusToken: saved.focusToken }), 'STALE_FOCUS')
    await expectError(requestPassage(small, { ...prose, revision: 'a'.repeat(64) }), 'REVISION')
    await expectError(requestPassage(small, { ...prose, documentId: 'unrelated owner' }), 'REVISION')
    await expectError(requestPassage(small, { ...prose, textRange: undefined }), 'UNSUPPORTED_LOCATION')
    await expectError(requestPassage(small, { ...prose, location: { kind: 'queryCell', queryId: 'old', row: 0, column: 0 } }), 'UNSUPPORTED_LOCATION')
    await expectError(requestPassage(small, makeAnchor(small, prose.location, [{ ...primary, textHash: '0'.repeat(64) }])), 'PASSAGE_MISMATCH')
    await expectError(requestPassage(small, makeAnchor(small, prose.location, [{ ...primary, startUtf16: primary.endUtf16 - 1 }])), 'PARAMS')
    await expectError(requestPassage(small, makeAnchor(small, prose.location, [primary, { ...secondary, scope: { kind: 'projection', page: 999999 } }])), 'PARAMS')

    const reopened = await mount('reopened')
    check(reopened.owner.documentId !== small.owner.documentId && reopened.owner.resourceId === small.owner.resourceId, 'Reopened resource did not get a new document identity')
    await expectError(requestPassage(reopened, prose), 'REVISION')
    const durable = { ...prose, documentId: undefined, resourceId: small.owner.resourceId }
    const restored = await requestPassage(reopened, durable)
    check(restored.precision === 'renderedText', 'Stable resource anchor failed after reopening')
    reopened.frame.focus({ preventScroll: true })
    await reopened.command('focusPassage', { focusToken: restored.focusToken })
    check((await reopened.command('selection', { textOnly: true }))?.textRange?.segments[0]?.startUtf16 === primary.startUtf16, 'Reopening selected the wrong occurrence')
    await expectError(requestPassage(small, { ...prose, resourceId: 'unrelated resource' }), 'REVISION')

    await command('setView', { view: 'source' })
    await command('navigate', { location: { kind: 'line', line: metadata.sourceLines } })
    const eof = await command('state')
    check(eof.position.location.line > 200, `Continuous Source could not reach beyond 200 lines: ${JSON.stringify(eof.position)}`)
    const match = (await command('search', { query: 'End 中文😀' })).matches[0]
    const reference = makeAnchor(small, match.sourceLocation)
    check((await requestPassage(small, reference)).precision === 'sourceBytes', 'Source byte passage precision lost')
    check((await command('selection', { textOnly: true }))?.quote === 'End 中文😀', 'Source selected the wrong UTF-8 byte range')
    const sourceRange = await command('sourceWindow', { offset: match.sourceLocation.startByte, end: match.sourceLocation.endByte })
    check(sourceRange.result.text === 'End 中文😀', 'Source range transport changed original bytes')
    const abort = new AbortController()
    abort.abort()
    await expectError(requestPassage(small, prose, { signal: abort.signal }), 'AbortError')

    const forced = await mount('small', 'paged')
    check(forced.owner.capabilities.sourceReading.flow === 'paged' && !forced.owner.capabilities.sourceReading.continuous, 'Explicit paging policy was ignored')
    await forced.command('setView', { view: 'source' })
    const centered = await forced.command('navigate', { location: { kind: 'line', line: 210 } })
    check(centered.position.location.line >= 186 && centered.position.location.line < 210, 'Public Source jump lost preceding context')
    const firstLine = await forced.command('navigate', { location: { kind: 'line', line: 1 } })
    check(firstLine.position.location.line === 1, 'Source jump before start was not clamped')
    const lastLine = await forced.command('navigate', { location: { kind: 'line', line: metadata.sourceLines } })
    check(lastLine.position.location.line >= metadata.sourceLines - 24 && lastLine.position.location.line < metadata.sourceLines, 'Source jump at EOF lost preceding context')

    const table = await mount('table')
    check(!table.owner.capabilities.sourceReading.continuous && table.owner.document.sourceBytes < 128 * 1024, 'Large table was not detected')
    const result = (await table.command('tableWindow', { table: 0, row: 0, columns: [0, 1] })).result
    const cell = fragment(result.html).querySelector('tbody tr').cells[1]
    check(cell.dataset.cellPreview === 'true', 'Fixture did not exercise clipped table text')
    const cellLocation = { kind: 'cell', table: 0, row: 0, column: 1 }
    const cellSelector = await segment(cellLocation, cell.textContent, 'visible prefix')
    check((await requestPassage(table, makeAnchor(table, cellLocation, [cellSelector]))).precision === 'renderedText', 'Clipped cell lost the saved visible fragment')
    check((await table.command('selection', { textOnly: true }))?.quote === 'visible prefix', 'Clipped cell selected the wrong text')
    const omitted = (await table.command('search', { query: 'omitted-tail' })).matches[0].sourceLocation
    check((await requestPassage(table, makeAnchor(table, omitted))).view === 'source', 'Omitted cell text did not open in Source')
    check((await table.command('selection', { textOnly: true }))?.quote === 'omitted-tail', 'Omitted cell text did not round-trip')

    const long = await mount('long')
    const firstWindow = (await long.command('sourceWindow', { line: 2 })).result
    const following = (await long.command('sourceWindow', { offset: firstWindow.end })).result
    check(firstWindow.endsMidLine && following.startsMidLine && following.start === firstWindow.end, 'Long Unicode line did not continue at its byte boundary')
    check(!firstWindow.text.includes('\uFFFD') && !following.text.includes('\uFFFD'), 'Source split a Unicode scalar')

    const pdf = await mount('pdf')
    await pdf.command('settings', { pdf: { zoom: 2, continuous: false } })
    const region = { kind: 'pdf', pages: [{ page: 2, rects: [{ x: 0.88, y: 0.62, width: 0.06, height: 0.02 }] }] }
    const pdfResult = await requestPassage(pdf, makeAnchor(pdf, region))
    check(pdfResult.precision === 'pdfRegion' && (await pdf.command('state')).position.location.pages[0].page === 2, 'PDF region did not open at its target page')
    check(await pdf.command('selection', { textOnly: true }) === null, 'PDF geometry fabricated a text selection')
    await expectError(requestPassage(pdf, makeAnchor(pdf, { ...region, pages: [...region.pages, { page: 3, rects: [] }] })), 'PARAMS')
    const page = await requestPassage(pdf, makeAnchor(pdf, { kind: 'pdf', pages: [{ page: 2, rects: [] }] }))
    check(page.precision === 'pdfPage', 'Page-only PDF reference claimed region precision')
    await requestPassage(pdf, makeAnchor(pdf, region))

    const epub = await mount('epub')
    const cfi = (await epub.command('search', { query: 'EPUB sentinel two' })).matches[0].location
    const book = await requestPassage(epub, makeAnchor(epub, cfi))
    check(book.precision === 'epubCfi' && (await epub.command('state')).position.location.href === cfi.href, 'EPUB CFI lost its chapter')
    check(await epub.command('selection', { textOnly: true }) === null, 'CFI navigation fabricated a text selection')
    await checkLateFocusReset(check)
    if (keepMounted) window.readingFixture = { small, forced, table, long, pdf, epub, reopened, prose }
    return { assertions, source: true, passage: true, lateFocusReset: true }
  } finally {
    if (!keepMounted) await Promise.all(panes.map(async pane => {
      await pane.client.dispose({ force: true })
      pane.frame.remove()
    }))
  }
}

async function checkLateFocusReset(check) {
  const { focusNotePassage } = await import('/note-passage.mjs')
  const viewport = document.createElement('div')
  viewport.tabIndex = 0
  viewport.style.cssText = 'position:fixed;inset:0 auto auto 0;background:white;z-index:2'
  const node = document.createElement('p')
  node.textContent = 'Repeat 中文😀 / Repeat 中文😀'
  viewport.append(node)
  document.body.append(viewport)
  const text = node.firstChild, range = document.createRange(), selection = document.getSelection()
  range.setStart(text, node.textContent.lastIndexOf('Repeat'))
  range.setEnd(text, text.length)
  let resets = 0
  let current = true
  const reset = (interrupt = false) => viewport.addEventListener('focus', () => {
    requestAnimationFrame(() => {
      selection.removeAllRanges()
      resets++
      if (interrupt) current = false
    })
  }, { once: true })
  try {
    reset()
    viewport.focus()
    selection.addRange(range.cloneRange())
    await new Promise(resolve => requestAnimationFrame(resolve))
    check(resets === 1 && selection.isCollapsed, 'Fixture did not clear the immediate selection')
    viewport.blur()
    reset()
    check(await focusNotePassage({ node, range }, viewport, () => current), 'Focus handoff did not recover the delayed reset')
    check(resets === 2 && selection.anchorOffset === range.startOffset && selection.focusOffset === range.endOffset,
      'Focus handoff restored a different occurrence')
    viewport.blur()
    reset(true)
    check(!await focusNotePassage({ node, range }, viewport, () => current) && resets === 3 && selection.isCollapsed,
      'A superseded focus handoff reselected the old passage')
  } finally {
    selection.removeAllRanges()
    viewport.remove()
  }
}
