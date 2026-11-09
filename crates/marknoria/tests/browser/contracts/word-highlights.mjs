export async function wordHighlightsContract(moduleUrl) {
  const { mountWordHighlights, WORD_HIGHLIGHT_LIMITS, isHighlightWord } = await import(moduleUrl)
  const { codeView } = await import('/editor.mjs')
  let assertions = 0
  const check = (condition, message) => { if (!condition) throw Error(message); assertions++ }
  const pause = delay => new Promise(resolve => setTimeout(resolve, delay))
  const settle = async engine => {
    await pause(90)
    for (let i = 0; i < 120 && engine.state().pending; i++) await pause(20)
    check(!engine.state().pending, 'Highlight work failed to terminate')
    const state = engine.state()
    check(state.visitedNodes <= WORD_HIGHLIGHT_LIMITS.nodes && state.bytes <= WORD_HIGHLIGHT_LIMITS.bytes && state.matches <= WORD_HIGHLIGHT_LIMITS.marks && state.marks <= WORD_HIGHLIGHT_LIMITS.marks, `Budget exceeded: ${JSON.stringify(state)}`)
    return state
  }
  function select(node, from, to) {
    const range = document.createRange(); range.setStart(node, from); range.setEnd(node, to)
    const selection = document.getSelection(); selection.removeAllRanges(); selection.addRange(range)
    document.dispatchEvent(new Event('selectionchange'))
  }
  const host = document.createElement('div')
  host.style.cssText = 'position:fixed;inset:10px;overflow:auto;font:16px/24px monospace'
  document.body.append(host)
  const results = []
  const originalFetch = window.fetch
  window.fetch = () => { throw Error('Selection highlighting must not fetch document data') }
  try {
    check(isHighlightWord('café') && isHighlightWord('变量') && isHighlightWord('user_name'), 'Unicode word policy rejected valid words')
    check(!isHighlightWord('foo', 'x', '') && !isHighlightWord('foo', '', '_') && !isHighlightWord('two words') && !isHighlightWord('x'.repeat(129)), 'Word boundaries/length policy accepted invalid selection')
    check(isHighlightWord('中文', '学', '字'), 'Native Han phrase selection policy was lost')
    for (const forceOverlay of [false, true]) {
      host.innerHTML = '<p>needle needle needles Needle _needle needle_</p><p>nee<strong>dle</strong></p><p hidden>needle</p><button>needle</button><div style="height:6000px"></div><p>needle</p>'
      const sentinel = document.createRange(); sentinel.selectNodeContents(host.firstChild)
      const existing = []
      if (window.CSS?.highlights && window.Highlight) for (const name of ['search-sentinel', 'comment-sentinel']) { const mark = new Highlight(sentinel); CSS.highlights.set(name, mark); existing.push([name, mark]) }
      const engine = mountWordHighlights(host, { viewport: host, enabled: true, forceOverlay })
      // A global materialization would immediately fail this contract.
      const originalQuery = host.querySelectorAll
      host.querySelectorAll = () => { throw Error('Highlight traversed the whole DOM with querySelectorAll') }
      try {
        select(host.firstChild.firstChild, 0, 6)
        let state = await settle(engine)
        check(state.matches === 3 && state.marks >= 3, `Whole-word/inline/visibility matching failed: ${JSON.stringify(state)}`)
        check(document.getSelection().toString() === 'needle', 'Highlight disturbed native selection')
        check(host.querySelector('strong').textContent === 'dle', 'Highlight rewrote inline content')
        const elementRange = document.createRange(); elementRange.selectNodeContents(host.children[1])
        document.getSelection().removeAllRanges(); document.getSelection().addRange(elementRange)
        document.dispatchEvent(new Event('selectionchange'))
        const elementState = await settle(engine)
        check(elementState.word === 'needle' && elementState.matches === 3, 'Element selection boundaries were not normalized across syntax/inline spans')
        if (forceOverlay) {
          const overlay = document.querySelector('[data-word-highlight-overlay]')
          check(getComputedStyle(overlay).pointerEvents === 'none' && [...overlay.children].every(mark => getComputedStyle(mark).pointerEvents === 'none'), 'Fallback captures pointer input')
        }
        // Collapsing selection clears both the queue and all owned paint.
        document.getSelection().collapse(host.firstChild.firstChild, 0)
        document.dispatchEvent(new Event('selectionchange'))
        check(engine.state().marks === 0, 'Deselection did not clear synchronously')
        await settle(engine)
        check(engine.state().word === '', 'Deselection retained stale word')
        select(host.firstChild.firstChild, 0, 6)
        engine.update({ enabled: false })
        await settle(engine)
        check(engine.state().marks === 0 && !engine.state().enabled, 'Off repainted pending selection')
        for (const [name, mark] of existing) check(CSS.highlights.get(name) === mark, 'Selection highlights cleared search/comment decorations')
        engine.update({ enabled: true })
        select(host.firstChild.firstChild, 14, 20) // A partial/invalid selection replaces old work.
        engine.clear()
        await pause(100)
        check(engine.state().marks === 0, 'Clear allowed stale scheduled paint')
        results.push({ backend: state.backend, maxFrameMs: state.maxFrameMs })
      } finally {
        host.querySelectorAll = originalQuery
        engine.dispose()
        for (const [name] of existing) CSS.highlights.delete(name)
      }
    }
    host.innerHTML = '<p>needle 中文</p><div class=table-controls>needle</div><table><tr><td>needle</td><td hidden>needle</td><td>need</td><td>le</td></tr></table><p>中文 中文字 Needle</p>'
    let tableEngine = mountWordHighlights(host, { viewport: host, enabled: true })
    try {
      select(host.firstChild.firstChild, 0, 6)
      const tableState = await settle(tableEngine)
      check(tableState.matches === 2, 'Table cells crossed boundaries or included a hidden column')
      select(host.firstChild.firstChild, 7, 9)
      const chineseState = await settle(tableEngine)
      check(chineseState.matches === 3, 'Bounded Chinese native selection did not find contiguous phrases')
      host.lastChild.textContent = 'x'.repeat(100000)
      select(host.lastChild.firstChild, 0, 100000)
      await settle(tableEngine)
      check(tableEngine.state().word === '' && tableEngine.state().bytes === 0, 'Oversized selection was materialized or searched')
    } finally { tableEngine.dispose() }
    host.textContent = 'needle '.repeat(2000)
    const denseEngine = mountWordHighlights(host, { viewport: host, enabled: true, forceOverlay: true })
    try {
      select(host.firstChild, 0, 6)
      const denseState = await settle(denseEngine)
      check(denseState.truncated && denseState.matches === 500 && denseState.marks <= 500, 'Dense occurrence paint was not capped')
      results.push({ scenario: 'dense', ...denseState })
      host.scrollTop = host.scrollHeight
      select(host.firstChild, host.firstChild.length - 7, host.firstChild.length - 1)
      const scrolled = await settle(denseEngine)
      check(scrolled.matches > 0 && scrolled.marks > 0, 'Offscreen occurrences exhausted the paint budget before reaching the visible selection')
    } finally { denseEngine.dispose(); host.scrollTop = 0 }
    // One huge text node: never read/copy/segment its entire contents.
    host.textContent = 'needle ' + 'a '.repeat(1024 * 1024)
    const originalSubstring = Text.prototype.substringData
    let largestRead = 0
    Text.prototype.substringData = function(offset, count) { largestRead = Math.max(largestRead, count); return originalSubstring.call(this, offset, count) }
    let engine = mountWordHighlights(host, { viewport: host, enabled: true })
    try {
      select(host.firstChild, 0, 6)
      const state = await settle(engine)
      check(state.truncated && state.marks === 1 && state.bytes <= 49152 && largestRead <= 1024, `Long single node was not bounded: ${JSON.stringify({ state, largestRead })}`)
      results.push({ scenario: 'long-node', ...state })
    } finally { engine.dispose(); Text.prototype.substringData = originalSubstring }
    host.replaceChildren()
    const fragment = document.createDocumentFragment()
    const lead = document.createElement('p'); lead.textContent = 'needle'; fragment.append(lead)
    for (let i = 0; i < 5000; i++) { const span = document.createElement('span'); span.textContent = 'x '; fragment.append(span) }
    host.append(fragment)
    engine = mountWordHighlights(host, { viewport: host, enabled: true })
    try {
      select(lead.firstChild, 0, 6)
      const state = await settle(engine)
      check(state.truncated && state.marks === 1 && state.visitedNodes <= 2000, 'Many-node traversal was not bounded')
      results.push({ scenario: 'many-nodes', ...state })
      select(lead.firstChild, 0, 6)
      engine.dispose()
      await pause(100)
      check(engine.state().marks === 0 && !engine.state().pending, 'Disposed engine repainted')
    } finally { engine.dispose() }
    host.replaceChildren()
    const sourceParent = document.createElement('div'); sourceParent.style.cssText = 'height:240px;width:700px'; host.append(sourceParent)
    const source = codeView(sourceParent, { readOnly: true })
    const sourceEngine = { state: () => source.wordHighlightState() }
    try {
      source.load('needle first\nneedle second\n' + 'other text\n'.repeat(10000) + 'needle last', 1, 'text')
      source.wordHighlights(true)
      await source.select(0, 6)
      const state = await settle(sourceEngine)
      check(state.word === 'needle' && state.marks === 2, `Source did not highlight its local viewport: ${JSON.stringify(state)}`)
      check(source.view.contentDOM.children.length < 300, 'Source materialized the full document')
      source.search()
      const input = sourceParent.querySelector('input[name=search]')
      input.value = 'needle'; input.dispatchEvent(new Event('change'))
      await pause(100)
      check(Boolean(sourceParent.querySelector('.cm-searchMatch')), 'Source word highlights removed search marks')
      source.wordHighlights(false)
      check(source.wordHighlightState().marks === 0 && Boolean(sourceParent.querySelector('.cm-searchMatch')), 'Disabling Source word marks cleared search')
      source.wordHighlights(true)
      await source.reveal(10003)
      await source.select(source.text.length - 11, source.text.length - 5)
      const bottom = await settle(sourceEngine)
      check(bottom.word === 'needle' && bottom.marks === 1, `Source viewport movement retained stale matches: ${JSON.stringify(bottom)}`)
      results.push({ scenario: 'source', ...state })
    } finally { source.dispose(); sourceParent.remove() }
    const editor = codeView(host)
    try {
      editor.load('needle needle', 1, 'text'); editor.wordHighlights(true); await editor.select(0, 6)
      await pause(100)
      check(!editor.wordHighlightState().enabled && editor.wordHighlightState().marks === 0, 'Edit enabled Source-only highlighting')
    } finally { editor.dispose() }
    check(!document.querySelector('[data-word-highlight-overlay]'), 'Highlight overlay leaked after disposal')
  } finally { window.fetch = originalFetch; document.getSelection().removeAllRanges(); host.remove() }
  return { assertions, results }
}
