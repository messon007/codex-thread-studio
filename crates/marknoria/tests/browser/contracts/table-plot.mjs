export async function tablePlotContract(moduleUrl) {
  const { mountTablePlot, mountTablePlotDialog } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const translate = createTranslator()
  const root = document.createElement('main')
  root.style.cssText = 'position:fixed;inset:10px;max-width:1100px;background:white;z-index:9999'
  document.body.append(root)
  const data = () => ({
    revision: 'public-revision',
    source: { kind: 'table', table: 3 },
    datasetScope: 'complete-source-table',
    inputRows: 123,
    rows: 123,
    xName: 'Name',
    yName: 'Value',
    points: Array.from({ length: 123 }, (_, row) => ({
      row,
      label: row === 0 ? '<script>literal</script>' : `row ${row} 中文😀`,
      labelTruncated: false,
      value: row === 2 ? null : row === 1 ? -1 : row,
      valueText: row === 2 ? null : String(row === 1 ? -1 : row)
    }))
  })
  let requests = 0, plot,
    mode = 'valid',
    resolveLate,
    lateSignal,
    assertions = 0
  const selections = [],
    states = []
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const until = async (condition, label) => {
    const deadline = performance.now() + 3000
    while (!condition() && performance.now() < deadline) await new Promise((r) => setTimeout(r, 10))
    check(condition(), label)
  }
  try {
    plot = mountTablePlot(root, {
      columns: ['Name', 'Value'],
      translate,
      onSelection: (value) => selections.push(value),
      onState: (value) => states.push(value),
      load: async ({ x, y, signal }) => {
        requests++
        check(x === 0 && y === 1, 'Plot axes lost original column IDs')
        if (mode === 'pending') {
          lateSignal = signal
          return new Promise((resolve) => (resolveLate = resolve))
        }
        const chart = data()
        if (mode === 'invalid') chart.points[0].value = Infinity
        return chart
      }
    })
    await plot.ready
    check(requests === 0 && root.querySelector('button').disabled, 'Opening a chart guessed a numeric column or ran a query')
    const numericColumn = root.querySelector('select[aria-label="Numeric column"]')
    numericColumn.value = '1'
    numericColumn.dispatchEvent(new Event('change'))
    await plot.refresh()
    const svg = root.querySelector('svg')
    await until(() => getComputedStyle(svg).display === 'block', 'Plot stylesheet did not load')
    check(
      plot.state().rows === 123 && root.querySelectorAll('rect').length === 122,
      'Plot used a visible 50-row page or lost null rows'
    )
    const last = root.querySelector('[data-chart-row="122"]')
    check(Boolean(last), 'Complete dataset last row is absent')
    const zero = Number(svg.querySelector('line').getAttribute('y1')),
      negative = root.querySelector('[data-chart-row="1"]')
    check(
      Number(negative.getAttribute('y')) === zero && Number(negative.getAttribute('height')) > 0,
      'Negative values lost their sign'
    )
    check(
      !root.querySelector('svg script') &&
        root.querySelector('title').textContent.includes('<script>literal</script>'),
      'Plot label was interpreted as markup'
    )
    last.focus()
    await until(() => plot.selection()?.location.row === 122, 'Keyboard plot selection failed')
    check(
      JSON.stringify(plot.selection().location) ===
        JSON.stringify({ kind: 'cell', table: 3, row: 122, column: 1 }) &&
        plot.selection().quote === '122',
      'Plot selection invented a source position'
    )
    const plotNode = root.querySelector('svg'),
      selected = plot.selection(),
      beforeRequests = states.length
    translate.update({
      locale: 'zh-CN',
      messages: {
        'Draw chart': '绘制图表',
        'Bar chart': '柱状图',
        'All {rows} rows · {input} source rows evaluated': '全部 {rows} 行 · 计算了 {input} 个源行',
        Name: 'WRONG DATA'
      }
    })
    check(
      root.querySelector('button').textContent === '绘制图表' &&
        plotNode.getAttribute('aria-label') === '柱状图' &&
        root.querySelector('.table-plot-description').textContent ===
          '全部 123 行 · 计算了 123 个源行',
      'Plot UI did not update without redraw'
    )
    check(
      root.querySelector('svg') === plotNode &&
        plot.selection() === selected &&
        states.length === beforeRequests &&
        root.querySelector('select option[value="0"]').textContent === '1 · Name',
      'Plot locale update redrew data, changed names or lost selection'
    )
    mode = 'invalid'
    let rejected = false
    try {
      await plot.refresh()
    } catch {
      rejected = true
    }
    check(
      rejected && plot.state().error && root.querySelector('[data-chart-row="122"]'),
      'Invalid numeric data replaced the last valid plot'
    )
    mode = 'pending'
    const pending = plot.refresh()
    await until(() => Boolean(resolveLate), 'Pending provider was not called')
    const events = states.length
    plot.dispose()
    check(lateSignal.aborted, 'Plot disposal did not cancel the provider')
    resolveLate(data())
    await pending
    await new Promise((r) => setTimeout(r, 20))
    check(
      root.childElementCount === 0 && states.length === events && !plot.selection(),
      'Disposed plot retained DOM, data or emitted late state'
    )
    check(translate.state().subscribers === 0, 'Plot disposal retained translations')
    plot = mountTablePlotDialog(root, {
      columns: ['Name', 'Value'],
      translate,
      load: async () => data()
    })
    await plot.ready
    translate.update({ locale: 'fr', messages: { 'Table chart': 'Graphique', Close: 'Fermer' } })
    check(
      root.querySelector('.table-plot-dialog h2').textContent === 'Graphique' &&
        root.querySelector('button[aria-label="Fermer"]'),
      'Plot dialog chrome did not update its translation'
    )
    plot.dispose()
    check(
      translate.state().subscribers === 0 && !root.querySelector('dialog'),
      'Plot dialog disposal retained its UI subscription'
    )
    const fill = Array.from({ length: 255 }, () => translate.subscribe(() => {}))
    let failed = false
    try {
      mountTablePlotDialog(root, {
        columns: ['Name', 'Value'],
        translate,
        load: async () => data()
      })
    } catch {
      failed = true
    }
    check(
      failed && translate.state().subscribers === 255 && !root.querySelector('dialog'),
      'Failed plot setup retained dialog or subscriptions'
    )
    for (const remove of fill) remove()
    return {
      assertions,
      completeInput: true,
      negativeAndNull: true,
      sourceCoordinates: true,
      cancellation: true,
      lifecycle: true
    }
  } finally {
    plot?.dispose()
    root.remove()
  }
}
