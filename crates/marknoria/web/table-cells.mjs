// Cell interaction adapted from Codex Thread Studio a2f87a37 (MIT).
// See third-party/README.md. The host/provider owns data and source coordinates.
import { dialogClose } from './icons.mjs'
import { copyText } from './clipboard.mjs'
import { componentStyles } from './component-styles.mjs'
import { uiLabels } from './translation.mjs'

export function mountTableCells(
  content,
  { controls, locate, onSelection = () => {}, translate = (value) => value, copy = copyText } = {}
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
    timer
  const tabIndices = new Map()
  let values = new WeakMap()
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'table-cell-copy'
  labels.set(button, 'Copy cell')
  button.disabled = true
  controls?.append(button)
  const close = (restoreFocus = false) => {
    dialogLabels?.dispose()
    dialogLabels = null
    dialogLifetime?.abort()
    dialogLifetime = null
    dialog?.remove()
    dialog = null
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
  function selection() {
    if (!selected?.isConnected || !content.contains(selected)) return null
    const location = locate(selected)
    if (!location) return null
    const text = values.get(selected) || '',
      bounds = selected.getBoundingClientRect()
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
      quoteTruncated: quote.length < text.length,
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
        labels.set(button, 'Copy cell')
      }, 2000)
    } catch (error) {
      if (!stopped) {
        labels.set(button, 'Copy failed')
        button.title = String(error.message || error).slice(0, 800)
      }
    }
  }
  function viewCell(cell) {
    select(cell)
    close()
    dialogLifetime = new AbortController()
    const dialogOptions = { signal: dialogLifetime.signal }
    dialogLabels = uiLabels(translate, dialogOptions)
    dialog = document.createElement('dialog')
    dialog.className = 'table-cell-dialog'
    dialogLabels.set(dialog, 'Cell content', {}, 'aria-label')
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
    dialogLabels.set(title, 'Cell content')
    const closeButton = dialogClose(() => close(true))
    dialogLabels.set(closeButton, 'Close', {}, 'aria-label')
    dialogLabels.set(closeButton, 'Close', {}, 'title')
    header.append(title, closeButton)
    const address = document.createElement('p'),
      location = locate(cell)
    address.className = 'table-cell-address'
    dialogLabels.set(address, 'Row {row} · Column {column}', {
      row: location.row + 1,
      column: location.column + 1
    })
    const full = document.createElement('pre')
    full.className = 'table-cell-full'
    full.tabIndex = 0
    full.textContent = values.get(cell) || ''
    const footer = document.createElement('footer'),
      copyButton = document.createElement('button')
    copyButton.type = 'button'
    dialogLabels.set(copyButton, 'Copy cell')
    copyButton.addEventListener('click', copySelected, dialogOptions)
    footer.append(copyButton)
    dialog.append(header, address, full, footer)
    document.body.append(dialog)
    dialog.addEventListener('cancel', () => close(true), dialogOptions)
    dialog.showModal()
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
  content.addEventListener(
    'dblclick',
    (event) => {
      const cell = cellFor(event.target)
      if (cell) {
        event.preventDefault()
        viewCell(cell)
      }
    },
    options
  )
  content.addEventListener(
    'keydown',
    (event) => {
      const cell = cellFor(event.target)
      if (!cell || event.isComposing) return
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
  return {
    selection,
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
      }
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
