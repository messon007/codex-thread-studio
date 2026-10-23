import DOMPurify from './vendor/purify.mjs'
import { pagerIcon } from './icons.mjs'

export function tableController(index, api, changed) {
  const items = []
  const itemForPage = []
  const states = new Map()
  const active = new Map()
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
    const controls = document.createElement('div')
    controls.className = 'table-controls'
    const position = document.createElement('span')
    position.className = 'table-position'
    const caption = document.createElement('div')
    caption.className = 'table-caption'
    const title = document.createElement('span')
    title.className = 'table-label'
    title.textContent = 'Table'
    caption.append(title, position)
    const actions = document.createElement('div')
    actions.className = 'table-actions'
    const previous = document.createElement('button')
    previous.append(pagerIcon('previous'))
    previous.title = 'Previous page'
    previous.setAttribute('aria-label', 'Previous page')
    const next = document.createElement('button')
    next.append(pagerIcon('next'))
    next.title = 'Next page'
    next.setAttribute('aria-label', 'Next page')
    const pageLabel = document.createElement('label')
    pageLabel.className = 'table-page-label'
    const pageInput = document.createElement('input')
    pageInput.type = 'number'
    pageInput.min = '1'
    pageInput.className = 'table-page-number'
    pageInput.setAttribute('aria-label', 'Table page')
    const pageCount = document.createElement('span')
    pageLabel.append(pageInput, pageCount)
    const picker = document.createElement('details')
    picker.className = 'column-picker'
    const summary = document.createElement('summary')
    summary.append(document.createTextNode('Columns'))
    const columnCount = document.createElement('span')
    columnCount.className = 'column-count'
    summary.append(columnCount)
    const panel = document.createElement('div')
    panel.className = 'column-panel'
    const panelTitle = document.createElement('strong')
    panelTitle.className = 'column-panel-title'
    panelTitle.textContent = 'Visible columns'
    const search = document.createElement('input')
    search.type = 'search'
    search.placeholder = 'Find column…'
    search.setAttribute('aria-label', 'Find column')
    const choices = document.createElement('div')
    choices.className = 'column-choices'
    const all = document.createElement('button')
    all.textContent = 'Select all'
    const none = document.createElement('button')
    none.textContent = 'Clear'
    const apply = document.createElement('button')
    apply.textContent = 'Apply'
    const hint = document.createElement('span')
    hint.className = 'column-selection-hint'
    const draft = new Set(state.columns)
    const updateSelection = () => {
      apply.disabled = draft.size === 0
      hint.textContent = `${draft.size} of ${metadata.columns.length} selected`
    }
    updateSelection()
    const checkboxes = metadata.columns.map((name, column) => {
      const label = document.createElement('label')
      const checkbox = document.createElement('input')
      checkbox.type = 'checkbox'
      checkbox.checked = draft.has(column)
      checkbox.value = column
      const number = document.createElement('span')
      number.className = 'column-number'
      number.textContent = column + 1
      const text = document.createElement('span')
      text.className = 'column-name'
      text.textContent = name || '(unnamed)'
      label.append(checkbox, number, text)
      choices.append(label)
      checkbox.addEventListener('change', () => {
        if (checkbox.checked) draft.add(column)
        else draft.delete(column)
        updateSelection()
      })
      return { label, checkbox, name: name.toLocaleLowerCase() }
    })
    search.addEventListener('input', () => {
      const query = search.value.toLocaleLowerCase()
      for (const choice of checkboxes) choice.label.hidden = !choice.name.includes(query)
    })
    const select = selected => {
      for (const { checkbox } of checkboxes) checkbox.checked = selected
      draft.clear()
      if (selected) metadata.columns.forEach((_, column) => draft.add(column))
      updateSelection()
    }
    all.addEventListener('click', () => select(true))
    none.addEventListener('click', () => select(false))
    const shortcuts = document.createElement('div')
    shortcuts.className = 'column-shortcuts'
    shortcuts.append(all, none)
    const footer = document.createElement('div')
    footer.className = 'column-panel-footer'
    footer.append(hint, apply)
    panel.append(panelTitle, search, shortcuts, choices, footer)
    picker.append(summary, panel)
    actions.append(previous, pageLabel, next, picker)
    controls.append(caption, actions)
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
      const top = content.getBoundingClientRect().top + (content.querySelector('thead')?.getBoundingClientRect().height || 0)
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
        article.style.setProperty('--columns', state.columns.length)
        columnCount.textContent = `${state.columns.length}/${metadata.columns.length}`
        content.innerHTML = DOMPurify.sanitize(current.html, { USE_PROFILES: { html: true }, FORBID_TAGS: ['img', 'video', 'audio', 'iframe', 'form', 'style'] })
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
        const target = content.querySelectorAll('tbody tr')[state.row - current.rowStart]
        if (target) {
          const headerHeight = content.querySelector('thead')?.getBoundingClientRect().height || 0
          content.scrollTop += target.getBoundingClientRect().top - content.getBoundingClientRect().top - content.clientTop - headerHeight
        }
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
    apply.addEventListener('click', () => {
      state.row = anchorRow()
      state.columns = [...draft].sort((left, right) => left - right)
      picker.open = false
      move(state.row)
    })
    const closePicker = () => { picker.open = false }
    const outside = event => { if (picker.open && !picker.contains(event.target)) closePicker() }
    const escape = event => { if (event.key === 'Escape' && picker.open) { closePicker(); summary.focus() } }
    const placePicker = () => {
      if (!picker.open) return
      const bounds = summary.getBoundingClientRect()
      const viewport = article.closest('#viewport').getBoundingClientRect()
      const below = viewport.bottom - bounds.bottom - 16
      const above = bounds.top - viewport.top - 16
      const upward = below < 340 && above > below
      picker.dataset.side = upward ? 'above' : 'below'
      panel.style.maxHeight = `${Math.min(430, Math.max(150, upward ? above : below))}px`
    }
    picker.addEventListener('toggle', () => {
      if (picker.open) {
        for (const open of article.closest('.reader-shell').querySelectorAll('.column-picker[open]')) if (open !== picker) open.open = false
        placePicker()
      }
    })
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    active.set(article, { anchor: anchorRow, capture: () => { state.row = anchorRow() }, refresh: () => { if (requestedRow !== requestKey()) load(); placePicker() }, stop: () => {
      ticket += 1
      request?.abort()
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape)
    } })
    load()
  }

  return {
    items, itemForPage, target, columnCount, mount,
    anchor(article) { return active.get(article)?.anchor() ?? 0 },
    capture(article) { active.get(article)?.capture() },
    refresh(article) { active.get(article)?.refresh() },
    prune() { for (const [article, controller] of active) if (!article.isConnected) { controller.stop(); active.delete(article) } },
    dispose() { for (const controller of active.values()) controller.stop(); active.clear() },
  }
}
