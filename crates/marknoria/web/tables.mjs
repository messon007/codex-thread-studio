import DOMPurify from './vendor/purify.mjs'
import { tableControls } from './table-controls.mjs'
import { tableLayout } from './table-layout.mjs'
import { tableQuery } from './table-query.mjs'

export function tableController(index, api, changed) {
  const items = []
  const itemForPage = []
  const states = new Map()
  const active = new Map()
  // Session-local presentation state, isolated to this document/revision/pane.
  const widths = new Map()
  let queryDialog = null
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
    const metadata = index.tables[id]
    const state = stateFor(id)
    const tools=tableControls({names:metadata.columns,selected:state.columns,label:metadata.label||'Table',bounds:()=>document.querySelector('#viewport'),onApply:columns=>{
      state.row=anchorRow();state.columns=columns;move(state.row)
    }})
    const {controls,position,actions,previous,next,pageLabel,pageInput,pageCount,picker,columnCount,resetWidths}=tools
    const sql = document.createElement('button'); sql.textContent = 'SQL'; sql.className = 'table-query-open'; sql.title = 'Query and filter all table rows'
    sql.addEventListener('click', () => { queryDialog?.dispose(); queryDialog = tableQuery(id, index, api) })
    actions.append(sql)
    const content = document.createElement('div')
    content.className = 'table-content'
    content.style.setProperty('--table-page-height', `${state.height}px`)
    const errorNotice = document.createElement('div')
    errorNotice.className = 'table-load-error'
    errorNotice.setAttribute('role', 'status')
    errorNotice.hidden = true
    const errorMessage = document.createElement('span')
    const retry = document.createElement('button')
    retry.textContent = 'Retry'
    retry.addEventListener('click', () => load())
    errorNotice.append(errorMessage, retry)
    controls.append(errorNotice)
    article.replaceChildren(controls, content)
    article.dataset.table = id
    const layout = tableLayout(article, content, state, () => new Map(widths.get(id)), value => saveWidths(id, value), changed)
    resetWidths.addEventListener('click', () => { picker.open = false; layout.reset() })
    article.dataset.loading = 'true'
    article.classList.remove('placeholder')
    let request
    let ticket = 0
    let current
    let loading = false
    let requestedRow = null
    const requestKey = () => `${state.row}:${state.columns.join(',')}`
    const updatePager = () => {
      previous.setAttribute('aria-disabled', String(loading || !current || current.rowStart === 0))
      next.setAttribute('aria-disabled', String(loading || !current || current.rowEnd >= current.totalRows))
      pageInput.readOnly = loading
    }
    const retainHeight = () => {
      if (!current || current.totalRows <= current.pageSize) return
      state.height = Math.max(state.height, content.getBoundingClientRect().height)
      content.style.setProperty('--table-page-height', `${state.height}px`)
    }
    const anchorRow = () => {
      if (!current || !article.dataset.ready) return state.row
      const top = Math.max(content.getBoundingClientRect().top, article.closest('#viewport').getBoundingClientRect().top)
      const rows = [...content.querySelectorAll('tbody tr')]
      const row = rows.findIndex(row => row.getBoundingClientRect().bottom > top + 2)
      return row < 0 ? current.rowStart : current.rowStart + row
    }

    async function load() {
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
      if (!current) { content.textContent = 'Loading table…'; changed(article) }
      try {
        const response = await api(`/api/table?id=${id}&row=${state.row}&columns=${state.columns.join(',')}`, request.signal)
        if (generation !== ticket || !article.isConnected || response.revision !== index.revision) return
        current = response.result
        article.dataset.selectedColumns=JSON.stringify(state.columns)
        article.style.setProperty('--columns', state.columns.length)
        columnCount.textContent = `${state.columns.length}/${metadata.columns.length}`
        content.innerHTML = DOMPurify.sanitize(current.html, { USE_PROFILES: { html: true }, FORBID_TAGS: ['img', 'video', 'audio', 'iframe', 'form', 'style'] })
        layout.update()
        for (const link of content.querySelectorAll('a')) { link.target = '_blank'; link.rel = 'noopener noreferrer' }
        for (const row of content.querySelectorAll('tr[data-source-line]')) row.title = `Source line ${row.dataset.sourceLine}`
        position.textContent = `${(current.totalRows ? current.rowStart + 1 : 0).toLocaleString()}–${current.rowEnd.toLocaleString()} of ${current.totalRows.toLocaleString()} rows`
        const pages = Math.max(1, Math.ceil(current.totalRows / current.pageSize))
        pageInput.value = Math.floor(current.rowStart / current.pageSize) + 1
        pageInput.max = pages
        pageCount.textContent = ` / ${pages.toLocaleString()}`
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
        else content.textContent = 'Table unavailable.'
        errorMessage.textContent = error.message
        errorNotice.hidden = false
        changed(article)
      }
    }

    const move = row => {
      state.row = row
      load()
    }
    previous.addEventListener('click', () => { if (!loading && current?.rowStart > 0) move(Math.max(0, current.rowStart - current.pageSize)) })
    next.addEventListener('click', () => { if (!loading && current && current.rowEnd < current.totalRows) move(current.rowEnd) })
    pageInput.addEventListener('change', () => {
      if (loading || !current) return
      const page = Math.min(Number(pageInput.max), Math.max(1, Math.trunc(Number(pageInput.value)) || 1))
      move((page - 1) * current.pageSize)
    })
    active.set(article, { position: layout.position, anchor: anchorRow, capture: () => { state.row = anchorRow() }, refresh: () => { if (requestedRow !== requestKey()) load(); layout.refresh(); tools.refresh() }, stop: () => {
      ticket += 1
      request?.abort()
      layout.dispose()
      tools.dispose()
    } })
    load()
  }

  return {
    items, itemForPage, target, columnCount, mount,
    anchor(article) { return active.get(article)?.anchor() ?? 0 },
    capture(article) { active.get(article)?.capture() },
    refresh(article) { active.get(article)?.refresh() },
    position() { for (const controller of active.values()) controller.position() },
    prune() { for (const [article, controller] of active) if (!article.isConnected) { controller.stop(); active.delete(article) } },
    dispose() { queryDialog?.dispose(); for (const controller of active.values()) controller.stop(); active.clear() },
  }
}
