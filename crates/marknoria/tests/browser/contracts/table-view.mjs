// Ordinary hosts and virtual reader containers mount this exact table view.
export async function tableViewContract(moduleUrl) {
  const { mountTableView, readerTableProvider } = await import(moduleUrl)
  let assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const tick = () => new Promise((resolve) => setTimeout(resolve, 25))
  const wait = async (predicate) => {
    const deadline = performance.now() + 5000
    while (!predicate()) {
      if (performance.now() > deadline) throw Error('Table view timed out')
      await tick()
    }
  }
  const sourceCalls = [], sourceSignal = new AbortController().signal
  const exportedProvider = await readerTableProvider({ request: async (...args) => { sourceCalls.push(args); return {} } }, { documentId: 'source-owner', revision: 'source-revision' }, 2)
  await exportedProvider.loadSource({ offset: 17, previous: true, signal: sourceSignal })
  check(sourceCalls[1][0] === 'sourceWindow' && sourceCalls[1][1].documentId === 'source-owner' && sourceCalls[1][1].revision === 'source-revision' && sourceCalls[1][1].offset === 17 && sourceCalls[1][1].previous === true && sourceCalls[1][2].signal === sourceSignal, 'Public table provider lost source ownership, backward cursor or cancellation')
  const existingStyles = document.querySelectorAll('link[href$="/table-view.css"]').length
  const host = document.createElement('section')
  host.style.cssText =
    'position:fixed;inset:12px;background:white;z-index:9999;display:flex;gap:20px'
  const outside = document.createElement('button')
  outside.textContent = 'Host control'
  document.body.append(outside, host)
  const outsideBefore = getComputedStyle(outside).borderRadius
  const create = () => {
    const workspace = document.createElement('div'),
      viewport = document.createElement('div'),
      article = document.createElement('article')
    workspace.style.cssText = 'position:relative;width:400px;height:300px'
    viewport.style.cssText = 'overflow:auto;height:100%;width:100%'
    viewport.append(article)
    workspace.append(viewport)
    host.append(workspace)
    return { workspace, viewport, article }
  }
  const left = create(),
    right = create(),
    revision = 'owned-revision'
  const metadata = { columns: ['ID', 'Value', 'Long text'], label: 'Fixture' }
  const state = { columns: [0, 1, 2], row: 0, height: 0 },
    widths = new Map()
  let mode = 'normal',
    pending,
    cancelled = 0,
    calls = [],
    queries = 0,
    charts = 0,
    reader,
    other,
    shifted = false,
    layoutFrame = null
  const provider = async ({ row, columns, signal }) => {
    calls.push({ row, columns: [...columns] })
    const deferred = mode === 'defer-first'
    if (deferred) mode = 'normal'
    if (mode === 'pending' || deferred)
      return await new Promise((resolve) => {
        pending = resolve
        signal.addEventListener('abort', () => cancelled++, { once: true })
      })
    const rowStart = Math.floor((mode === 'wrong-range' ? 0 : row) / 50) * 50,
      rowEnd = Math.min(123, rowStart + 50)
    let html =
      '<table><thead><tr>' +
      columns.map((i) => `<th>${metadata.columns[i]}</th>`).join('') +
      '</tr></thead><tbody>' +
      Array.from(
        { length: rowEnd - rowStart },
        (_, i) =>
          '<tr>' +
          columns
            .map(
              (c) =>
                `<td${mode === 'tall' && c === 2 && rowStart + i === 122 ? ' style="height:700px"' : ''}>${c === 2 ? 'Long text 中文😀 ' + 'more '.repeat(40) : `${rowStart + i}:${c}`}<script>window.compromised=true</script></td>`
            )
            .join('') +
          '</tr>'
      ).join('') +
      '</tbody></table>'
    if (mode === 'oversized') html = 'x'.repeat(131073)
    return {
      revision: mode === 'stale' ? 'foreign-revision' : revision,
      result: {
        html,
        rowStart,
        rowEnd,
        totalRows: mode === 'changed-total' ? 124 : 123,
        pageSize: 50
      }
    }
  }
  const mount = (area, presentation, translate) =>
    mountTableView(area.article, {
      ...area,
      metadata,
      translate,
      revision,
      table: 7,
      state: presentation,
      load: provider,
      readWidths: () => widths,
      saveWidths: (value) => {
        widths.clear()
        for (const [k, v] of value) widths.set(k, v)
      },
      onChange: () => {
        if (mode === 'unstable' && layoutFrame == null) {
          let frame = 0
          const shift = () => { area.article.style.marginTop = `${frame % 2}px`; if (frame++ < 20) layoutFrame = requestAnimationFrame(shift) }
          layoutFrame = requestAnimationFrame(shift)
        }
        if (mode === 'late-layout' && !shifted) {
          shifted = true
          requestAnimationFrame(() => {
            const spacer = document.createElement('div');spacer.style.height = '59px'
            area.viewport.prepend(spacer)
          })
        }
      },
      onQuery: () => queries++,
      onChart: () => charts++
    })
  try {
    reader = mount(left, state)
    other = mount(right, { columns: [0, 1], row: 100, height: 0 })
    await Promise.all([reader.ready, other.ready])
    await wait(
      () => getComputedStyle(left.article.querySelector('.table-controls')).display === 'flex'
    )
    check(
      left.article.querySelectorAll('tbody tr').length === 50 &&
        right.article.querySelectorAll('tbody tr').length === 23,
      'Hosts lost independent row windows'
    )
    check(
      !window.compromised && !left.article.querySelector('script'),
      'Provider HTML executed script'
    )
    check(
      getComputedStyle(outside).borderRadius === outsideBefore,
      'Public styles changed host controls'
    )
    check(
      left.article.querySelectorAll('.table-column-resize').length === 3,
      'Shared resize handles missing'
    )
    const handle = left.article.querySelector('.table-column-resize')
    handle.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    check(widths.has(0), 'Keyboard width adjustment did not persist original column')
    const manual = widths.get(0)
    left.article.querySelector('.table-query-open').click()
    left.article.querySelector('.table-chart-open').click()
    check(queries === 1 && charts === 1, 'Host action slots missing')
    const input = left.article.querySelector('.table-page-number')
    input.value = '3'
    input.dispatchEvent(new Event('change'))
    await wait(() => left.article.dataset.rowStart === '100')
    check(
      left.article.querySelectorAll('tbody tr').length === 23 && widths.get(0) === manual,
      'Independent page lost rows or widths'
    )
    const first = left.article.querySelector('tbody td')
    first.focus()
    check(
      reader.selection().location.table === 7 && reader.selection().location.row === 100,
      'Cell coordinates lost source identity'
    )
    const picker = left.article.querySelector('.column-picker')
    picker.open = true
    await tick()
    const boxes = picker.querySelectorAll('.column-choices input')
    boxes[2].checked = false
    boxes[2].dispatchEvent(new Event('change'))
    picker.querySelector('.column-panel-footer button').click()
    await wait(
      () =>
        left.article.dataset.selectedColumns === '[0,1]' && left.article.dataset.ready === 'true'
    )
    check(
      left.article.querySelectorAll('thead th').length === 2,
      'Column selection did not project the provider'
    )
    await reader.go(122, 2)
    check(
      reader.selection()?.location.column === 2 && reader.selection().location.row === 122,
      'Navigation did not reveal/focus an original hidden column'
    )
    check(left.article.querySelectorAll('thead th').length === 3, 'Hidden target remained hidden')
    check(
      left.article.querySelectorAll('.column-choices input')[2].checked,
      'A revealed column stayed unchecked in Columns'
    )
    const focused = reader.selection().viewportRect,
      area = left.viewport.getBoundingClientRect()
    check(
      focused.y >= area.top - 1 &&
        focused.y + focused.height <= area.bottom + 1 &&
        right.viewport.scrollTop === 0,
      'Ordinary cell navigation did not scroll its own viewport'
    )
    mode = 'late-layout'
    await reader.go(122, 1)
    for (let frame = 0; frame < 4; frame++) {
      await new Promise(requestAnimationFrame)
      const cell = reader.selection().viewportRect, viewport = left.viewport.getBoundingClientRect()
      check(cell.y >= viewport.top - 1 && cell.y + cell.height <= viewport.top + left.viewport.clientHeight + 1,
        'Host layout frame clipped the selected cell')
    }
    mode = 'tall'
    await reader.go(122, 2)
    check(reader.selection().viewportRect.height > left.viewport.clientHeight &&
      Math.abs(reader.selection().viewportRect.y - left.viewport.getBoundingClientRect().top) < 1,
      'An oversized cell was not aligned to the reading viewport top')
    mode = 'unstable'
    const started = performance.now()
    await reader.go(122, 1).then(() => { throw Error('Unstable layout falsely reported success') }, error =>
      check(error.code === 'BUSY', 'Unstable layout did not produce a bounded retryable error'))
    check(performance.now() - started < 1000, 'Layout stabilization exceeded its bounded deadline')
    cancelAnimationFrame(layoutFrame);layoutFrame = null;left.article.style.marginTop = ''
    mode = 'normal'
    await reader.go(122, 1)
    mode = 'defer-first'
    const earlier = reader.go(120, 0)
    await tick()
    const delayed = pending
    await reader.go(122, 1)
    delayed(await provider({ row: 120, columns: [0, 1, 2], signal: new AbortController().signal }))
    await earlier
    check(
      reader.selection()?.location.row === 122 && reader.selection().location.column === 1,
      'A late navigation stole the latest focused cell'
    )
    mode = 'defer-first'
    const signal = new AbortController(), beforeCancelled = left.article.querySelector('tbody').textContent
    const aborted = reader.go(0, 0, {signal:signal.signal}).then(()=>false,error=>error.name === 'AbortError')
    await tick(); const cancelledResponse = pending; signal.abort()
    cancelledResponse(await provider({row:0,columns:[0,1,2],signal:new AbortController().signal}))
    check(await aborted, 'Aborted table navigation did not reject')
    check(left.article.querySelector('tbody').textContent === beforeCancelled && !reader.state().loading,
      'Provider ignoring abort published a cancelled row window')
    const scrollProperty = Object.getOwnPropertyDescriptor(Element.prototype, 'scrollTop'), scrollWrites = []
    Object.defineProperty(left.viewport, 'scrollTop', {
      configurable:true, get() {return scrollProperty.get.call(this)},
      set(value) {scrollWrites.push(value);scrollProperty.set.call(this,value)}
    })
    const moving = reader.go(120, 0)
    await new Promise(requestAnimationFrame)
    left.viewport.scrollTop = Math.max(0,left.viewport.scrollTop - left.viewport.clientHeight * 2)
    left.viewport.dispatchEvent(new WheelEvent('wheel',{bubbles:true,deltaY:-left.viewport.clientHeight * 2}))
    const writesAtInput = scrollWrites.length
    check(await moving === false, 'User input did not interrupt table navigation')
    await tick()
    // A generic host may let WebKit anchor detached focused rows natively.
    // The component must issue no later scroll write; the real reader contract
    // separately verifies the actual viewport stays at the user's position.
    check(scrollWrites.length === writesAtInput, 'An interrupted reveal issued another scroll write: '+JSON.stringify(scrollWrites))
    delete left.viewport.scrollTop
    cancelled = 0
    for (const invalid of ['wrong-range', 'changed-total']) {
      mode = invalid
      await reader.go(122)
      check(
        reader.state().error?.includes('row range') &&
          left.article.querySelector('tbody td').textContent === '100:0',
        'An invalid provider range displaced the last valid grid'
      )
    }
    mode = 'stale'
    await reader.go(0)
    check(
      left.article.querySelector('[role="status"]').textContent.includes('revision'),
      'Foreign revision did not produce an explicit error'
    )
    check(
      left.article.querySelector('tbody td').textContent === '100:0',
      'Foreign response replaced the last valid grid'
    )
    mode = 'oversized'
    await reader.go(0)
    check(
      reader.state().error?.includes('bounded') &&
        left.article.querySelector('tbody td').textContent === '100:0',
      'Oversized provider window displaced valid content'
    )
    mode = 'pending'
    const disposing = reader.go(50)
    await tick()
    reader.dispose()
    reader.dispose()
    check(cancelled === 1, 'Dispose did not abort the outstanding provider')
    pending({
      revision,
      result: { html: '<table></table>', rowStart: 0, rowEnd: 0, totalRows: 0, pageSize: 50 }
    })
    await disposing
    await tick()
    check(
      !left.article.querySelector('table') && !left.workspace.querySelector('.table-scrollbar'),
      'Late provider resurrected disposed content/tracks'
    )
    check(
      !left.article.hasAttribute('data-marknoria-table-view'),
      'Dispose retained owned root marker'
    )
    mode = 'normal'
    const { createTranslator } = await import(
      new URL('./translation.mjs', new URL(moduleUrl, location.href))
    )
    const translate = createTranslator({
      locale: 'zh-CN',
      messages: {
        Columns: '列',
        Chart: '图表',
        Retry: '重试',
        Fixture: 'WRONG DATA',
        '0:0': 'WRONG DATA'
      }
    })
    reader = mount(left, state, translate)
    await reader.ready
    check(
      left.article.querySelector('.table-chart-open').textContent === '图表' &&
        left.article.querySelector('.table-label').textContent === 'Fixture' &&
        left.article.querySelector('tbody td').textContent ===
          `${Math.floor(state.row / 50) * 50}:0`,
      'Projected table labels changed literal data or ignored translation'
    )
    const literal = left.article.querySelector('tbody td').textContent
    const callsBefore = calls.length
    translate.update({
      locale: 'fr',
      messages: {
        Columns: 'Colonnes',
        Chart: 'Graphique',
        Retry: 'Réessayer',
        [literal]: 'WRONG DATA'
      }
    })
    check(
      left.article.querySelector('.table-chart-open').textContent === 'Graphique' &&
        left.article.querySelector('.table-load-error button').textContent === 'Réessayer' &&
        calls.length === callsBefore &&
        left.article.querySelector('tbody td').textContent === literal,
      'Locale update refetched the table or missed existing controls'
    )
    const fillers = Array.from({ length: 256 - translate.state().subscribers }, () =>
        translate.subscribe(() => {})
      ),
      blocked = create()
    let failed = false
    try {
      mount(blocked, { columns: [0, 1, 2], row: 0, height: 0 }, translate)
    } catch {
      failed = true
    }
    check(
      failed &&
        !blocked.article.hasAttribute('data-marknoria-table-view') &&
        !blocked.article.querySelector('.table-controls'),
      'Failed table setup changed its host root'
    )
    for (const remove of fillers) remove()
    const partialFill = Array.from({ length: 253 - translate.state().subscribers }, () =>
        translate.subscribe(() => {})
      ),
      partial = create()
    failed = false
    try {
      mount(partial, { columns: [0, 1, 2], row: 0, height: 0 }, translate)
    } catch {
      failed = true
    }
    check(
      failed &&
        !partial.article.querySelector('.table-controls') &&
        !partial.article.hasAttribute('data-marknoria-table-view') &&
        translate.state().subscribers === 253,
      'Partial table setup retained subscriptions or changed the host'
    )
    for (const remove of partialFill) remove()

    check(
      left.article.querySelectorAll('.table-controls').length === 1 && widths.get(0) === manual,
      'Same-node remount duplicated controls or lost widths'
    )
    other.dispose()
    reader.dispose()
    check(translate.state().subscribers === 0, 'Table disposal retained translation subscribers')
    check(
      document.querySelectorAll('link[href$="/table-view.css"]').length === existingStyles,
      'Dispose changed other table owners or retained its stylesheet'
    )
    return {
      assertions,
      providerWindows: calls.length,
      independentHosts: true,
      hiddenColumnNavigation: true,
      lifecycle: true
    }
  } finally {
    cancelAnimationFrame(layoutFrame)
    reader?.dispose()
    other?.dispose()
    host.remove()
    outside.remove()
  }
}
