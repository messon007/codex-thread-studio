// Cell interaction adapted from Codex Thread Studio a2f87a37 (MIT).
// See third-party/README.md. The host/provider owns data and source coordinates.
import { captureCommentRange, registerCommentScope } from './comment-ranges.mjs'
import { dialogClose } from './icons.mjs'
import { copyText } from './clipboard.mjs'
import { componentStyles } from './component-styles.mjs'
import { uiLabels } from './translation.mjs'

export function mountTableCells(
  content,
  { controls, locate, loadSource, revision, onSelection = () => {}, translate = (value) => value, copy = copyText, commentOwner = content } = {}
) {
  if (!(content instanceof HTMLElement) || typeof locate !== 'function')
    throw Error('Table cells require a content root and coordinate provider')
  const lifetime = new AbortController(),
    options = { signal: lifetime.signal },
    labels = uiLabels(translate, options)
  const releaseStyles = componentStyles(
    content.ownerDocument,
    new URL('./table-cells.css', import.meta.url)
  )
  let selected = null,
    dialog = null,
    dialogLifetime = null,
    dialogLabels = null,
    stopped = false,
    sourceDialog = false,
    timer
  const tabIndices = new Map()
  let values = new WeakMap()
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'table-cell-copy'
  labels.set(button, 'Copy cell')
  button.disabled = true
  const viewButton = document.createElement('button')
  viewButton.type = 'button'
  viewButton.className = 'table-cell-view'
  labels.set(viewButton, 'View full cell')
  labels.set(viewButton, 'View full cell (Enter on a focused cell)', {}, 'title')
  viewButton.disabled = true
  controls?.append(button, viewButton)
  const notice = document.createElement('div')
  notice.className = 'table-cell-preview-notice'
  notice.setAttribute('role', 'note')
  const noticeText = document.createElement('span'), headerSource = document.createElement('button')
  labels.set(noticeText, 'Some cells show a shortened preview. View the original row for omitted text.')
  headerSource.type = 'button'
  labels.set(headerSource, 'View original header row')
  notice.append(noticeText, headerSource)
  headerSource.addEventListener('click', () => {
    const header = content.querySelector('thead th[data-cell-preview="true"]')
    if (header) viewCell(header, true)
  }, options)
  const preview = cell => cell?.dataset.cellPreview === 'true'
  const copyLabel = () => preview(selected) ? 'Copy preview' : 'Copy cell'
  const viewLabel = () => preview(selected) ? 'View original row' : 'View full cell'
  const close = (restoreFocus = false) => {
    dialogLabels?.dispose()
    dialogLabels = null
    dialogLifetime?.abort()
    dialogLifetime = null
    dialog?.remove()
    dialog = null
    sourceDialog = false
    if (restoreFocus && selected?.isConnected) selected.focus({ preventScroll: true })
  }
  const cellFor = (target) => {
    let cell = target?.closest?.('tbody td')
    while (cell && content.contains(cell)) {
      if (values.has(cell)) return cell
      cell = cell.parentElement?.closest('tbody td')
    }
    return null
  }
  function selection({ textOnly = false } = {}) {
    if (sourceDialog || !selected?.isConnected || !content.contains(selected)) return null
    const location = locate(selected)
    if (!location) return null
    let text = values.get(selected) || '',
      bounds = selected.getBoundingClientRect(),
      selectionType = 'object'
    const current = content.ownerDocument.getSelection(),
      range = current?.rangeCount ? current.getRangeAt(0) : null
    if (dialog) {
      const full = dialog.querySelector('.table-cell-full')
      if (!range || range.collapsed || !full.contains(range.startContainer) || !full.contains(range.endContainer))
        return null
      text = range.toString()
      bounds = range.getBoundingClientRect()
      selectionType = 'text'
    } else if (range && !range.collapsed && selected.contains(range.startContainer) && selected.contains(range.endContainer)) {
      text = range.toString()
      bounds = range.getBoundingClientRect()
      selectionType = 'text'
    }
    if (textOnly && (selectionType !== 'text' || !text.trim())) return null
    // Bound excerpt transport, retaining the full cell in its own dialog/copy.
    let quote = '',
      bytes = 0
    for (const scalar of text) {
      const point = scalar.codePointAt(0),
        size = point < 128 ? 1 : point < 2048 ? 2 : point < 65536 ? 3 : 4
      if (bytes + size > 32768) break
      quote += scalar
      bytes += size
    }
    return {
      location,
      quote,
      quoteTruncated: quote.length < text.length || (selectionType === 'object' && preview(selected)),
      precision: 'exact',
      viewportRect: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }
    }
  }
  function select(cell) {
    if (!cell || stopped) return
    selected?.classList.remove('table-cell-selected')
    selected = cell
    selected.classList.add('table-cell-selected')
    button.disabled = false
    viewButton.disabled = false
    labels.set(button, copyLabel())
    labels.set(viewButton, viewLabel())
    labels.set(viewButton, preview(selected) ? 'View original row (Enter on a focused cell)' : 'View full cell (Enter on a focused cell)', {}, 'title')
    onSelection(selection())
  }
  async function copySelected() {
    if (!selection()) return
    try {
      await copy(values.get(selected) || '')
      if (stopped) return
      labels.set(button, 'Copied')
      clearTimeout(timer)
      timer = setTimeout(() => {
        labels.set(button, copyLabel())
      }, 2000)
    } catch (error) {
      if (!stopped) {
        labels.set(button, 'Copy failed')
        button.title = String(error.message || error).slice(0, 800)
      }
    }
  }
  function viewCell(cell, headerOnly = false) {
    if (!headerOnly) select(cell)
    close()
    sourceDialog = preview(cell)
    dialogLifetime = new AbortController()
    const dialogOptions = { signal: dialogLifetime.signal }
    dialogLabels = uiLabels(translate, dialogOptions)
    dialog = document.createElement('dialog')
    dialog.className = 'table-cell-dialog'
    dialogLabels.set(dialog, sourceDialog ? 'Original source row' : 'Cell content', {}, 'aria-label')
    const presentation = getComputedStyle(content)
    for (const name of [
      'panel',
      'soft',
      'text',
      'muted',
      'faint',
      'border',
      'brand',
      'brand-soft'
    ]) {
      const property = `--reader-${name}`,
        value = presentation.getPropertyValue(property)
      if (value) dialog.style.setProperty(property, value)
    }
    const header = document.createElement('header'),
      title = document.createElement('h2')
    dialogLabels.set(title, sourceDialog ? 'Original source row' : 'Cell content')
    const closeButton = dialogClose(() => close(true))
    dialogLabels.set(closeButton, 'Close', {}, 'aria-label')
    dialogLabels.set(closeButton, 'Close', {}, 'title')
    header.append(title, closeButton)
    const address = document.createElement('p'),
      location = locate(cell)
    address.className = 'table-cell-address'
    if (!sourceDialog) dialogLabels.set(address, 'Row {row} · Column {column}', {
      row: location.row + 1,
      column: location.column + 1
    })
    const full = document.createElement('pre')
    full.className = 'table-cell-full'
    full.tabIndex = 0
    full.textContent = sourceDialog ? '' : values.get(cell) || ''
    if (!sourceDialog) registerCommentScope(full, location, commentOwner)
    dialog.append(header, address, full)
    if (sourceDialog) mountSourceRow(cell, dialog, full, address, dialogLifetime, dialogLabels)
    document.body.append(dialog)
    dialog.addEventListener('cancel', () => close(true), dialogOptions)
    // Update the owning table before Notes handles the same bubbling input.
    // selectionchange is intentionally left to the reader/bridge observer.
    for (const type of ['pointerup', 'keyup'])
      full.addEventListener(type, () => onSelection(selection()), dialogOptions)
    content.ownerDocument.getSelection()?.removeAllRanges()
    dialog.showModal()
    onSelection(null)
  }
  function mountSourceRow(cell, ownedDialog, full, address, lifetime, ui) {
    const row = cell.closest('tr'), start = Number(row?.dataset.sourceStart), end = Number(row?.dataset.sourceEnd)
    const help = document.createElement('p'), status = document.createElement('p'), footer = document.createElement('footer')
    help.className = status.className = 'table-cell-address'
    status.setAttribute('role', 'status')
    ui.set(help, 'Original row syntax, not a complete cell value. Comments are unavailable in this source window; use Source view to select original text.')
    const previous = document.createElement('button'), next = document.createElement('button'), copyPage = document.createElement('button'), retry = document.createElement('button')
    for (const control of [previous, next, copyPage, retry]) control.type = 'button'
    previous.className = 'table-source-previous'; next.className = 'table-source-next'; copyPage.className = 'table-source-copy'; retry.className = 'table-source-retry'
    ui.set(previous, 'Previous source page'); ui.set(next, 'Next source page'); ui.set(copyPage, 'Copy source page'); ui.set(retry, 'Retry')
    footer.append(previous, next, copyPage, retry)
    ownedDialog.insertBefore(help, full)
    ownedDialog.append(status, footer)
    let page = null, request = null, generation = 0, loading = false, query = null
    const live = () => !stopped && !lifetime.signal.aborted && dialog === ownedDialog
    const update = () => {
      previous.disabled = loading || !page || page.start <= start
      next.disabled = loading || !page || page.end >= end
      copyPage.disabled = loading || !page
    }
    const valid = Number.isSafeInteger(start) && Number.isSafeInteger(end) && start >= 0 && end > start
    retry.hidden = true
    update()
    async function load(params) {
      request?.abort()
      const current = request = new AbortController(), abort = () => current.abort(), ticket = ++generation
      lifetime.signal.addEventListener('abort', abort, { once: true })
      query = params; loading = true; retry.hidden = true; update()
      full.setAttribute('aria-busy', 'true')
      ui.set(status, 'Loading source…')
      try {
        const response = await loadSource({ ...params, signal: current.signal })
        if (!live() || current.signal.aborted || ticket !== generation) return
        if (revision !== undefined && response?.revision !== revision) throw Error('Source revision changed; reopen the owning document')
        const value = response?.result
        if (!value || typeof value.text !== 'string' || value.text.length > 128 * 1024 ||
            ![value.start, value.end, value.line, value.endLine].every(Number.isSafeInteger) ||
            value.start < 0 || value.end <= value.start || value.end - value.start > 128 * 1024 || value.line < 1 || value.endLine < value.line)
          throw Error('Invalid bounded source window')
        const bytes = new TextEncoder().encode(value.text)
        if (bytes.length !== value.end - value.start ||
            (params.previous ? value.end !== params.offset || value.start >= params.offset : value.start !== params.offset || value.end <= params.offset))
          throw Error('Source window did not advance from the requested byte boundary')
        const from = Math.max(start, value.start), to = Math.min(end, value.end)
        if (to <= from) throw Error('Source window does not intersect the original row')
        const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes.subarray(from - value.start, to - value.start))
        page = { start: from, end: to, text }
        content.ownerDocument.getSelection()?.removeAllRanges()
        full.textContent = text
        full.scrollTop = full.scrollLeft = 0
        ui.set(address, 'Original row · bytes {start}–{end} · source page {from}–{to}', { start, end, from, to })
        ui.set(status, from > start || to < end ? 'This source page shows part of the original row. Use Previous or Next for omitted text.' : 'This source page contains the entire original row.')
        onSelection(null)
      } catch (error) {
        if (live() && !current.signal.aborted && ticket === generation) {
          ui.clear(status); status.textContent = String(error.message || error).slice(0, 800)
          retry.hidden = false
        }
      } finally {
        lifetime.signal.removeEventListener('abort', abort)
        if (live() && ticket === generation) { loading = false; full.removeAttribute('aria-busy'); update() }
      }
    }
    previous.addEventListener('click', () => { if (!previous.disabled) void load({ offset: page.start, previous: true }) }, { signal: lifetime.signal })
    next.addEventListener('click', () => { if (!next.disabled) void load({ offset: page.end }) }, { signal: lifetime.signal })
    retry.addEventListener('click', () => { if (!loading && query) void load(query) }, { signal: lifetime.signal })
    copyPage.addEventListener('click', async () => {
      if (copyPage.disabled) return
      const text = page.text
      try { await copy(text); if (live()) ui.set(status, 'Copied source page') }
      catch (error) { if (live()) { ui.clear(status); status.textContent = String(error.message || error).slice(0, 800) } }
    }, { signal: lifetime.signal })
    if (!valid) ui.set(status, 'Original row source coordinates are unavailable.')
    else if (typeof loadSource !== 'function') ui.set(status, 'Original source access is unavailable in this host.')
    else void load({ offset: start })
  }
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (
        event.defaultPrevented ||
        !selected ||
        content.contains(event.target) ||
        controls?.contains(event.target) ||
        dialog?.contains(event.target)
      )
        return
      selected.classList.remove('table-cell-selected')
      selected = null
      button.disabled = true
      viewButton.disabled = true
      onSelection(null)
    },
    options
  )
  content.addEventListener(
    'click',
    (event) => {
      const cell = cellFor(event.target)
      if (cell) select(cell)
    },
    options
  )
  content.addEventListener(
    'focusin',
    (event) => {
      const cell = cellFor(event.target)
      if (cell) select(cell)
    },
    options
  )
  // Text gestures and native context menus belong to the browser/host. Full
  // content has an explicit per-table action, shared by all cell providers.
  content.addEventListener(
    'keydown',
    (event) => {
      const cell = cellFor(event.target)
      if (!cell || event.target !== cell || event.isComposing) return
      const current = content.ownerDocument.getSelection()
      if (current && !current.isCollapsed &&
          (content.contains(current.anchorNode) || content.contains(current.focusNode))) return
      if (event.key === 'Enter') {
        event.preventDefault()
        viewCell(cell)
        return
      }
      if (
        (event.ctrlKey || event.metaKey) &&
        !event.altKey &&
        event.key.toLowerCase() === 'c' &&
        !getSelection()?.toString()
      ) {
        event.preventDefault()
        select(cell)
        void copySelected()
        return
      }
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
      const row = cell.parentElement,
        body = row.parentElement
      const target = {
        ArrowLeft: row.cells[cell.cellIndex - 1],
        ArrowRight: row.cells[cell.cellIndex + 1],
        ArrowUp: body.rows[row.sectionRowIndex - 1]?.cells[cell.cellIndex],
        ArrowDown: body.rows[row.sectionRowIndex + 1]?.cells[cell.cellIndex]
      }[event.key]
      if (target) {
        event.preventDefault()
        target.focus()
      }
    },
    options
  )
  button.addEventListener('click', copySelected, options)
  viewButton.addEventListener('click', () => {
    if (selected?.isConnected && content.contains(selected)) viewCell(selected)
  }, options)
  return {
    selection,
    isOpen: () => Boolean(dialog?.open),
    close: () => close(),
    async captureSelection(options = {}, signal) {
      const value = selection(options)
      if (!value) return null
      const current = content.ownerDocument.getSelection()
      const range = current?.rangeCount ? current.getRangeAt(0) : null
      const node = dialog?.querySelector('.table-cell-full') || selected
      if (range && !range.collapsed && node?.contains(range.startContainer) && node.contains(range.endContainer))
        value.textRange = await captureCommentRange(range, [{ node, scope: value.location }], signal)
      return value
    },
    select(target) {
      const cell=cellFor(target)
      if (!cell) return null
      select(cell)
      return selection()
    },
    update() {
      // Ordinary DOM hosts use the same projected-window budget as the service.
      // Validate bounded text/node work before mutating focus attributes.
      const next = new Map()
      let nodes = 0,
        units = 0,
        rows = 0
      for (const body of content.querySelector('table')?.tBodies || [])
        for (const row of body.rows) {
          if (++rows > 512)
            throw Error('Table cell window exceeds 512 rows; use the paged provider')
          for (const cell of row.cells) {
            if (next.size >= 512)
              throw Error('Table cell window exceeds 512 cells; use the paged provider')
            const walker = document.createTreeWalker(
                cell,
                NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT
              ),
              parts = []
            let node
            while ((node = walker.nextNode())) {
              if (++nodes > 32768) throw Error('Table cell window exceeds its node budget')
              if (node.nodeType !== Node.TEXT_NODE) continue
              if ((units += node.length) > 128 * 1024)
                throw Error('Table cell window exceeds its text budget')
              parts.push(node.data)
            }
            next.set(cell, parts.join(''))
          }
        }
      close()
      selected?.classList.remove('table-cell-selected')
      selected = null
      button.disabled = true
      viewButton.disabled = true
      values = new WeakMap(next)
      for (const [cell, original] of tabIndices)
        if (!next.has(cell)) {
          original == null
            ? cell.removeAttribute('tabindex')
            : cell.setAttribute('tabindex', original)
          tabIndices.delete(cell)
        }
      for (const cell of next.keys()) {
        if (!tabIndices.has(cell)) tabIndices.set(cell, cell.getAttribute('tabindex'))
        cell.tabIndex = 0
        registerCommentScope(cell, locate(cell), commentOwner)
      }
      const omitted = content.querySelector('[data-cell-preview="true"]')
      if (omitted) {
        content.before(notice)
        headerSource.hidden = !content.querySelector('thead th[data-cell-preview="true"]')
      } else notice.remove()
      labels.set(button, 'Copy cell')
      labels.set(viewButton, 'View full cell')
      labels.set(viewButton, 'View full cell (Enter on a focused cell)', {}, 'title')
      onSelection(null)
    },
    dispose() {
      if (stopped) return
      stopped = true
      lifetime.abort()
      releaseStyles()
      clearTimeout(timer)
      close()
      selected?.classList.remove('table-cell-selected')
      button.remove()
      viewButton.remove()
      notice.remove()
      selected = null
      for (const [cell, original] of tabIndices)
        original == null
          ? cell.removeAttribute('tabindex')
          : cell.setAttribute('tabindex', original)
      tabIndices.clear()
      values = new WeakMap()
    }
  }
}
