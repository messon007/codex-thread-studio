// Shared document/SQL table toolbar: paging, column selection and sizing reset.
import { pagerIcon } from './icons.mjs'
import { uiLabels } from './translation.mjs'

export function tableControls({
  names,
  selected,
  label = 'Table',
  previousLabel = 'Previous page',
  nextLabel = 'Next page',
  pageName = 'Table page',
  labelKey = label === 'Table' ? 'Table' : null,
  translate,
  bounds,
  onApply
}) {
  if (
    !Array.isArray(names) ||
    !names.length ||
    names.length > 512 ||
    names.some((name) => typeof name !== 'string' || name.length > 16384) ||
    names.reduce((sum, name) => sum + name.length, 0) > 65536 ||
    !Array.isArray(selected) ||
    new Set(selected).size !== selected.length ||
    selected.some((column) => !Number.isInteger(column) || column < 0 || column >= names.length)
  )
    throw TypeError('Invalid bounded table controls')
  const lifetime = new AbortController()
  const labels = uiLabels(translate, { signal: lifetime.signal })
  const controls = document.createElement('div')
  controls.className = 'table-controls'
  const position = document.createElement('span')
  position.className = 'table-position'
  const caption = document.createElement('div')
  caption.className = 'table-caption'
  const title = document.createElement('span')
  title.className = 'table-label'
  if (labelKey) labels.set(title, labelKey)
  else title.textContent = label
  caption.append(title, position)
  const actions = document.createElement('div')
  actions.className = 'table-actions'
  const previous = document.createElement('button')
  previous.append(pagerIcon('previous'))
  labels.set(previous, previousLabel, {}, 'title')
  labels.set(previous, previousLabel, {}, 'aria-label')
  const next = document.createElement('button')
  next.append(pagerIcon('next'))
  labels.set(next, nextLabel, {}, 'title')
  labels.set(next, nextLabel, {}, 'aria-label')
  const pageLabel = document.createElement('label')
  pageLabel.className = 'table-page-label'
  const pageInput = document.createElement('input')
  pageInput.type = 'number'
  pageInput.min = '1'
  pageInput.className = 'table-page-number'
  labels.set(pageInput, pageName, {}, 'aria-label')
  const pageCount = document.createElement('span')
  pageLabel.append(pageInput, pageCount)
  const picker = document.createElement('details')
  picker.className = 'column-picker'
  const summary = document.createElement('summary')
  const pickerLabel = document.createElement('span')
  labels.set(pickerLabel, 'Columns')
  summary.append(pickerLabel)
  const columnCount = document.createElement('span')
  columnCount.className = 'column-count'
  summary.append(columnCount)
  const panel = document.createElement('div')
  panel.className = 'column-panel'
  const panelTitle = document.createElement('strong')
  panelTitle.className = 'column-panel-title'
  labels.set(panelTitle, 'Visible columns')
  const search = document.createElement('input')
  search.type = 'search'
  labels.set(search, 'Find column…', {}, 'placeholder')
  labels.set(search, 'Find column', {}, 'aria-label')
  const choices = document.createElement('div')
  choices.className = 'column-choices'
  const all = document.createElement('button')
  labels.set(all, 'Select all')
  const none = document.createElement('button')
  labels.set(none, 'Clear')
  const apply = document.createElement('button')
  labels.set(apply, 'Apply')
  const hint = document.createElement('span')
  hint.className = 'column-selection-hint'
  const draft = new Set(selected)
  const updateSelection = () => {
    apply.disabled = draft.size === 0
    labels.set(hint, '{selected} of {total} selected', {
      selected: draft.size,
      total: names.length
    })
  }
  updateSelection()
  const checkboxes = names.map((name, column) => {
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
    if (name) text.textContent = name
    else labels.set(text, '(unnamed)')
    label.append(checkbox, number, text)
    choices.append(label)
    checkbox.addEventListener('change', () => {
      if (checkbox.checked) draft.add(column)
      else draft.delete(column)
      updateSelection()
    })
    return { label, checkbox, name: name.toLowerCase() }
  })
  search.addEventListener('input', () => {
    const query = search.value.toLowerCase()
    for (const choice of checkboxes) choice.label.hidden = !choice.name.includes(query)
  })
  const select = (selected) => {
    for (const { checkbox } of checkboxes) checkbox.checked = selected
    draft.clear()
    if (selected) names.forEach((_, column) => draft.add(column))
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
  const resetWidths = document.createElement('button')
  resetWidths.className = 'column-width-reset'
  labels.set(resetWidths, 'Reset column widths')
  panel.append(resetWidths)
  picker.append(summary, panel)
  actions.append(previous, pageLabel, next, picker)
  controls.append(caption, actions)
  const placePicker = () => {
    if (!picker.open) return
    const anchor = summary.getBoundingClientRect(),
      area = bounds().getBoundingClientRect()
    const below = area.bottom - anchor.bottom - 16,
      above = anchor.top - area.top - 16
    const upward = below < 340 && above > below
    picker.dataset.side = upward ? 'above' : 'below'
    panel.style.maxHeight = `${Math.max(0, Math.min(430, upward ? above : below))}px`
    panel.style.maxWidth = `${Math.max(0, Math.min(336, area.width - 24))}px`
  }
  apply.addEventListener('click', () => {
    if (!draft.size) return
    picker.open = false
    onApply([...draft].sort((a, b) => a - b))
  })
  summary.addEventListener('click', (event) => {
    event.preventDefault()
    picker.open = !picker.open
    placePicker()
  })
  picker.addEventListener('toggle', () => {
    if (!picker.open) return
    const root = controls.closest('.reader-shell') || controls.parentElement
    for (const open of root.querySelectorAll('.column-picker[open]'))
      if (open !== picker) open.open = false
    placePicker()
  })
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (picker.open && !picker.contains(event.target)) picker.open = false
    },
    { signal: lifetime.signal }
  )
  document.addEventListener(
    'keydown',
    (event) => {
      if (event.key === 'Escape' && picker.open) {
        event.preventDefault()
        event.stopImmediatePropagation()
        picker.open = false
        summary.focus()
      }
    },
    { capture: true, signal: lifetime.signal }
  )
  const viewport = bounds()
  viewport?.addEventListener('scroll', placePicker, { passive: true, signal: lifetime.signal })
  const observer = new ResizeObserver(placePicker)
  if (viewport) observer.observe(viewport)
  let committed = new Set(selected)
  const updateColumns = (columns) => {
    if (
      !Array.isArray(columns) ||
      new Set(columns).size !== columns.length ||
      columns.some((column) => !Number.isInteger(column) || column < 0 || column >= names.length)
    )
      throw TypeError('Invalid selected columns')
    labels.set(columnCount, '{selected}/{total}', { selected: columns.length, total: names.length })
    if (columns.length !== committed.size || columns.some((column) => !committed.has(column))) {
      committed = new Set(columns)
      draft.clear()
      for (const column of columns) draft.add(column)
      for (const { checkbox } of checkboxes)
        checkbox.checked = committed.has(Number(checkbox.value))
      updateSelection()
    }
  }
  updateColumns(selected)
  return {
    controls,
    position,
    caption,
    actions,
    previous,
    next,
    pageLabel,
    pageInput,
    pageCount,
    picker,
    columnCount,
    resetWidths,
    labels,
    counts(start, end, total, pages, columns = selected) {
      labels.set(position, '{start}–{end} of {total} rows', { start, end, total })
      labels.set(pageCount, ' / {pages}', { pages })
      updateColumns(columns)
    },
    columns: updateColumns,
    refresh: placePicker,
    dispose() {
      lifetime.abort()
      observer.disconnect()
      picker.open = false
    }
  }
}
