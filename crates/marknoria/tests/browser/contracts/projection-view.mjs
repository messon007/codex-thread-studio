export async function projectionViewContract(moduleUrl) {
  const { mountProjectionView, readerProjectionProvider } = await import(moduleUrl)
  const { projectionHtml } = await import(
    new URL('./projection-html.mjs', new URL(moduleUrl, location.href))
  )
  let assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const rejects = async (task, pattern) => {
    try {
      await task()
      throw Error('Expected rejection')
    } catch (error) {
      check(pattern.test(String(error)), String(error))
    }
  }
  const wait = async (predicate) => {
    const deadline = performance.now() + 5000
    while (!predicate()) {
      if (performance.now() > deadline) throw Error('Projection timed out')
      await new Promise((resolve) => setTimeout(resolve, 20))
    }
  }
  const host = document.createElement('section')
  const previousStyles = [
    ...document.querySelectorAll(
      'link[href$="/document-prose.css"],link[href$="/projection-view.css"],link[href$="/table-view.css"]'
    )
  ]
  host.style.cssText = 'position:fixed;inset:10px;background:white;overflow:auto'
  document.body.append(host)
  const revision = 'a'.repeat(64),
    requests = [],
    selected = []
  let mode = 'normal',
    deferred,
    aborted = 0
  const sourcePage = (page, html, table = null) => ({
    page,
    sourceLine: page + 1,
    sourceStartByte: page * 100,
    sourceEndByte: (page + 1) * 100,
    table,
    rowStart: 0,
    html
  })
  const pages = [
    sourcePage(
      0,
      '<h1>中文😀</h1><p>Literal 001 <a href="https://example.org">link</a><script>globalThis.compromised=true</script><img src="https://example.org/never"><span onclick="alert(1)">safe</span></p>'
    ),
    sourcePage(1, '<table><tr><td>partial rows</td></tr></table>', 0),
    sourcePage(2, '<table><tr><td>more partial rows</td></tr></table>', 0),
    sourcePage(3, '<p>EOF sentinel</p>')
  ]
  const provider = {
    documentId: 'document-one',
    revision,
    window: async ({ start, count, signal }) => {
      requests.push([start, count])
      signal.addEventListener('abort', () => aborted++, { once: true })
      if (mode === 'defer') await new Promise((resolve) => (deferred = resolve))
      const result = {
        revision,
        totalPages: 4,
        pages: pages.slice(start, start + count).map((value) => ({ ...value }))
      }
      if (mode === 'revision') result.revision = 'b'.repeat(64)
      if (mode === 'total') result.totalPages = 5
      if (mode === 'metadata') result.pages[0].sourceEndByte = -1
      if (mode === 'large') result.pages[0].html = '😀'.repeat(33000)
      if (mode === 'nodes') result.pages[0].html = '<b>x</b>'.repeat(9000)
      return result
    },
    table: async (table) => ({
      table,
      revision,
      metadata: { columns: ['Original', 'Value'], rows: 101 },
      load: async ({ row, columns }) => {
        const rowStart = Math.floor(row / 50) * 50,
          rowEnd = Math.min(101, rowStart + 50)
        return {
          revision,
          result: {
            rowStart,
            rowEnd,
            totalRows: 101,
            totalColumns: 2,
            pageSize: 50,
            columns,
            html:
              '<table><thead><tr>' +
              columns.map((i) => `<th>${['Original', 'Value'][i]}</th>`).join('') +
              '</tr></thead><tbody>' +
              Array.from(
                { length: rowEnd - rowStart },
                (_, i) =>
                  '<tr>' +
                  columns.map((c) => `<td>${c === 0 ? rowStart + i : '001 中文😀'}</td>`).join('') +
                  '</tr>'
              ).join('') +
              '</tbody></table>'
          }
        }
      }
    })
  }
  const make = () => {
    const container = document.createElement('div')
    host.append(container)
    return mountProjectionView(container, {
      provider,
      viewport: host,
      onSelection: (value) => selected.push(value)
    })
  }
  const view = make(),
    other = make()
  try {
    await view.show(0, 4)
    await wait(() => view.root.querySelectorAll('tbody tr').length === 50)
    check(view.root.querySelector('h1').textContent === '中文😀', 'Unicode heading lost')
    const range = document.createRange()
    range.selectNodeContents(view.root.querySelector('h1'))
    document.getSelection().removeAllRanges()
    document.getSelection().addRange(range)
    const quote = view.selection()
    check(
      quote.documentId === 'document-one' &&
        quote.revision === revision &&
        quote.quote === '中文😀' &&
        quote.precision === 'approximate' &&
        quote.location.line === 1,
      'Ordinary text selection invented exact source offsets or lost scope'
    )
    document.getSelection().removeAllRanges()
    check(
      view.root.querySelectorAll('[data-marknoria-table-view]').length === 1,
      'Repeated projection fragments became duplicate tables'
    )
    check(
      !view.root.querySelector('script,img,[onclick]') && !globalThis.compromised,
      'Document gained active authority'
    )
    check(view.root.querySelector('a').rel === 'noopener noreferrer', 'Link opener protection lost')
    check(view.root.lastElementChild.textContent === 'EOF sentinel', 'EOF content missing')
    check(
      view.root.querySelector('article').dataset.sourceStart === '0',
      'Original source byte anchor lost'
    )
    check(
      view.root.querySelector('tbody tr').cells[1].textContent === '001 中文😀',
      'Table exact text changed'
    )
    const next =
      view.root.querySelector('.table-next') ||
      view.root.querySelector('button[aria-label="Next page"]')
    check(Boolean(next), 'Independent table pager missing')
    next.click()
    await wait(() => view.root.querySelector('tbody tr')?.cells[0].textContent === '50')
    check(
      view.root.querySelectorAll('tbody tr').length === 50,
      'Table paged through document fragments instead of complete input'
    )
    await other.show(0, 4)
    await wait(() => other.root.querySelector('tbody tr')?.cells[0].textContent === '0')
    check(
      view.root.querySelector('tbody tr').cells[0].textContent === '50',
      'Pane table states leaked'
    )
    for (const failure of ['revision', 'total', 'metadata', 'large', 'nodes']) {
      mode = failure
      await rejects(() => view.show(0, 4), /Invalid|budget/)
      check(
        view.root.querySelector('tbody tr').cells[0].textContent === '50',
        'Rejected candidate replaced valid content'
      )
    }
    mode = 'normal'
    await view.show(3, 1)
    check(view.root.textContent === 'EOF sentinel', 'Window boundary not respected')
    await view.show(0, 4)
    await wait(() => view.root.querySelector('tbody tr')?.cells[0].textContent === '50')
    check(true, 'Table state did not survive remount')
    mode = 'defer'
    const late = view.show(0, 4)
    await wait(() => deferred)
    mode = 'normal'
    await view.show(3, 1)
    deferred()
    await rejects(() => late, /Abort|Projection replaced/)
    check(view.root.textContent === 'EOF sentinel', 'Late window replaced current content')
    await rejects(() => view.show(0, 6), /budget/)
    const calls = []
    const publicProvider = readerProjectionProvider(
      {
        request: async (...args) => {
          calls.push(args)
          return { revision, table: 0, columns: ['ID'], rows: 1 }
        }
      },
      { documentId: 'doc', revision }
    )
    await publicProvider.window({ start: 1, count: 2 })
    const table = await publicProvider.table(0, {})
    await table.load({ row: 0, columns: [0] })
    check(
      calls.map((call) => call[0]).join(',') === 'projectionWindow,tableInfo,tableWindow',
      'Public adapter used private reader access'
    )
    check(
      calls.every((call) => call[1].documentId === 'doc' && call[1].revision === revision),
      'Public adapter lost document scope'
    )
    await rejects(() => projectionHtml('x'.repeat(131073)), /budget/)
    check(aborted > 0, 'Old window requests were not cancelled')
    view.dispose()
    other.dispose()
    check(!host.querySelector('[data-marknoria-projection]'), 'Owned projection DOM leaked')
    check(
      [
        ...document.querySelectorAll(
          'link[href$="/document-prose.css"],link[href$="/projection-view.css"],link[href$="/table-view.css"]'
        )
      ].every((link) => previousStyles.includes(link)),
      'Styles or table tracks leaked'
    )
    await rejects(() => view.show(), /disposed/)
    return { assertions, ordinaryDom: true, completeTable: true, cancel: true, sourceAnchors: true }
  } finally {
    view.dispose()
    other.dispose()
    host.remove()
  }
}
