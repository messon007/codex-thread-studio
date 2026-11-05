import { projectionHtml } from './projection-html.mjs'
import { mountTableView, readerTableProvider } from './table-view.mjs'
import { componentStyles } from './component-styles.mjs'

export const PROJECTION_WINDOW_PROFILE = Object.freeze({
  version: 1,
  pages: 5,
  pageBytes: 131072,
  nodes: 20000,
  tableStates: 256
})

export function readerProjectionProvider(client, owner) {
  const identity = { documentId: owner.documentId, revision: owner.revision }
  return {
    documentId: owner.documentId,
    revision: owner.revision,
    window: ({ start, count, signal }) =>
      client.request('projectionWindow', { ...identity, start, count }, { signal }),
    table: (table, { signal }) => readerTableProvider(client, owner, table, { signal })
  }
}

// An ordinary DOM window over the immutable Rust projection. The caller owns
// navigation/virtualization and supplies an explicit scrolling element.
export function mountProjectionView(container, options) {
  const { provider, viewport, workspace = viewport, translate } = options
  if (
    !provider ||
    typeof provider.window !== 'function' ||
    typeof provider.revision !== 'string' ||
    typeof provider.documentId !== 'string' ||
    !provider.documentId ||
    provider.documentId.length > 256 ||
    !viewport
  )
    throw new TypeError('Projection requires a scoped provider and explicit viewport')
  const root = container.ownerDocument.createElement('section')
  root.setAttribute('data-marknoria-projection', '')
  const releaseStyles = componentStyles(
    container.ownerDocument,
    new URL('./projection-view.css', import.meta.url)
  )
  const releaseProseStyles = componentStyles(
    container.ownerDocument,
    new URL('./document-prose.css', import.meta.url)
  )
  const lifetime = new AbortController()
  const notify = () => root.dispatchEvent(new Event('marknoria:projectionchange'))
  if (options.onChange)
    root.addEventListener('marknoria:projectionchange', options.onChange, {
      signal: lifetime.signal
    })
  const states = new Map(),
    widths = new Map()
  let disposed = false,
    request,
    ticket = 0,
    current = null,
    mounted = [],
    lastSelection = null,
    selectionFrame = null
  const identify = (value) =>
    value && { ...value, documentId: provider.documentId, revision: provider.revision }
  function selection() {
    if (disposed) return null
    const range = container.ownerDocument.getSelection()?.rangeCount
      ? container.ownerDocument.getSelection().getRangeAt(0)
      : null
    if (
      !range ||
      range.collapsed ||
      !root.contains(range.startContainer) ||
      !root.contains(range.endContainer)
    )
      return lastSelection
    const start =
      range.startContainer.nodeType === 1
        ? range.startContainer
        : range.startContainer.parentElement
    const end =
      range.endContainer.nodeType === 1 ? range.endContainer : range.endContainer.parentElement
    const text = range.toString()
    let length = Math.min(text.length, 16384)
    if (length && /[\uD800-\uDBFF]/.test(text[length - 1])) --length
    const bounds = range.getBoundingClientRect()
    const cell = start.closest('td'),
      article = start.closest('article')
    let location = {
        kind: 'line',
        line:
          Number(start.closest('[data-source-line]')?.dataset.sourceLine) ||
          Number(article?.dataset.sourceLine) ||
          1
      },
      precision = 'approximate'
    if (cell && cell === end.closest('td') && article?.dataset.table != null) {
      const columns = JSON.parse(article.dataset.selectedColumns || '[]')
      location = {
        kind: 'cell',
        table: Number(article.dataset.table),
        row: Number(article.dataset.rowStart) + cell.parentElement.sectionRowIndex,
        column: columns[cell.cellIndex]
      }
      precision = 'exact'
    }
    return identify({
      quote: text.slice(0, length),
      quoteTruncated: length < text.length,
      precision,
      location,
      viewportRect: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }
    })
  }
  container.ownerDocument.addEventListener(
    'selectionchange',
    () => {
      if (selectionFrame != null || disposed) return
      selectionFrame = requestAnimationFrame(() => {
        selectionFrame = null
        const value = selection()
        if (value) options.onSelection?.(value)
      })
    },
    { signal: lifetime.signal }
  )
  container.append(root)
  const close = (records) => {
    for (const record of records) record.dispose?.()
  }
  const stateFor = (table, metadata, row) => {
    let state = states.get(table)
    if (!state) {
      state = { columns: metadata.columns.map((_, i) => i), row, height: 0 }
      states.set(table, state)
      while (states.size > PROJECTION_WINDOW_PROFILE.tableStates) {
        const key = states.keys().next().value
        states.delete(key)
        widths.delete(key)
      }
    }
    return state
  }
  async function show(start = 0, count = 5) {
    if (disposed) throw new DOMException('Projection view disposed', 'AbortError')
    if (
      !Number.isSafeInteger(start) ||
      start < 0 ||
      !Number.isInteger(count) ||
      count < 1 ||
      count > 5
    )
      throw new RangeError('Projection window exceeds its page budget')
    request?.abort()
    const operation = (request = new AbortController()),
      own = ++ticket,
      candidates = []
    const signal = operation.signal
    try {
      const value = await provider.window({ start, count, signal })
      signal.throwIfAborted()
      if (
        value?.revision !== provider.revision ||
        !Number.isSafeInteger(value.totalPages) ||
        value.totalPages < 0 ||
        (current && value.totalPages !== current.totalPages) ||
        !Array.isArray(value.pages) ||
        value.pages.length !== Math.min(count, Math.max(0, value.totalPages - start)) ||
        (start >= value.totalPages && !(start === 0 && value.totalPages === 0))
      )
        throw Error('Invalid immutable projection window')
      const fragment = container.ownerDocument.createDocumentFragment(),
        tables = new Set()
      let nodes = 0
      for (const [index, page] of value.pages.entries()) {
        if (
          !page ||
          page.page !== start + index ||
          !Number.isSafeInteger(page.sourceLine) ||
          page.sourceLine < 1 ||
          (page.table != null && (!Number.isSafeInteger(page.table) || page.table < 0)) ||
          !Number.isSafeInteger(page.rowStart) ||
          page.rowStart < 0 ||
          (page.sourceStartByte == null) !== (page.sourceEndByte == null) ||
          (page.sourceStartByte != null &&
            (!Number.isSafeInteger(page.sourceStartByte) ||
              page.sourceStartByte < 0 ||
              !Number.isSafeInteger(page.sourceEndByte) ||
              page.sourceEndByte < page.sourceStartByte))
        )
          throw Error('Invalid source projection metadata')
        // Validate even duplicate table pages before coalescing their DOM.
        const content = projectionHtml(page.html, container.ownerDocument)
        const walker = container.ownerDocument.createTreeWalker(content, NodeFilter.SHOW_ALL)
        while (walker.nextNode())
          if (++nodes > PROJECTION_WINDOW_PROFILE.nodes)
            throw Error('Projection window exceeds its DOM budget')
        if (page.table != null && tables.has(page.table)) continue
        const article = container.ownerDocument.createElement('article')
        article.setAttribute('data-marknoria-prose', '')
        article.dataset.page = page.page
        article.dataset.sourceLine = page.sourceLine
        if (page.sourceStartByte != null) {
          article.dataset.sourceStart = page.sourceStartByte
          article.dataset.sourceEnd = page.sourceEndByte
        }
        if (page.table != null && typeof provider.table === 'function') {
          tables.add(page.table)
          const table = await provider.table(page.table, { signal })
          signal.throwIfAborted()
          if (table.revision !== provider.revision || table.table !== page.table)
            throw Error('Table provider belongs to another projection')
          candidates.push({ article, table, row: page.rowStart })
        } else article.append(content)
        fragment.append(article)
      }
      signal.throwIfAborted()
      if (own !== ticket || disposed) throw new DOMException('Projection replaced', 'AbortError')
      // Setup can fail; keep the previous window until every component exists.
      for (const candidate of candidates) {
        const { article, table } = candidate
        const view = mountTableView(article, {
          ...table,
          viewport,
          workspace,
          shell: root,
          translate,
          state: stateFor(table.table, table.metadata, candidate.row),
          readWidths: () => widths.get(table.table),
          saveWidths: (value) => widths.set(table.table, new Map(value)),
          fillWidth: options.fillWidth,
          onQuery: options.onQuery ? () => options.onQuery(table.table) : undefined,
          onChart: options.onChart ? () => options.onChart(table.table) : undefined,
          onSelection: (value) => {
            lastSelection = identify(value)
            options.onSelection?.(lastSelection)
          },
          onChange: notify
        })
        candidate.dispose = () => view.dispose()
        candidate.view = view
      }
      close(mounted)
      root.replaceChildren(fragment)
      mounted = candidates
      lastSelection = null
      current = {
        revision: value.revision,
        start,
        count: value.pages.length,
        totalPages: value.totalPages
      }
      for (const candidate of mounted) candidate.view.refresh()
      notify()
      return { ...current }
    } catch (error) {
      close(candidates)
      throw error
    }
  }
  return {
    root,
    show,
    selection,
    state: () => current && { ...current },
    dispose() {
      if (disposed) return
      disposed = true
      lifetime.abort()
      if (selectionFrame != null) cancelAnimationFrame(selectionFrame)
      ++ticket
      request?.abort()
      close(mounted)
      mounted = []
      states.clear()
      widths.clear()
      current = null
      lastSelection = null
      root.remove()
      releaseStyles()
      releaseProseStyles()
    }
  }
}
