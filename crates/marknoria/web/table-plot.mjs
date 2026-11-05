// Bar-chart layout adapted from Codex Thread Studio a2f87a37 (MIT).
// See third-party/README.md; complete-input projection/lifecycle are local.
// Shared DOM plot adapter. Rust/provider owns complete input and numeric values.
import { dialogClose } from './icons.mjs'
import { queryOperation } from './query-operation.mjs'
import { componentStyles } from './component-styles.mjs'
import { uiLabels } from './translation.mjs'
export const tablePlotStyles = (owner) =>
  componentStyles(owner.ownerDocument, new URL('./table-plot.css', import.meta.url))
const SVG = 'http://www.w3.org/2000/svg'
const element = (tag, attributes = {}) => {
  const node = document.createElementNS(SVG, tag)
  for (const [key, value] of Object.entries(attributes)) node.setAttribute(key, value)
  return node
}

export function mountTablePlot(
  container,
  { columns, load, translate = (value) => value, onSelection = () => {}, onState = () => {} } = {}
) {
  if (
    !(container instanceof HTMLElement) ||
    !Array.isArray(columns) ||
    !columns.length ||
    columns.length > 256 ||
    typeof load !== 'function'
  )
    throw Error('Plot requires bounded columns and a complete-input provider')
  const lifetime = new AbortController(),
    options = { signal: lifetime.signal },
    labels = uiLabels(translate, options),
    root = document.createElement('section')
  root.className = 'table-plot'
  const releaseStyles = tablePlotStyles(container)
  let paintLifetime = new AbortController()
  const axes = document.createElement('form'),
    x = document.createElement('select'),
    y = document.createElement('select'),
    draw = document.createElement('button')
  axes.className = 'table-plot-axes'
  labels.set(x, 'Category column', {}, 'aria-label')
  labels.set(y, 'Numeric column', {}, 'aria-label')
  const rowOption = document.createElement('option')
  rowOption.value = ''
  labels.set(rowOption, 'Row number')
  x.append(rowOption)
  columns.forEach((name, index) => {
    for (const selector of [x, y]) {
      const option = document.createElement('option')
      option.value = index
      option.textContent = `${index + 1} · ${String(name).slice(0, 256)}`
      selector.append(option)
    }
  })
  x.value = '0'
  y.value = String(Math.min(1, columns.length - 1))
  draw.type = 'submit'
  labels.set(draw, 'Draw chart')
  axes.append(x, y, draw)
  const status = document.createElement('p')
  status.role = 'status'
  status.className = 'table-plot-status'
  const graphic = document.createElement('div')
  graphic.className = 'table-plot-graphic'
  root.append(axes, status, graphic)
  container.append(root)
  let request,
    generation = 0,
    stopped = false,
    data = null,
    selected = null,
    busy = false,
    error = null
  const state = () => ({
    busy,
    error,
    rows: data?.rows ?? null,
    inputRows: data?.inputRows ?? null,
    datasetScope: data?.datasetScope ?? null
  })
  const emit = () => {
    if (!stopped) onState(state())
  }
  const graphicBindings = []
  function validate(chart) {
    if (
      !chart ||
      !['complete-source-table', 'complete-query-result'].includes(chart.datasetScope) ||
      !Array.isArray(chart.points) ||
      chart.points.length > 2000 ||
      chart.rows !== chart.points.length
    )
      throw Error('Plot provider did not return a complete bounded result')
    const source = chart.source
    if (
      !source ||
      (source.kind === 'table' && (!Number.isSafeInteger(source.table) || source.table < 0)) ||
      (source.kind === 'query' &&
        (typeof source.queryId !== 'string' ||
          !/^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(source.queryId))) ||
      !['table', 'query'].includes(source.kind) ||
      !Number.isSafeInteger(chart.inputRows) ||
      chart.inputRows < 0
    )
      throw Error('Invalid plot source identity')
    let units = 0
    for (const [index, point] of chart.points.entries()) {
      if (
        point.row !== index ||
        !Number.isSafeInteger(point.row) ||
        point.row < 0 ||
        typeof point.label !== 'string' ||
        point.label.length > 512 ||
        (point.value !== null &&
          (!Number.isFinite(point.value) || typeof point.valueText !== 'string')) ||
        (point.valueText != null &&
          (typeof point.valueText !== 'string' || point.valueText.length > 1024))
      )
        throw Error('Invalid bounded plot point')
      units +=
        new TextEncoder().encode(point.label).length +
        new TextEncoder().encode(point.valueText || '').length
      if (units > 128 * 1024) throw Error('Plot provider exceeded its text budget')
    }
  }
  function paint(chart, valueColumn) {
    paintLifetime.abort()
    paintLifetime = new AbortController()
    const paintOptions = { signal: paintLifetime.signal }
    for (const [node, attribute] of graphicBindings) labels.clear(node, attribute)
    graphicBindings.length = 0
    graphic.replaceChildren()
    selected = null
    onSelection(null)
    const numeric = chart.points.filter((point) => point.value != null)
    const description = document.createElement('p')
    description.className = 'table-plot-description'
    const counts = document.createElement('span')
    labels.set(counts, 'All {rows} rows · {input} source rows evaluated', {
      rows: chart.rows,
      input: chart.inputRows
    })
    description.append(counts)
    graphicBindings.push([counts, 'text'])
    if (chart.points.some((point) => point.labelTruncated)) {
      const hint = document.createElement('span')
      labels.set(hint, 'Some labels abbreviated')
      description.append(' · ', hint)
      graphicBindings.push([hint, 'text'])
    }
    graphic.append(description)
    if (!numeric.length) {
      const empty = document.createElement('p')
      labels.set(empty, 'No numeric values to plot')
      graphicBindings.push([empty, 'text'])
      graphic.append(empty)
      return
    }
    const scale = Math.max(...numeric.map((point) => Math.abs(point.value)), 1),
      high = Math.max(0, ...numeric.map((point) => point.value / scale)),
      low = Math.min(0, ...numeric.map((point) => point.value / scale)),
      span = high - low || 1
    const top = (value) => 30 + ((high - value / scale) / span) * 270,
      zero = top(0),
      step = 620 / chart.rows
    const svg = element('svg', {
      viewBox: '0 0 720 360',
      role: 'group'
    })
    labels.set(svg, 'Bar chart', {}, 'aria-label')
    graphicBindings.push([svg, 'aria-label'])
    svg.append(element('line', { x1: 60, y1: zero, x2: 710, y2: zero, stroke: 'currentColor' }))
    const bars = []
    for (const point of chart.points) {
      if (point.value == null) continue
      const rect = element('rect', {
        x: 70 + point.row * step,
        y: Math.min(zero, top(point.value)),
        width: Math.max(0.1, step * 0.8),
        height: Math.max(0.5, Math.abs(zero - top(point.value))),
        rx: Math.min(2, step / 4),
        tabindex: '0',
        role: 'button',
        'data-chart-row': point.row,
        'aria-label': `${point.label}: ${point.valueText}`
      })
      const title = element('title')
      title.textContent = `${point.label}${point.labelTruncated ? '…' : ''}: ${point.valueText}`
      rect.append(title)
      const select = () => {
        for (const bar of bars) bar.classList.remove('selected')
        rect.classList.add('selected')
        const bounds = rect.getBoundingClientRect()
        const location =
          chart.source.kind === 'table'
            ? { kind: 'cell', table: chart.source.table, row: point.row, column: valueColumn }
            : {
                kind: 'queryCell',
                queryId: chart.source.queryId,
                row: point.row,
                column: valueColumn
              }
        selected = {
          quote: point.valueText || '',
          quoteTruncated: false,
          precision: 'exact',
          location,
          viewportRect: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }
        }
        onSelection(selected)
      }
      rect.addEventListener('click', select, paintOptions)
      rect.addEventListener('focus', select, paintOptions)
      rect.addEventListener(
        'keydown',
        (event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            select()
          } else if (['ArrowLeft', 'ArrowRight'].includes(event.key)) {
            event.preventDefault()
            bars[bars.indexOf(rect) + (event.key === 'ArrowLeft' ? -1 : 1)]?.focus()
          }
        },
        paintOptions
      )
      bars.push(rect)
      svg.append(rect)
      if (chart.rows <= 18) {
        const label = element('text', {
          x: 70 + point.row * step + step * 0.4,
          y: 335,
          'text-anchor': 'middle'
        })
        label.textContent = point.label.slice(0, 10)
        svg.append(label)
      }
    }
    const heading = element('text', { x: 60, y: 18 })
    heading.textContent = String(chart.yName || columns[valueColumn]).slice(0, 256)
    svg.append(heading)
    graphic.append(svg)
  }
  async function refresh() {
    const ticket = ++generation
    request?.abort()
    request = new AbortController()
    busy = true
    error = null
    draw.disabled = true
    labels.set(status, 'Computing the complete dataset…')
    emit()
    try {
      const valueColumn = Number(y.value),
        chart = await load({
          x: x.value === '' ? null : Number(x.value),
          y: valueColumn,
          signal: request.signal
        })
      if (stopped || ticket !== generation) return
      validate(chart)
      data = chart
      paint(chart, valueColumn)
      labels.set(
        status,
        chart.datasetScope === 'complete-source-table'
          ? 'Complete source table'
          : 'Complete SQL result'
      )
    } catch (value) {
      if (!stopped && ticket === generation && value.name !== 'AbortError') {
        error = String(value.message || value).slice(0, 800)
        labels.clear(status)
        status.textContent = error
      }
      throw value
    } finally {
      if (!stopped && ticket === generation) {
        busy = false
        draw.disabled = false
        emit()
      }
    }
  }
  axes.addEventListener(
    'submit',
    (event) => {
      event.preventDefault()
      event.stopPropagation()
      void refresh().catch(() => {})
    },
    options
  )
  const ready = refresh()
  void ready.catch(() => {})
  return {
    ready,
    state,
    selection: () => selected,
    refresh,
    dispose() {
      if (stopped) return
      stopped = true
      generation++
      request?.abort()
      lifetime.abort()
      paintLifetime.abort()
      releaseStyles()
      root.remove()
      selected = null
      data = null
    }
  }
}

export function mountTablePlotDialog(container, { columns, load, ...options }) {
  const previous = document.activeElement,
    lifetime = new AbortController(),
    labels = uiLabels(options.translate, { signal: lifetime.signal })
  const dialog = document.createElement('dialog')
  dialog.className = 'table-plot-dialog'
  labels.set(dialog, 'Table chart', {}, 'aria-label')
  const presentation = getComputedStyle(options.presentationRoot || container)
  for (const name of ['panel', 'soft', 'text', 'muted', 'faint', 'border', 'brand', 'brand-soft']) {
    const property = `--reader-${name}`,
      value = presentation.getPropertyValue(property)
    if (value) dialog.style.setProperty(property, value)
  }
  const header = document.createElement('header'),
    title = document.createElement('h2')
  labels.set(title, 'Table chart')
  let plot,
    stopped = false
  const dispose = () => {
    if (stopped) return
    stopped = true
    lifetime.abort()
    plot?.dispose()
    dialog.remove()
    if (previous?.isConnected) previous.focus({ preventScroll: true })
  }
  const closeButton = dialogClose(dispose)
  labels.set(closeButton, 'Close', {}, 'aria-label')
  labels.set(closeButton, 'Close', {}, 'title')
  header.append(title, closeButton)
  const body = document.createElement('div')
  body.className = 'table-plot-body'
  dialog.append(header, body)
  container.append(dialog)
  dialog.addEventListener('cancel', dispose, { once: true })
  try {
    dialog.showModal()
    plot = mountTablePlot(body, {
      columns,
      translate: options.translate,
      onSelection: options.onSelection,
      load
    })
  } catch (error) {
    dispose()
    throw error
  }
  return {
    dispose,
    isOpen: () => !stopped,
    selection: plot.selection,
    ready: plot.ready,
    state: plot.state
  }
}

export function tableChartDialog(index, source, columns, api, options = {}) {
  return mountTablePlotDialog(document.body, {
    ...options,
    columns,
    presentationRoot:
      options.presentationRoot || document.querySelector('.reader-shell') || document.body,
    load: ({ x, y, signal }) =>
      queryOperation(api, index.revision, signal, (operationId) =>
        api('/api/table-chart', signal, {
          method: 'POST',
          body: JSON.stringify({ revision: index.revision, source, x, y, operationId })
        })
      )
  })
}
