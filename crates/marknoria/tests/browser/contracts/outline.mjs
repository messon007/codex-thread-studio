export async function outlineContract(moduleUrl) {
  const { mountOutline } = await import(moduleUrl)
  const { createTranslator } = await import(new URL('./translation.mjs', new URL(moduleUrl, location.href)))
  const { createPdfOutline } = await import(new URL('./pdf-outline.mjs', new URL(moduleUrl, location.href)))
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const until = async predicate => {
    for (let i = 0; i < 500 && !predicate(); i++) await new Promise(resolve => setTimeout(resolve, 10))
    check(predicate(), 'Outline did not settle')
  }
  const sheet = document.createElement('link'); sheet.rel = 'stylesheet'; sheet.href = '/styles.css'
  document.head.append(sheet)
  await new Promise((resolve, reject) => { sheet.onload = resolve; sheet.onerror = reject })
  const root = document.createElement('div'); root.className = 'reader-shell'
  root.style.cssText = 'width:350px;height:420px;display:flex;flex-direction:column'
  const filter = document.createElement('input'), feedback = document.createElement('div'), viewport = document.createElement('div'), space = document.createElement('div')
  viewport.style.cssText = 'flex:1;min-height:0;overflow:auto'; space.style.position = 'relative'
  viewport.append(space); root.append(filter, feedback, viewport); document.body.append(root)
  const navigated = [], translate = createTranslator()
  const ui = mountOutline({ viewport, space, filter, feedback, translate, navigate: id => navigated.push(id) })
  const query = async value => { filter.value = value; filter.dispatchEvent(new Event('input')); await until(() => viewport.getAttribute('aria-busy') === 'false') }
  try {
    const headings = [{ id: 0, level: 1, label: 'Root' }, { id: 1, level: 2, label: 'Child' }, { id: 2, level: 3, label: 'Grandchild needle' },
      ...Array.from({ length: 2100 }, (_, i) => ({ id: i + 3, level: 1, label: `Chapter ${i + 3}` }))]
    await ui.setItems(headings)
    check(space.querySelectorAll('.toc-row').length < 30, 'Outline mounted the complete heading list')
    space.querySelector('[data-outline-id="0"] .toc-branch').click()
    await until(() => !space.querySelector('[data-outline-id="1"]'))
    check(space.querySelector('[data-outline-id="0"] .toc-branch').getAttribute('aria-expanded') === 'false', 'Collapse state missing')
    await query('needle')
    check(space.querySelectorAll('.toc-row').length === 3, 'Filter lost ancestor context')
    check(space.querySelectorAll('.toc-context').length === 2, 'Filter did not distinguish ancestor context')
    filter.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    await until(() => navigated.at(-1) === 2)
    await query('')
    check(!space.querySelector('[data-outline-id="1"]'), 'Clearing filter lost saved collapse state')
    const firstLink = space.querySelector('.toc-link')
    firstLink.focus()
    firstLink.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }))
    await until(() => document.activeElement?.textContent === 'Chapter 2102')
    check(space.querySelectorAll('.toc-row').length < 30, 'Keyboard navigation mounted all rows')
    await query('Chapter 2102')
    check(space.querySelectorAll('.toc-row').length === 1, 'Headings after 2,000 were silently discarded')
    space.querySelector('.toc-link').click(); await until(() => navigated.at(-1) === 2102)
    check(root.isConnected && !root.hidden, 'Navigation closed the outline')
    ui.active(2102)
    check(space.querySelector('[aria-current="location"]')?.textContent === 'Chapter 2102', 'Active heading missing')
    translate.update({ locale: 'fr', messages: { 'Filter contents': 'Filtrer le sommaire' } })
    check(filter.getAttribute('aria-label') === 'Filtrer le sommaire', 'Outline filter label did not update')
    const beforeEnter = navigated.length
    filter.value = 'Root'; filter.dispatchEvent(new Event('input'))
    filter.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    filter.value = 'Child'; filter.dispatchEvent(new Event('input'))
    await until(() => viewport.getAttribute('aria-busy') === 'false')
    check(!space.textContent.includes('Chapter'), 'Superseded filter work replaced newer results')
    check(navigated.length === beforeEnter, 'Pending filter navigated an old result')
    await query('no such heading')
    check(space.children.length === 0 && feedback.textContent.includes('0'), 'No-match feedback missing')
    const stale = ui.setItems(headings)
    await ui.setItems([{ id: 0, level: 1, label: 'New document' }])
    await stale
    check(filter.value === '' && space.textContent.includes('New document'), 'Document change retained stale filter/content')
    filter.value = 'New'; filter.dispatchEvent(new Event('input')); ui.dispose()
    await new Promise(resolve => setTimeout(resolve, 20))
    check(!space.children.length, 'Disposed outline remounted content')
  } finally { ui.dispose(); root.remove(); sheet.remove() }

  // Real renderer helper, injected PDF document: metadata listing must not
  // resolve even a named destination; resolving one page must not visit others.
  const counts = { outline: 0, destination: 0, page: 0 }
  const doc = { numPages: 12,
    async getOutline() { counts.outline++; return [{ title: 'Group', dest: null, items: [{ title: 'Named', dest: 'named' }] }, { title: 'Numeric', dest: [4] }] },
    async getDestination() { counts.destination++; return [{ num: 7 }] },
    async getPageIndex() { counts.page++; return 11 }
  }
  const manyCounts = { outline: 0, destination: 0 }
  const manyDoc = { numPages: 600,
    getOutline: async () => { manyCounts.outline++; return Array.from({length:600},(_,i)=>({title:`Page ${i}`,dest:String(i)})) },
    getDestination: async name => { manyCounts.destination++; return [Number(name)] }
  }
  const many = createPdfOutline({ document: () => manyDoc, ensure() {}, bounded: promise => promise, pause: () => Promise.resolve() })
  await many.page({ offset:256, resolve:false })
  check(manyCounts.destination === 0, 'PDF metadata page resolved destinations')
  const second = await many.page({ offset:256 })
  check(second.items.length === 256 && second.items[0].location.pages[0].page === 257 && manyCounts.destination === 256, 'PDF resolved destinations outside requested page')
  const cancelledMany = new AbortController(); cancelledMany.abort()
  try { await many.page({ offset:0,signal:cancelledMany.signal }) } catch {}
  check(manyCounts.destination === 256, 'Cancelled PDF page started destination work')
  many.dispose()
  let disposed = false
  const helper = createPdfOutline({ document: () => doc, ensure: () => { if (disposed) throw Error('Disposed') }, bounded: promise => promise, pause: () => Promise.resolve() })
  const metadata = await helper.page({ resolve: false })
  check(metadata.items.length === 3 && !metadata.items[0].navigable && metadata.items[1].level === 2, 'PDF group/hierarchy lost')
  check(counts.destination === 0 && counts.page === 0, 'Metadata listing resolved PDF destinations')
  check((await helper.location(1)).pages[0].page === 12, 'Named PDF destination wrong')
  await helper.location(1)
  check(counts.destination === 1 && counts.page === 1, 'PDF destination cache not reused')
  check((await helper.page()).items[2].location.pages[0].page === 5, 'Default resolved outline compatibility lost')
  const cancel = new AbortController(); cancel.abort()
  let cancelled = false
  try { await helper.page({ resolve: false, signal: cancel.signal }) } catch (error) { cancelled = error.name === 'AbortError' }
  check(cancelled, 'Aborted outline request succeeded')
  disposed = true; helper.dispose()
  let failed = false; try { await helper.location(1) } catch { failed = true }
  check(failed && counts.outline === 1, 'Disposed PDF outline started new document work')
  const empty = createPdfOutline({ document: () => ({ getOutline: async () => { counts.outline++; return null } }), ensure() {}, bounded: promise => promise, pause: () => Promise.resolve() })
  const before = counts.outline
  await empty.page(); await empty.page()
  check(counts.outline === before + 1, 'Empty PDF outline was repeatedly loaded')
  return { assertions }
}
