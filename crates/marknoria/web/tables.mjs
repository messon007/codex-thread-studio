import { mountTableView } from './table-view.mjs'
import { tableQuery } from './table-query.mjs'
import { tableChartDialog, tablePlotStyles } from './table-plot.mjs'

export function tableController(index, api, changed, options = {}) {
  const items = []
  const itemForPage = []
  const states = new Map()
  const active = new Map()
  // Session-local presentation state, isolated to this document/revision/pane.
  const widths = new Map()
  let queryDialog = null, chartDialog = null, selectedTable = null
  const releasePlotStyles = index.tables.length ? tablePlotStyles(document.body) : () => {}
  const saveWidths = (id, value) => {
    widths.delete(id)
    if (value.size) widths.set(id, value)
    if (widths.size > 256) widths.delete(widths.keys().next().value)
  }
  for (let page = 0; page < index.pages.length; page += 1) {
    const table = index.pages[page].table
    if (table == null || page === index.tables[table].firstPage) items.push({ page, table })
    itemForPage.push(items.length - 1)
  }

  function stateFor(id) {
    if (!states.has(id)) states.set(id, { columns: index.tables[id].columns.map((_, column) => column), row: 0, height: 0 })
    return states.get(id)
  }

  function target(page, row) {
    const id = index.pages[page].table
    if (id != null) stateFor(id).row = row ?? index.pages[page].rowStart
    return itemForPage[page]
  }

  function columnCount(item) {
    return item.table == null ? index.pages[item.page].columns : stateFor(item.table).columns.length
  }

  function mount(article, item) {
    const id = item.table
    let controller
    controller = mountTableView(article, {
      metadata: index.tables[id], table: id, state: stateFor(id), revision: index.revision,
      viewport: document.querySelector('#viewport'), workspace: document.querySelector('#workspace'), shell: document.querySelector('.reader-shell'),
      load: ({ row, columns, signal }) => api(`/api/table?id=${id}&row=${row}&columns=${columns.join(',')}`, signal),
      readWidths: () => widths.get(id), saveWidths: value => saveWidths(id, value), onChange: changed,
      onSelection: value => { if (value) selectedTable = controller; else if (selectedTable === controller) selectedTable = null; options.onSelection?.(value) },
      translate: options.translate,
      ...(options.queries === false ? {} : {
        onQuery: () => { queryDialog?.dispose(); queryDialog = tableQuery(id, index, api, options) },
        onChart: () => { chartDialog?.dispose(); chartDialog = tableChartDialog(index, { kind: 'table', table: id }, index.tables[id].columns, api, { ...options, presentationRoot: article }) },
      }),
    })
    active.set(article, controller)
  }

  return {
    items, itemForPage, target, columnCount, mount,
    async go(table, row, column) {
      const entry = [...active].find(([article]) => article.isConnected && Number(article.dataset.table) === table)
      if (!entry) throw Error('The requested table is not mounted')
      await entry[1].go(row, column)
      if (entry[1].state().error) throw Error(entry[1].state().error)
      selectedTable = entry[1]
    },
    hasOverlay() { return Boolean(chartDialog?.isOpen() || queryDialog?.isOpen()) },
    closeOverlays() { chartDialog?.dispose(); queryDialog?.dispose() },
    async captureSelection(options = {}, signal) { if (chartDialog?.isOpen()) return options.textOnly ? null : chartDialog.selection(); if (queryDialog?.isOpen()) return queryDialog.captureSelection(options, signal); return selectedTable?.captureSelection(options, signal) || null },
    selection(options = {}) { if (chartDialog?.isOpen()) return options.textOnly ? null : chartDialog.selection(); if (queryDialog?.isOpen()) return queryDialog.selection(options); return selectedTable?.selection(options) || null },
    anchor(article) { return active.get(article)?.anchor() ?? 0 },
    capture(article) { active.get(article)?.capture() },
    refresh(article) { active.get(article)?.refresh() },
    position() { for (const controller of active.values()) controller.position() },
    prune() { for (const [article, controller] of active) if (!article.isConnected) { controller.dispose(); active.delete(article) } },
    dispose() { releasePlotStyles(); chartDialog?.dispose(); queryDialog?.dispose(); for (const controller of active.values()) controller.dispose(); active.clear() },
  }
}
