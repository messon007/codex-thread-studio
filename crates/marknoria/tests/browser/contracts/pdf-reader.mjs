export async function pdfReaderContract(moduleUrl) {
  const { mountPdfReader } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const translate = createTranslator()
  const viewport = document.createElement('div')
  viewport.style.cssText =
    'position:fixed;inset:10px;width:1000px;height:700px;overflow:auto;background:#eee'
  const root = document.createElement('main')
  viewport.append(root)
  document.body.append(viewport)
  const events = [],
    selections = []
  const reader = mountPdfReader(root, {
    viewport,
    translate,
    url: new URL('/fixture.pdf', location.href).href,
    documentId: 'public-pdf',
    revision: 'a'.repeat(64),
    resourceId: 'public-resource',
    onState: (value) => events.push(value),
    onSelection: (value) => selections.push(value)
  })
  let assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const until = async (condition) => {
    const deadline = performance.now() + 30000
    while (!condition() && performance.now() < deadline) {
      if (reader.state().error) throw Error(reader.state().error)
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
    check(condition(), 'PDF did not reach its expected state')
  }
  const locationFor = (page) => ({
    kind: 'pdf',
    pages: [{ page, rects: [{ x: 0, y: 0, width: 1, height: 1 }] }]
  })
  try {
    check((await reader.ready).pages === 12, 'PDF page count changed')
    await until(() => root.querySelector('[data-pdf-page="1"][data-state="ready"] .textLayer span'))
    check(root.querySelectorAll('.pdf-controls').length === 1, 'PDF has per-page zoom controls')
    const first = root.querySelector('[data-pdf-page="1"]')
    check(first.clientWidth >= 900, 'Fit width left most of the reading area unused')
    const span = [...first.querySelectorAll('.textLayer span')].find((span) =>
      span.textContent.includes('sentinel')
    )
    check(Boolean(span), 'PDF text layer was absent')
    const range = document.createRange()
    range.selectNodeContents(span)
    getSelection().removeAllRanges()
    getSelection().addRange(range)
    const selected = reader.selection()
    check(
      selected.documentId === 'public-pdf' &&
        selected.revision === 'a'.repeat(64) &&
        selected.precision === 'exact',
      'PDF selection lost its owner'
    )
    check(
      selected.quote.includes('sentinel page 1') && selected.location.pages[0].page === 1,
      'PDF selection has the wrong page/text'
    )
    check(
      selected.location.pages[0].rects.every(
        (rect) =>
          rect.x >= 0 &&
          rect.y >= 0 &&
          rect.x + rect.width <= 1.000001 &&
          rect.y + rect.height <= 1.000001
      ),
      'PDF selection rectangles are not normalized'
    )
    const canvas = first.querySelector('canvas'),
      eventCount = events.length
    translate.update({
      locale: 'fr',
      messages: {
        Previous: 'Précédent',
        'Fit width': 'Adapter la largeur',
        'PDF page': 'Page PDF',
        'Page {page}': 'Feuille {page}',
        'Page {page} text': 'Texte feuille {page}',
        'Continuous reading': 'Lecture continue',
        '/ {pages}': 'sur {pages}',
        'PDF sentinel page 1': 'WRONG DATA'
      }
    })
    check(
      root.querySelector('.pdf-controls button').textContent === 'Précédent' &&
        first.getAttribute('aria-label') === 'Feuille 1' &&
        first.querySelector('.textLayer').getAttribute('aria-label') === 'Texte feuille 1',
      'PDF page or control labels did not update'
    )
    check(
      root.querySelector('.pdf-controls input[type=number]').getAttribute('aria-label') ===
        'Page PDF' &&
        root.querySelector('.pdf-controls label span').textContent === 'Lecture continue',
      'PDF input/flow labels did not update'
    )
    check(
      first.querySelector('canvas') === canvas &&
        reader.selection().quote === selected.quote &&
        events.length === eventCount,
      'PDF locale update redrew content, changed selection or emitted renderer state'
    )
    const boundary = range.cloneRange()
    boundary.setEndAfter(first)
    getSelection().removeAllRanges()
    getSelection().addRange(boundary)
    check(
      reader.selection()?.quote.includes('sentinel page 1'),
      'Selection ending at the page boundary lost its PDF text'
    )
    getSelection().removeAllRanges()
    const results = await reader.search('sentinel page 12')
    check(
      results.complete && results.matches.length === 1 && results.matches[0].page === 12,
      'PDF search did not scan the complete document'
    )
    await reader.activateSearch({ ...results.matches[0], page: 1 }, { query: 'sentinel page 12' })
    const pdfOwner = root.querySelector('.marknoria-pdf')
    check(reader.state().page === 12 && pdfOwner.dataset.searchCurrent === results.matches[0].searchMatchId, 'PDF activation ignored its owned occurrence')
    const highlightedPdf = CSS.highlights?.get(pdfOwner.dataset.searchHighlight)
    check(highlightedPdf ? [...highlightedPdf][0].toString() === 'sentinel page 12' : pdfOwner.querySelector('[data-search-current-rect]'), 'PDF current occurrence has no exact visible marker')
    reader.clearSearch({ keepResults: true })
    check(!pdfOwner.hasAttribute('data-search-current'), 'PDF presentation clear retained a marker')
    const repaintPosition = reader.state().page, repaintScroll = viewport.scrollTop
    check(reader.repaintSearch(results.matches[0], { query: 'sentinel page 12' }) && pdfOwner.dataset.searchCurrent === results.matches[0].searchMatchId && reader.state().page === repaintPosition && viewport.scrollTop === repaintScroll, 'PDF presentation repaint did not restore the owned marker without navigation')
    await reader.activateSearch(results.matches[0], { query: 'sentinel page 12' })
    reader.clearSearch()
    check(!pdfOwner.hasAttribute('data-search-current') && await reader.activateSearch(results.matches[0], { query: 'sentinel page 12' }).then(() => false, () => true), 'PDF cleared occurrence remained active/activatable')
    check(await reader.search('sentinel', { offset: 100 }).then(() => false, () => true), 'PDF adapter silently ignored unsupported offset')
    const repeated = await reader.search('needle')
    check(repeated.complete && repeated.matches.length === 2 && repeated.matches.every(match => match.page === 1 && match.locationPrecision === 'page'), 'PDF did not retain repeated occurrences or honest page precision')
    check(root.querySelector('.textLayer')?.textContent !== '', 'PDF text disappeared during search')
    await reader.settings({ continuous: false })
    await reader.navigate(locationFor(2))
    await reader.activateSearch(repeated.matches[1], { query: 'needle' })
    check(root.querySelector('[data-pdf-page="1"] .textLayer').textContent.includes('İ'), 'Unicode search fixture did not preserve its expanding lowercase character')
    const repeatedHighlight = CSS.highlights?.get(pdfOwner.dataset.searchHighlight)
    check(repeatedHighlight ? [...repeatedHighlight][0].toString() === 'NEEDLE' : pdfOwner.querySelector('[data-search-current-rect]'), 'Unicode case expansion shifted the PDF occurrence range')
    if (repeatedHighlight) {
      const rect = [...repeatedHighlight][0].getBoundingClientRect(), view = viewport.getBoundingClientRect()
      check(rect.top >= view.top && rect.bottom <= view.bottom, 'A lower occurrence on a previously hidden PDF page remained outside the viewport')
    }
    reader.clearSearch()
    const metadata = await reader.outline({ resolve: false })
    check(metadata.total === 2 && metadata.items.every(item => item.navigable && !('location' in item)), 'PDF metadata outline resolved or exposed destinations')
    await reader.navigateOutline(metadata.items[1].id)
    check(reader.state().page === 12, 'PDF outline ID navigation failed')
    const outline = await reader.outline()
    check(
      outline.total === 2 && outline.items[1].location.pages[0].page === 12,
      'PDF outline did not resolve its destination'
    )
    await reader.settings({ zoom: 1, continuous: false })
    await reader.navigate(locationFor(2))
    await until(() => root.querySelector('[data-pdf-page="2"][data-state="ready"]'))
    check(
      reader.state().zoom === 1 && root.querySelectorAll('.pdf-page:not([hidden])').length === 1,
      'Global 100% zoom or paged flow was lost'
    )
    check(
      Math.abs(root.querySelector('[data-pdf-page="2"]').clientWidth - 800) <= 2,
      '100% PDF scale is not 96 CSS pixels per inch'
    )
    root
      .querySelector('[data-pdf-page="2"]')
      .dispatchEvent(new KeyboardEvent('keydown', { key: 'PageDown', bubbles: true }))
    await until(
      () =>
        reader.state().page === 3 && root.querySelector('[data-pdf-page="3"][data-state="ready"]')
    )
    check(reader.state().zoom === 1, 'Page turn reset the document zoom')
    await reader.navigate(locationFor(10))
    await until(() => root.querySelector('[data-pdf-page="10"][data-state="ready"]'))
    check(
      Math.abs(root.querySelector('[data-pdf-page="10"]').clientWidth - (800 * 4) / 3) <= 2,
      'Rotated PDF page dimensions were lost'
    )
    await reader.settings({ continuous: true })
    await reader.navigate(locationFor(12))
    await until(() => root.querySelector('[data-pdf-page="12"][data-state="ready"]'))
    check(
      reader.state().mountedPages <= 3 && root.querySelectorAll('canvas').length <= 3,
      'PDF retained offscreen canvases'
    )
    check(
      !root.querySelector('[data-pdf-page="1"] canvas'),
      'First-page canvas survived long-distance navigation'
    )
    check(
      reader.state().position.location.kind === 'pdf' &&
        reader.state().position.location.pages[0].page === 12,
      'Reading position lost the original PDF page'
    )
    const invalid = await reader.navigate(locationFor(13)).then(
      () => false,
      () => true
    )
    check(invalid, 'Out-of-range PDF navigation succeeded')
    const cancelled = new AbortController()
    cancelled.abort()
    check(
      await reader.search('sentinel', { signal: cancelled.signal }).then(
        () => false,
        () => true
      ),
      'PDF search cancellation was ignored'
    )
    const firstContentMs = reader.state().firstUsefulContentMs
    await reader.dispose()
    await reader.dispose()
    check(root.childElementCount === 0, 'PDF disposal retained its DOM')
    check(translate.state().subscribers === 0, 'PDF disposal retained UI subscriptions')
    const fill = Array.from({ length: 255 }, () => translate.subscribe(() => {}))
    let failed = false
    try {
      mountPdfReader(root, {
        viewport,
        translate,
        url: new URL('/fixture.pdf', location.href).href,
        documentId: 'public-pdf',
        revision: 'a'.repeat(64),
        resourceId: 'public-resource'
      })
    } catch {
      failed = true
    }
    check(
      failed && root.childElementCount === 0 && translate.state().subscribers === 255,
      'Failed PDF UI initialization retained scopes or DOM'
    )
    for (const remove of fill) remove()
    const before = events.length
    viewport.dispatchEvent(new Event('scroll'))
    await new Promise((resolve) => setTimeout(resolve, 50))
    check(events.length === before, 'Disposed PDF still emitted state')
    const realTimeout = globalThis.setTimeout
    let stalled
    try {
      globalThis.setTimeout = (callback, milliseconds, ...args) => realTimeout(callback, milliseconds === 60000 ? 25 : milliseconds, ...args)
      stalled = mountPdfReader(root, { viewport, translate, url: new URL('/stall.pdf', location.href).href, documentId:'stalled', resourceId:'stalled', revision:'b'.repeat(64) })
      let cause
      try { await stalled.ready } catch (error) { cause = error }
      check(cause?.name !== 'AbortError' && cause?.message === 'PDF loading exceeded 60 seconds', 'Disposal hid the original PDF timeout cause')
    } finally { globalThis.setTimeout = realTimeout; await stalled?.dispose() }
    return {
      assertions,
      pages: 12,
      firstContentMs,
      textSelection: true,
      crossPageSearch: true,
      globalZoom: true,
      pagedNavigation: true,
      cleanup: true
    }
  } finally {
    reader.dispose()
    viewport.remove()
  }
}
