// Shared projected table view. Providers own complete data and return bounded windows.
import DOMPurify from './vendor/purify.mjs'
import { tableControls } from './table-controls.mjs'
import { tableLayout } from './table-layout.mjs'
import { componentStyles } from './component-styles.mjs'
import { mountTableCells } from './table-cells.mjs'

export function mountTableView(article, options) {
  const metadata = options.metadata
  if (
    !metadata ||
    !Array.isArray(metadata.columns) ||
    !metadata.columns.length ||
    (metadata.rows != null && (!Number.isSafeInteger(metadata.rows) || metadata.rows < 0)) ||
    metadata.columns.length > 512 ||
    metadata.columns.some((name) => typeof name !== 'string' || name.length > 16384) ||
    metadata.columns.reduce((sum, name) => sum + name.length, 0) > 65536
  )
    throw new TypeError('Invalid bounded table metadata')
  if (typeof options.load !== 'function' || typeof options.revision !== 'string')
    throw new TypeError('Table requires a revision and window provider')
  const id = options.table ?? 0
  const state = options.state || { columns: metadata.columns.map((_, i) => i), row: 0, height: 0 }
  if (
    !Array.isArray(state.columns) ||
    !state.columns.length ||
    new Set(state.columns).size !== state.columns.length ||
    state.columns.some((i) => !Number.isInteger(i) || i < 0 || i >= metadata.columns.length) ||
    !Number.isSafeInteger(state.row) ||
    state.row < 0 ||
    (metadata.rows != null && state.row >= Math.max(1, metadata.rows))
  )
    throw new TypeError('Invalid table presentation state')
  const { viewport, workspace = viewport, shell = article } = options
  if (!viewport || !workspace || !shell) throw new TypeError('Table requires an explicit viewport')
  const releaseStyles = componentStyles(
    article.ownerDocument,
    new URL('./table-view.css', import.meta.url)
  )
  const ownedAttributes = [
    'data-table',
    'data-ready',
    'data-loading',
    'data-row-start',
    'data-row-end',
    'data-selected-columns'
  ]
  const originalAttributes = new Map(
    ownedAttributes.map((name) => [name, article.getAttribute(name)])
  )
  const originalColumns = [
    article.style.getPropertyValue('--columns'),
    article.style.getPropertyPriority('--columns')
  ]
  const originalMarker = article.getAttribute('data-marknoria-table-view')
  const changed = (node) => options.onChange?.(node)
  let tools
  try {
    tools = tableControls({
      names: metadata.columns,
      selected: state.columns,
      label: metadata.label || 'Table',
      labelKey: metadata.label ? null : 'Table',
      translate: options.translate,
      bounds: () => viewport,
      onApply: (columns) => {
        state.row = anchorRow()
        state.columns = columns
        move(state.row)
      }
    })
  } catch (error) {
    releaseStyles()
    throw error
  }
  const {
    controls,
    position,
    actions,
    previous,
    next,
    pageLabel,
    pageInput,
    pageCount,
    picker,
    columnCount,
    resetWidths
  } = tools
  const sql = document.createElement('button')
  tools.labels.set(sql, 'SQL')
  sql.className = 'table-query-open'
  tools.labels.set(sql, 'Query and filter all table rows', {}, 'title')
  sql.addEventListener('click', () => options.onQuery?.())
  const chart = document.createElement('button')
  chart.type = 'button'
  tools.labels.set(chart, 'Chart')
  chart.className = 'table-chart-open'
  chart.addEventListener('click', () => options.onChart?.())
  if (options.onQuery) actions.append(sql)
  if (options.onChart) actions.append(chart)
  const content = document.createElement('div')
  content.className = 'table-content'
  content.style.setProperty('--table-page-height', `${state.height}px`)
  const errorNotice = document.createElement('div')
  errorNotice.className = 'table-load-error'
  errorNotice.setAttribute('role', 'status')
  errorNotice.hidden = true
  const errorMessage = document.createElement('span')
  const retry = document.createElement('button')
  tools.labels.set(retry, 'Retry')
  retry.addEventListener('click', () => load())
  errorNotice.append(errorMessage, retry)
  controls.append(errorNotice)
  let layout
  try {
    layout = tableLayout(
      article,
      content,
      state,
      () => new Map(options.readWidths?.() || []),
      (value) => options.saveWidths?.(value),
      changed,
      { viewport, workspace, shell, tableId: id, fillWidth: options.fillWidth, translate: options.translate }
    )
  } catch (error) {
    tools.dispose()
    releaseStyles()
    throw error
  }
  resetWidths.addEventListener('click', () => {
    picker.open = false
    layout.reset()
  })
  let request
  let ticket = 0
  let current
  let loading = false
  let stopped = false
  let failure = null
  let requestedRow = null
  let cells
  try {
    cells = mountTableCells(content, {
      controls: actions,
      locate: (cell) =>
        current
          ? {
              kind: 'cell',
              table: id,
              row: current.rowStart + cell.parentElement.sectionRowIndex,
              column: JSON.parse(article.dataset.selectedColumns || '[]')[cell.cellIndex]
            }
          : null,
      onSelection: (value) => options.onSelection?.(value),
      translate: options.translate
    })
  } catch (error) {
    layout.dispose()
    tools.dispose()
    releaseStyles()
    throw error
  }
  article.setAttribute('data-marknoria-table-view', '')
  article.replaceChildren(controls, content)
  article.dataset.table = id
  article.dataset.loading = 'true'
  article.classList.remove('placeholder')
  const requestKey = () => `${state.row}:${state.columns.join(',')}`
  const updatePager = () => {
    previous.setAttribute('aria-disabled', String(loading || !current || current.rowStart === 0))
    next.setAttribute(
      'aria-disabled',
      String(loading || !current || current.rowEnd >= current.totalRows)
    )
    pageInput.readOnly = loading
  }
  const retainHeight = () => {
    if (!current || current.totalRows <= current.pageSize) return
    state.height = Math.max(state.height, content.getBoundingClientRect().height)
    content.style.setProperty('--table-page-height', `${state.height}px`)
  }
  const anchorRow = () => {
    if (!current || !article.dataset.ready) return state.row
    const top = Math.max(content.getBoundingClientRect().top, viewport.getBoundingClientRect().top)
    const rows = [...content.querySelectorAll('tbody tr')]
    const row = rows.findIndex((row) => row.getBoundingClientRect().bottom > top + 2)
    return row < 0 ? current.rowStart : current.rowStart + row
  }

  async function load() {
    if (stopped) return
    const generation = ++ticket
    request?.abort()
    request = new AbortController()
    requestedRow = requestKey()
    retainHeight()
    loading = true
    updatePager()
    article.dataset.ready = ''
    article.dataset.loading = 'true'
    content.setAttribute('aria-busy', 'true')
    errorNotice.hidden = true
    failure = null
    if (!current) {
      tools.labels.set(content, 'Loading table…')
      changed(article)
    }
    try {
      const columns = [...state.columns],
        requested = state.row
      const response = await options.load({ row: requested, columns, signal: request.signal })
      if (generation !== ticket || !article.isConnected || stopped) return
      if (response.revision !== options.revision)
        throw Error('Table revision changed; remount the owning document')
      const result = response.result
      if (
        !result ||
        typeof result.html !== 'string' ||
        result.html.length > 131072 ||
        new TextEncoder().encode(result.html).length > 131072 ||
        ![result.rowStart, result.rowEnd, result.totalRows, result.pageSize].every(
          Number.isSafeInteger
        ) ||
        result.rowStart < 0 ||
        result.rowEnd < result.rowStart ||
        result.rowEnd > result.totalRows ||
        result.pageSize < 1 ||
        result.pageSize > 50 ||
        result.rowEnd - result.rowStart > result.pageSize
      )
        throw Error('Invalid bounded table window')
      const expectedRows = metadata.rows ?? current?.totalRows
      if (
        (expectedRows != null && result.totalRows !== expectedRows) ||
        (result.totalRows > 0 && (requested < result.rowStart || requested >= result.rowEnd))
      )
        throw Error('Table window does not match its immutable row range')
      const fragment = DOMPurify.sanitize(result.html, {
        RETURN_DOM_FRAGMENT: true,
        USE_PROFILES: { html: true },
        FORBID_TAGS: ['img', 'video', 'audio', 'iframe', 'form', 'style']
      })
      const grid = fragment.querySelector('table')
      if (
        !grid ||
        !grid.tHead ||
        grid.tHead.rows.length !== 1 ||
        fragment.querySelectorAll('table').length !== 1 ||
        grid.tBodies.length !== 1 ||
        grid.tBodies[0].rows.length !== result.rowEnd - result.rowStart ||
        [...grid.rows].some((row) => row.cells.length !== columns.length)
      )
        throw Error('Table window does not match its row/column coordinates')
      if ([...grid.tBodies[0].rows].reduce((sum, row) => sum + row.cells.length, 0) > 512)
        throw Error('Table window exceeds 512 cells')
      const walker = article.ownerDocument.createTreeWalker(
        fragment,
        NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT
      )
      let visited = 0
      while (walker.nextNode())
        if (++visited > 32768) throw Error('Table window exceeds its node budget')
      current = result
      article.dataset.selectedColumns = JSON.stringify(columns)
      article.style.setProperty('--columns', columns.length)
      tools.columns(columns)
      tools.labels.clear(content)
      content.replaceChildren(fragment)
      layout.update()
      cells.update()
      for (const link of content.querySelectorAll('a')) {
        link.target = '_blank'
        link.rel = 'noopener noreferrer'
      }
      for (const row of content.querySelectorAll('tr[data-source-line]')) {
        const line = Number(row.dataset.sourceLine)
        if (Number.isSafeInteger(line) && line > 0)
          tools.labels.set(row, 'Source line {line}', { line }, 'title')
      }
      const pages = Math.max(1, Math.ceil(current.totalRows / current.pageSize))
      pageInput.value = Math.floor(current.rowStart / current.pageSize) + 1
      pageInput.max = pages
      tools.counts(
        current.totalRows ? current.rowStart + 1 : 0,
        current.rowEnd,
        current.totalRows,
        pages,
        columns
      )
      content.dataset.paginated = String(pages > 1)
      previous.hidden = pages === 1
      next.hidden = pages === 1
      pageLabel.hidden = pages === 1
      loading = false
      updatePager()
      content.removeAttribute('aria-busy')
      article.dataset.loading = 'false'
      article.dataset.rowStart = current.rowStart
      article.dataset.rowEnd = current.rowEnd
      article.dataset.ready = 'true'
      changed(article)
      retainHeight()
    } catch (error) {
      if (generation !== ticket || error.name === 'AbortError' || !article.isConnected) return
      loading = false
      updatePager()
      content.removeAttribute('aria-busy')
      article.dataset.loading = 'false'
      if (current) article.dataset.ready = 'true'
      else tools.labels.set(content, 'Table unavailable.')
      failure = String(error.message || error).slice(0, 800)
      errorMessage.textContent = failure
      errorNotice.hidden = false
      changed(article)
    }
  }

  const move = (row) => {
    state.row = row
    load()
  }
  previous.addEventListener('click', () => {
    if (!loading && current?.rowStart > 0) move(Math.max(0, current.rowStart - current.pageSize))
  })
  next.addEventListener('click', () => {
    if (!loading && current && current.rowEnd < current.totalRows) move(current.rowEnd)
  })
  pageInput.addEventListener('change', () => {
    if (loading || !current) return
    const page = Math.min(
      Number(pageInput.max),
      Math.max(1, Math.trunc(Number(pageInput.value)) || 1)
    )
    move((page - 1) * current.pageSize)
  })
  const controller = {
    state: () => ({
      loading,
      error: failure,
      stopped,
      row: current?.rowStart ?? state.row,
      columns: [...state.columns],
      totalRows: current?.totalRows ?? null
    }),
    selection: cells.selection,
    position: layout.position,
    anchor: anchorRow,
    capture: () => {
      state.row = anchorRow()
    },
    refresh: () => {
      if (requestedRow !== requestKey()) load()
      layout.refresh()
      tools.refresh()
    },
    dispose: () => {
      if (stopped) return
      stopped = true
      ticket += 1
      request?.abort()
      cells.dispose()
      layout.dispose()
      tools.dispose()
      releaseStyles()
      controls.remove()
      content.remove()
      for (const [name, value] of originalAttributes) {
        if (value == null) article.removeAttribute(name)
        else article.setAttribute(name, value)
      }
      if (originalColumns[0]) article.style.setProperty('--columns', ...originalColumns)
      else article.style.removeProperty('--columns')
      if (originalMarker == null) article.removeAttribute('data-marknoria-table-view')
      else article.setAttribute('data-marknoria-table-view', originalMarker)
    }
  }
  controller.ready = load()
  controller.go = async (row, column) => {
    if (stopped) throw new DOMException('Table view disposed', 'AbortError')
    if (
      column != null &&
      (!Number.isInteger(column) || column < 0 || column >= metadata.columns.length)
    )
      throw new RangeError('Unknown source column')
    const total = metadata.rows ?? current?.totalRows
    if (!Number.isSafeInteger(row) || row < 0 || (total != null && row >= Math.max(1, total)))
      throw new RangeError('Unknown source row')
    state.row = row
    if (column != null && !state.columns.includes(column))
      state.columns = [...state.columns, column].sort((a, b) => a - b)
    const operation = ticket + 1
    await load()
    if (stopped || operation !== ticket || failure || !current || column == null) return
    const target =
      content.querySelector('tbody')?.rows[row - current.rowStart]?.cells[
        state.columns.indexOf(column)
      ]
    target?.focus({ preventScroll: true })
    if (target) {
      const bounds = target.getBoundingClientRect(),
        area = content.getBoundingClientRect()
      content.scrollLeft +=
        bounds.left < area.left
          ? bounds.left - area.left
          : bounds.right > area.right
            ? bounds.right - area.right
            : 0
      const reading = viewport.getBoundingClientRect(),
        top = reading.top + viewport.clientTop,
        bottom = top + viewport.clientHeight
      const vertical = target.getBoundingClientRect()
      if (vertical.height > viewport.clientHeight || vertical.top < top)
        viewport.scrollTop += vertical.top - top
      else if (vertical.bottom > bottom) viewport.scrollTop += vertical.bottom - bottom
    }
  }
  return controller
}

// Same scoped provider for ordinary DOM hosts; no access to the iframe's DOM.
export async function readerTableProvider(client, owner, table, { signal } = {}) {
  const identity = { documentId: owner.documentId, revision: owner.revision, table }
  const metadata = await client.request('tableInfo', identity, { signal })
  return {
    table,
    revision: owner.revision,
    metadata,
    load: ({ row, columns, signal }) =>
      client.request('tableWindow', { ...identity, row, columns }, { signal })
  }
}
