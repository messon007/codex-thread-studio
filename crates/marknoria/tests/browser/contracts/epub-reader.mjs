export async function epubReaderContract(moduleUrl) {
  const { createEpubReader, normalizeEpubReaderState } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const translate = createTranslator()
  const root = document.createElement('main')
  root.style.cssText = 'position:fixed;inset:10px;width:1100px;height:800px'
  document.body.append(root)
  const lifetime = new AbortController(),
    selections = [],
    positions = [],
    external = [],
    errors = []
  let reader,
    assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const until = async (condition, label = '') => {
    const deadline = performance.now() + 10000
    while (!condition() && performance.now() < deadline)
      await new Promise((resolve) => setTimeout(resolve, 10))
    check(
      condition(),
      `EPUB did not reach ${label}: ${JSON.stringify({ errors, state: reader?.state(), positions: positions.length, selections: selections.length, frames: root.querySelectorAll('iframe').length })}`
    )
  }
  try {
    const original = { cfi: 'epubcfi(' + '/4/2'.repeat(3000) + ')' }
    let rejected = false
    try {
      normalizeEpubReaderState(original)
    } catch {
      rejected = true
    }
    check(rejected && original.cfi.length > 8192, 'Oversized CFI was silently truncated')
    const started = performance.now()
    reader = await createEpubReader({
      container: root,
      translate,
      source: new URL('/book/OPS/book.opf', location.href).href,
      resourceRoot: new URL('/book/', location.href).href,
      signal: lifetime.signal,
      onSelection: (value) => selections.push(value),
      onRelocate: (value) => positions.push(value),
      onExternalLink: (value) => external.push(value),
      onError: (error) => errors.push(String(error))
    })
    const firstContentMs = performance.now() - started
    await until(() => reader.state().cfi.startsWith('epubcfi('), 'initial CFI')
    check(reader.title === 'Public Book 中文', 'EPUB metadata changed')
    const localeFrame = root.querySelector('iframe'),
      positionBefore = reader.state().cfi,
      eventsBefore = positions.length
    translate.update({
      locale: 'fr',
      messages: {
        Previous: 'Précédent',
        'Reading settings': 'Réglages',
        Paper: 'Papier',
        'Build progress index': 'Construire la progression',
        'Public Book 中文': 'WRONG DATA'
      }
    })
    check(
      root.querySelector('.epub-previous').getAttribute('aria-label') === 'Précédent' &&
        root.querySelector('.epub-settings-toggle').title === 'Réglages' &&
        root.querySelector('.epub-progress-build').textContent === 'Construire la progression',
      'EPUB UI labels did not update'
    )
    check(
      root.querySelector('iframe') === localeFrame &&
        reader.state().cfi === positionBefore &&
        positions.length === eventsBefore &&
        root.querySelector('.epub-book-identity strong').textContent === 'Public Book 中文',
      'EPUB locale update rebuilt content, changed CFI or translated metadata'
    )

    check(reader.outline().length === 2, 'EPUB outline is incomplete')
    const matches = await reader.search('EPUB sentinel two')
    check(
      matches.complete && matches.matches.length === 1,
      'EPUB search did not scan the complete spine'
    )
    check(
      (await reader.quoteForCfi(matches.matches[0].cfi)) === 'EPUB sentinel two',
      'EPUB search lost its exact CFI'
    )
    const initialSearchCfi = reader.state().cfi
    await reader.activateSearch({ ...matches.matches[0], cfi: initialSearchCfi }, { query: 'EPUB sentinel two' })
    const epubOwner = root.querySelector('.epub-reader')
    check(epubOwner.dataset.searchCurrent === matches.matches[0].searchMatchId && reader.state().href.endsWith('two.xhtml'), 'EPUB activation trusted supplied coordinates instead of its owned CFI')
    const markedFrame = [...root.querySelectorAll('iframe')].find(frame => frame.contentDocument?.getElementById('target'))
    const highlightedEpub = markedFrame.contentWindow.CSS?.highlights?.get(epubOwner.dataset.searchHighlight)
    check(highlightedEpub ? [...highlightedEpub][0].toString() === 'EPUB sentinel two' : epubOwner.querySelector('[data-search-current-rect]'), 'EPUB current CFI has no exact visible marker')
    await reader.settings({ flow: 'scrolled' })
    check(epubOwner.dataset.searchCurrent === matches.matches[0].searchMatchId, 'EPUB flow change lost its active original CFI highlight')
    reader.clearSearch({ keepResults: true })
    check(!epubOwner.hasAttribute('data-search-current'), 'EPUB presentation clear retained a marker')
    const repaintCfi = reader.state().cfi, repaintScroll = root.querySelector('.epub-container').scrollTop
    check(reader.repaintSearch(matches.matches[0], { query: 'EPUB sentinel two' }) && epubOwner.dataset.searchCurrent === matches.matches[0].searchMatchId && reader.state().cfi === repaintCfi && root.querySelector('.epub-container').scrollTop === repaintScroll, 'EPUB presentation repaint did not restore its owned CFI without navigation')
    await reader.activateSearch(matches.matches[0], { query: 'EPUB sentinel two' })
    reader.clearSearch()
    check(!epubOwner.hasAttribute('data-search-current') && await reader.activateSearch(matches.matches[0], { query: 'EPUB sentinel two' }).then(() => false, () => true), 'EPUB cleared occurrence remained active/activatable')
    check(await reader.search('sentinel', { offset: 100 }).then(() => false, () => true), 'EPUB adapter silently ignored unsupported offset')
    const foldedCase = await reader.search('epub sentinel two', { matchCase: false })
    check(foldedCase.matches.length === 1 && foldedCase.matchCase === false, 'EPUB insensitive search ignored its option')
    await reader.activateSearch(foldedCase.matches[0], { query: 'epub sentinel two', matchCase: false })
    check(await reader.quoteForCfi(foldedCase.matches[0].cfi) === 'EPUB sentinel two', 'EPUB insensitive CFI changed original text')
    const strictCase = await reader.search('epub sentinel two')
    check(strictCase.matches.length === 0 && strictCase.matchCase === true, 'EPUB omitted option did not retain its strict default')
    check(await reader.activateSearch(foldedCase.matches[0], { query: 'epub sentinel two', matchCase: false }).then(() => false, () => true), 'EPUB case toggle retained stale owned matches')
    await reader.settings({ flow: 'paginated' })
    await reader.display(initialSearchCfi)
    const frame = root.querySelector('iframe')
    check(Boolean(frame), 'EPUB did not use its sandboxed document')
    check(
      frame.sandbox.contains('allow-same-origin') && !frame.sandbox.contains('allow-scripts'),
      'EPUB scripts gained authority'
    )
    check(
      !globalThis.compromised && !frame.contentDocument.querySelector('script'),
      'Book script executed or survived sanitization'
    )
    await until(() => frame.contentDocument.querySelector('img')?.naturalWidth > 0, 'image decode')
    const paragraph = [...frame.contentDocument.querySelectorAll('p')].find(
      (node) => node.textContent === 'EPUB sentinel one'
    )
    check(
      !paragraph.hasAttribute('contenteditable'),
      'EPUB preview allowed body edits that invalidate CFIs'
    )
    frame.contentWindow.focus()
    paragraph.tabIndex = -1
    paragraph.focus()
    const range = frame.contentDocument.createRange()
    range.selectNodeContents(paragraph)
    const selection = frame.contentWindow.getSelection()
    selection.removeAllRanges()
    selection.addRange(range)
    check(
      selection.rangeCount > 0 && !selection.isCollapsed,
      'EPUB document did not retain its actual DOM selection'
    )
    frame.contentDocument.dispatchEvent(new Event('selectionchange'))
    await until(() => selections.length > 0, 'selection callback')
    const selected = selections.at(-1)
    check(
      selected.quote === 'EPUB sentinel one' && selected.cfiRange.startsWith('epubcfi('),
      'EPUB selection lost its quote/CFI'
    )
    const resolvedQuote = await reader.quoteForCfi(selected.cfiRange)
    check(
      resolvedQuote === selected.quote,
      `CFI did not resolve to its original text: ${JSON.stringify({ cfi: selected.cfiRange, quote: selected.quote, resolvedQuote })}`
    )
    selection.collapse(paragraph.firstChild, 0)
    frame.contentDocument.dispatchEvent(new Event('selectionchange'))
    check(reader.selection() === null && selections.at(-1) === null, 'Collapsed EPUB text retained a stale CFI selection')
    selection.removeAllRanges(); selection.addRange(range)
    frame.contentDocument.dispatchEvent(new Event('selectionchange'))
    check(reader.selection()?.quote === selected.quote, 'EPUB selection did not recover after collapse')
    selection.removeAllRanges()
    frame.contentDocument.dispatchEvent(new Event('selectionchange'))
    check(reader.selection() === null && selections.at(-1) === null, 'Cleared EPUB range retained text')
    // Exercise the sandbox fallback, without dispatching selectionchange or
    // calling reader.selection() (both used to conceal stale invalidation).
    const suppress = event => event.stopImmediatePropagation()
    frame.contentDocument.addEventListener('selectionchange', suppress, true)
    try {
      selection.removeAllRanges(); selection.addRange(range)
      await until(() => selections.at(-1)?.quote === selected.quote, 'polled range capture')
      selection.collapse(paragraph.firstChild, 0)
      await until(() => selections.at(-1) === null, 'polled collapsed invalidation')
      selection.removeAllRanges(); selection.addRange(range)
      await until(() => selections.at(-1)?.quote === selected.quote, 'polled range recapture')
      selection.removeAllRanges()
      await until(() => selections.at(-1) === null, 'polled empty invalidation')
      check(true, 'Collapsed/empty sandbox selections must notify without getter calls')
    } finally { frame.contentDocument.removeEventListener('selectionchange', suppress, true) }
    frame.contentDocument.dispatchEvent(new Event('pointerdown'))
    selection.addRange(range)
    frame.contentDocument.dispatchEvent(new Event('selectionchange'))
    await until(() => selections.at(-1)?.quote === selected.quote, 'selection before scroll')
    root.querySelector('.epub-container').dispatchEvent(new Event('scroll'))
    await until(() => selections.at(-1) === null, 'scroll invalidation')
    check(reader.selection() === null, 'Unchanged range resurrected a scroll-dismissed selection')
    const partialRange = frame.contentDocument.createRange()
    partialRange.setStart(paragraph.firstChild, 0)
    partialRange.setEnd(paragraph.firstChild, 13)
    selection.removeAllRanges(); selection.addRange(partialRange)
    await until(() => selections.at(-1)?.quote === 'EPUB sentinel', 'new range after scroll')
    check(await reader.quoteForCfi(selections.at(-1).cfiRange) === 'EPUB sentinel', 'Partial reselection lost its exact CFI')
    const saved = reader.state().cfi
    await reader.settings({ theme: 'sepia', fontScale: 1.25, flow: 'scrolled' })
    await reader.display(saved)
    check(reader.selection() === null, 'Explicit CFI navigation retained the old selection')
    await until(
      () => reader.state().theme === 'sepia' && reader.state().flow === 'scrolled',
      'updated settings'
    )
    check(
      (await reader.quoteForCfi(selected.cfiRange)) === selected.quote,
      'Theme/font/flow changed the CFI body tree'
    )
    check(
      reader.state().fontScale === 1.25 && reader.state().progressPrecision === 'chapter',
      'Reading settings or lazy progress profile changed'
    )
    await reader.display(reader.outline()[1].target.href)
    await until(
      () =>
        [...root.querySelectorAll('iframe')].some((frame) =>
          frame.contentDocument?.getElementById('target')
        ),
      'second chapter'
    )
    check(
      (await reader.quoteForCfi(selected.cfiRange)) === selected.quote,
      'Chapter navigation invalidated a saved CFI'
    )
    check(root.querySelectorAll('iframe').length <= 3, 'EPUB retained too many document frames')
    check(
      root.querySelector('.epub-progress-seek').disabled,
      'EPUB eagerly enabled a partial progress index'
    )
    let needed = false
    try {
      await reader.seekProgress(0.5)
    } catch (error) {
      needed = error.code === 'PROGRESS_INDEX_REQUIRED'
    }
    check(needed, 'Percentage navigation silently used an incomplete index')
    const beforeIndex = reader.state().cfi,
      abortIndex = new AbortController()
    const cancelled = reader.buildProgressIndex({ signal: abortIndex.signal }).then(
      () => null,
      (error) => error
    )
    const busy = await reader.buildProgressIndex().then(
      () => false,
      (error) => error.code === 'BUSY'
    )
    abortIndex.abort()
    const cancellation = await cancelled
    check(
      busy &&
        cancellation?.name === 'AbortError' &&
        reader.state().progressIndex.status === 'cancelled',
      'EPUB indexing did not isolate/cancel its current operation'
    )
    check(
      reader.state().progressPrecision === 'chapter' && reader.state().cfi === beforeIndex,
      'Cancelled progress indexing changed the saved position or committed partial data'
    )
    const indexed = await reader.buildProgressIndex()
    check(
      indexed.status === 'ready' &&
        indexed.scanned === 2 &&
        indexed.locations > 0 &&
        reader.state().progressProfile.chunkUtf16 === 1600,
      'EPUB progress index did not cover the complete spine with the shared algorithm'
    )
    check(
      reader.state().progressPrecision === 'indexed' &&
        reader.state().cfi === beforeIndex &&
        !root.querySelector('.epub-progress-seek').disabled,
      'Indexing moved the reader or failed to expose its result'
    )
    const sliderKey = new KeyboardEvent('keydown', {
      key: 'ArrowRight',
      bubbles: true,
      cancelable: true
    })
    root.querySelector('.epub-progress-seek').dispatchEvent(sliderKey)
    check(
      !sliderKey.defaultPrevented,
      'Book page shortcuts intercepted the progress slider keyboard'
    )
    await reader.settings({ flow: 'paginated' })
    await reader.seekProgress(0)
    check(
      reader.state().href.includes('one.xhtml'),
      'Indexed start did not resolve to the original first chapter'
    )
    await reader.seekProgress(1)
    check(
      reader.state().href.includes('two.xhtml'),
      'Indexed end did not resolve to the original last chapter'
    )
    check(
      (await reader.quoteForCfi(selected.cfiRange)) === selected.quote,
      'Progress indexing changed an existing exact CFI'
    )
    const count = positions.length
    reader.destroy()
    lifetime.abort()
    await new Promise((resolve) => setTimeout(resolve, 50))
    check(
      root.childElementCount === 0 && positions.length === count,
      'Destroyed EPUB retained DOM or emitted state'
    )
    check(translate.state().subscribers === 0, 'EPUB destruction retained its translator')
    const fill = Array.from({ length: 256 }, () => translate.subscribe(() => {}))
    let failed = false
    try {
      await createEpubReader({
        container: root,
        translate,
        source: new URL('/book/OPS/book.opf', location.href).href,
        resourceRoot: new URL('/book/', location.href).href
      })
    } catch {
      failed = true
    }
    check(
      failed && !root.childElementCount && translate.state().subscribers === 256,
      'Failed EPUB UI setup changed its host or leaked a scope'
    )
    for (const remove of fill) remove()
    const budgetLifetime = new AbortController()
    reader = await createEpubReader({
      container: root,
      source: new URL('/budget-book/OPS/book.opf', location.href).href,
      resourceRoot: new URL('/budget-book/', location.href).href,
      signal: budgetLifetime.signal
    })
    const budgetCfi = reader.state().cfi
    const overBudget = await reader.buildProgressIndex().then(
      () => null,
      (error) => error
    )
    check(
      overBudget?.code === 'PROGRESS_BUDGET' &&
        reader.state().progressIndex.status === 'error' &&
        reader.state().progressPrecision === 'chapter',
      'Oversized book progress committed or silently truncated an index'
    )
    check(
      reader.state().cfi === budgetCfi &&
        root.querySelector('iframe') &&
        !root.querySelector('.epub-progress-build').disabled,
      'A progress budget error broke reading or retry controls'
    )
    reader.destroy()
    budgetLifetime.abort()
    const slowLifetime = new AbortController()
    reader = await createEpubReader({
      container: root,
      source: new URL('/slow-book/OPS/book.opf', location.href).href,
      resourceRoot: new URL('/slow-book/', location.href).href,
      signal: slowLifetime.signal
    })
    const slowCfi = reader.state().cfi
    const pendingIndex = reader.buildProgressIndex().then(
      () => null,
      (error) => error
    )
    await until(() => reader.state().progressIndex.scanned === 1, 'first progress chapter')
    const cancelAt = performance.now()
    reader.cancelProgressIndex()
    const cancelledIndex = await pendingIndex
    check(
      cancelledIndex?.name === 'AbortError' &&
        performance.now() - cancelAt < 1000 &&
        reader.state().cfi === slowCfi,
      'A stalled progress request did not cancel promptly or changed the CFI'
    )
    const timed = await reader.buildProgressIndex().then(
      () => null,
      (error) => error
    )
    check(
      timed?.code === 'PROGRESS_TIMEOUT' &&
        reader.state().progressIndex.status === 'error' &&
        reader.state().progressPrecision === 'chapter',
      'Progress indexing did not enforce its real deadline'
    )
    check(
      reader.state().cfi === slowCfi && root.querySelector('iframe'),
      'A progress deadline destroyed readable content'
    )
    reader.destroy()
    slowLifetime.abort()
    const corpusLifetime = new AbortController(),
      corpusStart = performance.now()
    reader = await createEpubReader({
      container: root,
      source: new URL('/corpus-book/OPS/book.opf', location.href).href,
      resourceRoot: new URL('/corpus-book/', location.href).href,
      signal: corpusLifetime.signal
    })
    const corpusFirstContentMs = performance.now() - corpusStart,
      corpusCfi = reader.state().cfi
    let heartbeatAt = performance.now(),
      maxGap = 0
    const heartbeat = setInterval(() => {
      const now = performance.now()
      maxGap = Math.max(maxGap, now - heartbeatAt)
      heartbeatAt = now
    }, 10)
    let corpusIndex
    try {
      corpusIndex = await reader.buildProgressIndex()
    } finally {
      clearInterval(heartbeat)
    }
    check(
      corpusIndex.scanned === 64 &&
        corpusIndex.textUtf16 > 3 * 1024 * 1024 &&
        corpusIndex.locations > 2000,
      'Large progress corpus was sampled or left incomplete'
    )
    check(
      reader.state().cfi === corpusCfi && root.querySelectorAll('iframe').length <= 3,
      'Large progress indexing moved the reader or retained chapter frames'
    )
    const corpusMetrics = {
      chapters: 64,
      firstContentMs: corpusFirstContentMs,
      indexMs: corpusIndex.elapsedMs,
      textUtf16: corpusIndex.textUtf16,
      locations: corpusIndex.locations,
      maxHeartbeatGapMs: maxGap
    }
    reader.destroy()
    corpusLifetime.abort()

    // Delay the target iframe stylesheet so cancellation lands inside epub.js
    // display, after source preload but before its asynchronous scroll commit.
    reader.destroy()
    reader = await createEpubReader({ container: root,
      source: new URL('/activation-book/OPS/book.opf', location.href).href,
      resourceRoot: new URL('/activation-book/', location.href).href,
      onError: error => errors.push(String(error)) })
    const abortMatches = await reader.search('EPUB sentinel two'), beforeActivation = reader.state().cfi
    const activation = reader.activateSearch(abortMatches.matches[0], { query: 'EPUB sentinel two' }).then(() => null, error => error)
    await until(() => [...root.querySelectorAll('iframe')].some(frame => frame.contentDocument?.getElementById('target')), 'delayed activation target')
    reader.clearSearch()
    check((await activation)?.name === 'AbortError', 'Cleared EPUB activation did not cancel promptly')
    await until(() => reader.state().cfi === beforeActivation && [...root.querySelectorAll('iframe')].some(frame => frame.contentDocument?.getElementById('start')), 'cancelled activation restored its original viewport')
    await new Promise(resolve => setTimeout(resolve, 650))
    check(reader.state().cfi === beforeActivation && !root.querySelector('[data-search-current]'), 'A cancelled delayed EPUB display moved or highlighted the viewport later')
    const supersededMatches = await reader.search('EPUB sentinel two')
    const superseded = reader.activateSearch(supersededMatches.matches[0], { query: 'EPUB sentinel two' }).then(() => null, error => error)
    await until(() => [...root.querySelectorAll('iframe')].some(frame => frame.contentDocument?.getElementById('target')), 'superseded activation target')
    await reader.display(beforeActivation)
    check((await superseded)?.name === 'AbortError', 'New EPUB navigation did not cancel its preceding activation')
    await new Promise(resolve => setTimeout(resolve, 650))
    check(reader.state().cfi === beforeActivation && !root.querySelector('[data-search-current]'), 'An old delayed EPUB activation overwrote newer navigation')
    return {
      assertions,
      firstContentMs,
      progressIndexMs: indexed.elapsedMs,
      progressTextUtf16: indexed.textUtf16,
      boundedProgress: true,
      corpus: corpusMetrics,
      exactCfi: true,
      sandbox: true,
      lazyProgress: true,
      settings: true,
      cleanup: true
    }
  } finally {
    reader?.destroy()
    lifetime.abort()
    root.remove()
  }
}
