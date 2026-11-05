export async function tableQueryContract(moduleUrl) {
  const { mountTableQuery } = await import(moduleUrl)
  const revision = 'query-owner',
    columns = ['Name', 'Value', 'Exact'],
    allRows = Array.from({ length: 123 }, (_, i) => [
      `row ${i} 中文😀`,
      String(i),
      i ? '001' : '<script>inert</script>'
    ])
  const root = document.createElement('section')
  document.body.append(root)
  const initialStyles = document.querySelectorAll('link[href$="/table-query.css"]').length
  let assertions = 0,
    currentId,
    late,
    lateSignal,
    closed = [],
    mode = 'normal',
    query,
    requests = 0,
    foreignPageId
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const tick = () => new Promise((resolve) => setTimeout(resolve, 25))
  const wait = async (predicate) => {
    const deadline = performance.now() + 5000
    while (!predicate()) {
      if (performance.now() > deadline) throw Error('Query contract timed out')
      await tick()
    }
  }
  const windowAt = (id, row = 0) => {
    const rowStart = Math.floor(row / 50) * 50,
      rowEnd = Math.min(123, rowStart + 50)
    return {
      id,
      revision,
      columns,
      rows: allRows.slice(rowStart, rowEnd),
      rowStart,
      rowEnd,
      totalRows: 123,
      pageSize: 50,
      elapsedMs: 1
    }
  }
  const provider = {
    schema: async () => {
      requests++
      return { columns, sourceRows: 123, defaultSql: 'SELECT * FROM data' }
    },
    run: async ({ sql, signal }) => {
      requests++
      if (mode === 'pending') {
        lateSignal = signal
        return await new Promise((resolve) => (late = resolve))
      }
      if (sql === 'DELETE FROM data') throw Error('not authorized')
      currentId = crypto.randomUUID()
      const result = windowAt(currentId)
      if (mode === 'renamed') result.columns = ['Other', ...columns.slice(1)]
      if (mode === 'oversized') result.rows[0] = [...result.rows[0].slice(0, 2), 'x'.repeat(131073)]
      return result
    },
    page: async ({ id, row }) => {
      requests++
      const result = windowAt(id, mode === 'wrong-range' ? 0 : row)
      if (mode === 'foreign-page') result.id = foreignPageId = crypto.randomUUID()
      if (mode === 'changed-schema') result.columns = ['Unexpected', ...columns.slice(1)]
      if (mode === 'changed-total') result.totalRows++
      return result
    },
    close: async ({ id }) => closed.push(id),
    chart: async ({ id }) => ({
      revision,
      source: { kind: 'query', queryId: id },
      datasetScope: 'complete-query-result',
      inputRows: 123,
      rows: 123,
      xName: 'Name',
      yName: 'Value',
      points: allRows.map((row, i) => ({
        row: i,
        label: row[0],
        labelTruncated: false,
        value: i,
        valueText: row[1]
      }))
    })
  }
  try {
    query = mountTableQuery(root, { provider, revision })
    await query.ready
    const dialog = root.querySelector('.table-query-dialog'),
      content = dialog.querySelector('.table-content')
    await wait(
      () => getComputedStyle(dialog.querySelector('.table-query-shell')).display === 'grid'
    )
    check(
      query.state().totalRows === 123 && content.querySelectorAll('tbody tr').length === 50,
      'Ordinary query lost complete-result inventory'
    )
    check(
      !content.querySelector('script') &&
        content.querySelector('tbody tr').cells[2].textContent === '<script>inert</script>',
      'SQL text became markup'
    )
    const handle = dialog.querySelector('.table-column-resize')
    handle.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    const width = content.querySelector('th').getBoundingClientRect().width
    await query.go(122)
    check(
      content.querySelectorAll('tbody tr').length === 23 && query.state().rowStart === 100,
      'Query last page unreachable'
    )
    check(
      Math.abs(content.querySelector('th').getBoundingClientRect().width - width) < 1,
      'Query page lost manual widths'
    )
    content.querySelector('tbody tr:last-child td:nth-child(2)').focus()
    check(
      query.selection().location.kind === 'queryCell' &&
        query.selection().location.row === 122 &&
        query.selection().quote === '122',
      'Query cell lost transient result coordinates'
    )
    const id = query.state().id
    await query.execute('DELETE FROM data')
    check(
      query.state().id === id && query.state().message.includes('authorized'),
      'Rejected query replaced valid results'
    )
    mode = 'oversized'
    await query.execute('SELECT * FROM data')
    await tick()
    check(
      query.state().id === id && query.state().message.includes('budget'),
      'Oversized provider result replaced valid results'
    )
    check(closed.includes(currentId), 'Invalid new result was not released')
    mode = 'normal'
    dialog.querySelector('.table-chart-open').click()
    await wait(() => root.querySelectorAll('.table-plot-dialog rect').length === 123)
    root.querySelector('[data-chart-row="122"]').focus()
    check(
      query.selection().location.queryId === id && query.selection().location.row === 122,
      'Plot used a displayed page or wrong query owner'
    )
    root.querySelector('.table-plot-dialog button[aria-label="Close"]').click()
    const bounds = dialog.getBoundingClientRect()
    check(
      bounds.left >= 0 &&
        bounds.top >= 0 &&
        bounds.right <= innerWidth &&
        bounds.bottom <= innerHeight,
      'Ordinary query dialog escaped viewport'
    )
    const other = mountTableQuery(root, { provider, revision })
    try {
      await other.ready
      check(
        other.state().id !== id &&
          query.state().id === id &&
          other.state().rowStart === 0 &&
          query.state().rowStart === 100,
        'Independent SQL windows shared result/page state'
      )
      const otherId = other.state().id
      await other.dispose()
      check(
        closed.includes(otherId) && !closed.includes(id) && query.isOpen(),
        'Closing another SQL window released this result'
      )
    } finally {
      await other.dispose()
    }
    mode = 'pending'
    const pending = query.execute('SELECT * FROM data')
    await tick()
    query.dispose()
    query.dispose()
    check(lateSignal.aborted, 'Dispose did not abort its provider')
    late(windowAt('00000000-0000-4000-8000-000000000001'))
    await pending
    await tick()
    check(
      !root.querySelector('dialog') &&
        closed.includes(id) &&
        closed.includes('00000000-0000-4000-8000-000000000001'),
      'Dispose retained results or resurrected content'
    )
    check(
      document.querySelectorAll('link[href$="/table-query.css"]').length === initialStyles,
      'Dispose changed another style owner or leaked styles'
    )
    mode = 'normal'
    query = mountTableQuery(root, { provider, revision })
    await query.ready
    check(
      root.querySelectorAll('.table-query-dialog').length === 1,
      'Ordinary query remount duplicated overlays'
    )
    const stableId = query.state().id
    for (const invalid of ['foreign-page', 'changed-schema', 'changed-total', 'wrong-range']) {
      mode = invalid
      await query.go(122)
      check(
        query.state().id === stableId &&
          query.state().rowStart === 0 &&
          query.state().message.includes(invalid === 'wrong-range' ? 'requested row' : 'identity'),
        'Invalid result page displaced its immutable dataset'
      )
    }
    check(!closed.includes(foreignPageId), 'A malformed page closed an unowned result')
    mode = 'normal'
    await query.dispose()
    const { createTranslator } = await import(
      new URL('./translation.mjs', new URL(moduleUrl, location.href))
    )
    const translate = createTranslator({
      locale: 'zh-CN',
      messages: { 'Query table with SQL': '查询表格', 'Run query': '执行查询' }
    })
    query = mountTableQuery(root, { provider, revision, translate })
    await query.ready
    const kept = query.state().id
    root.querySelector('.table-chart-open').click()
    await wait(() => root.querySelectorAll('.table-plot-dialog rect').length === 123)
    const fillers = Array.from({ length: 256 - translate.state().subscribers }, () =>
      translate.subscribe(() => {})
    )
    mode = 'renamed'
    await query.execute('SELECT * FROM data')
    await tick()
    check(
      query.state().id === kept &&
        query.state().message.includes('budget') &&
        closed.includes(currentId) &&
        !closed.includes(kept) &&
        Boolean(root.querySelector('.table-plot-dialog')),
      'Exhausted UI setup leaked a new result or discarded the previous result'
    )
    fillers.pop()()
    await query.execute('SELECT * FROM data')
    check(
      query.state().id !== kept &&
        root.querySelector('thead th').textContent === 'Other' &&
        !root.querySelector('.table-plot-dialog'),
      'SQL did not recover after releasing UI capacity'
    )
    const editor = root.querySelector('.table-query-editor'),
      value = editor.value,
      requestsBefore = requests
    translate.update({
      locale: 'fr',
      messages: {
        'Query table with SQL': 'Requête SQL',
        'Run query': 'Exécuter',
        'SQL query': 'Instruction SQL',
        Other: 'WRONG DATA'
      }
    })
    check(
      root.querySelector('h2').textContent === 'Requête SQL' &&
        editor.getAttribute('aria-label') === 'Instruction SQL' &&
        editor.value === value &&
        root.querySelector('thead th').textContent === 'Other' &&
        requests === requestsBefore,
      'Locale update changed SQL/data or refetched results'
    )
    await query.dispose()
    for (const remove of fillers) remove()
    check(
      translate.state().subscribers === 0 &&
        document.querySelectorAll('link[href$="/table-query.css"]').length === initialStyles,
      'Translated SQL disposal retained subscriptions or styles'
    )
    return {
      assertions,
      completeResult: true,
      exactText: true,
      providerCancellation: true,
      lifecycle: true
    }
  } finally {
    query?.dispose()
    root.remove()
  }
}
