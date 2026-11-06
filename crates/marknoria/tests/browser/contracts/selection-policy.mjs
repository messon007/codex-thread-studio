// Public ordinary-DOM text policy: object references remain available, while
// popup display requires an actual text range or an explicit host predicate.
export async function selectionPolicyContract(moduleUrl) {
  const { mountNotes } = await import(moduleUrl)
  const { mountTableCells } = await import(new URL('./table-cells.mjs', new URL(moduleUrl, location.href)))
  const host = document.createElement('section')
  host.style.cssText = 'position:fixed;inset:20px;background:white'
  host.innerHTML = '<nav></nav><table><tbody><tr><td tabindex="0">prefix selected suffix</td></tr></tbody></table><p>ordinary selected text</p>'
  document.body.append(host)
  const owner = { documentId: 'owned-document', revision: 'revision', name: 'Selection.md' }
  const cell = host.querySelector('td'), toolbar = host.querySelector('nav')
  const cells = mountTableCells(host, {
    locate: () => ({ kind: 'cell', table: 0, row: 0, column: 0 })
  })
  cells.update()
  let notes, calls = 0, assertions = 0, delaySelection = false, copied = null
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const tick = () => new Promise(resolve => setTimeout(resolve, 120))
  const select = (node, from, to) => {
    const range = document.createRange(); range.setStart(node.firstChild, from); range.setEnd(node.firstChild, to)
    getSelection().removeAllRanges(); getSelection().addRange(range)
  }
  const selection = async () => {
    calls++
    if (delaySelection) await new Promise(resolve => setTimeout(resolve, 200))
    const value = cells.selection({ textOnly: true })
    return value ? { ...owner, ...value } : null
  }
  const mount = (options = {}) => mountNotes({ owner, root: host, toolbar, selection, copy: async text => { copied = text }, selectionAllowed: () => Boolean(cells.selection({ textOnly: true })),
    storage: async () => ({ items: [], total: 0, limit: 64 }), ...options })
  try {
    notes = mount()
    cell.click(); cell.focus()
    await notes.selectionChanged()
    check(cells.selection()?.quote === 'prefix selected suffix', 'Programmatic cell reference disappeared')
    check(!Object.hasOwn(cells.selection(), 'selectionType'), 'Default reference shape changed')
    check(cells.selection({ textOnly: true }) === null, 'Cell focus became text')
    const menu = host.ownerDocument.querySelector('.notes-toolbar')
    check(menu.hidden, 'Cell focus opened a popup')
    select(cell, 7, 15)
    await notes.selectionChanged()
    check(!menu.hidden && cells.selection({ textOnly: true }).quote === 'selected', 'Partial cell text lost its quote')
    check(cells.selection({ textOnly: true }).location.kind === 'cell', 'Partial cell text lost its typed location')
    await tick()
    delaySelection = true
    const copyButton = menu.querySelector('button'), beforeCopy = calls
    copyButton.dispatchEvent(new MouseEvent('pointerup', { bubbles: true }))
    copyButton.click()
    await tick()
    check(copied === 'selected' && calls === beforeCopy, 'Popup pointer activation recaptured and lost its reference')
    copyButton.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', bubbles: true }))
    check(calls === beforeCopy, 'Popup keyboard activation recaptured its reference')
    delaySelection = false

    getSelection().collapse(cell.firstChild, 7)
    await tick()
    check(menu.hidden, 'Collapsed text kept popup')
    select(cell, 7, 15)
    await tick()
    document.dispatchEvent(new Event('selectionchange'))
    window.dispatchEvent(new Event('scroll'))
    await tick()
    check(menu.hidden, 'Pending selection timer reopened popup after scroll')
    getSelection().removeAllRanges()

    cell.focus()
    cell.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    await tick()
    check(cells.selection({ textOnly: true }) === null && menu.hidden, 'Unselected expanded cell opened popup')
    const full = document.querySelector('.table-cell-full')
    select(full, 7, 15)
    await notes.selectionChanged()
    check(!menu.hidden && cells.selection({ textOnly: true }).quote === 'selected', 'Expanded partial text lost owner')
    getSelection().removeAllRanges(); await tick()
    check(menu.hidden, 'Clearing modal text kept popup')
    document.querySelector('.table-cell-dialog button').click()
    notes.dispose()
    notes = mount({ selectionMenu: false })
    const before = calls
    select(cell, 7, 15); cell.dispatchEvent(new Event('pointerup', { bubbles: true })); await tick()
    check(!document.querySelector('.notes-toolbar') && calls === before, 'Host popup ownership retained default DOM/listeners')
    check(toolbar.querySelector('.notes-open'), 'Independent menu policy disabled Notes list')
    notes.dispose()
    notes = mount({ selectionAllowed: () => true, selection: async () => ({ ...owner, quote: 'selected', precision: 'exact', location: { kind: 'source', startByte: 7, endByte: 15 }, viewportRect: { x: 30, y: 100, width: 60, height: 20 } }) })
    getSelection().removeAllRanges()
    await notes.selectionChanged()
    check(!document.querySelector('.notes-toolbar').hidden, 'Trusted remote text policy required private child DOM')
    notes.clearSelection()
    check(document.querySelector('.notes-toolbar').hidden, 'Explicit lifecycle clear kept popup')
    notes.dispose()
    notes = mount({ selectionAllowed: () => true, selection: async () => ({ ...owner, quote: '', precision: 'exact', location: { kind: 'pdf', pages: [{ page: 1, rects: [{ x: 0, y: 0, width: .1, height: .1 }] }] }, viewportRect: { x: 30, y: 100, width: 60, height: 20 } }) })
    await notes.selectionChanged()
    check(!document.querySelector('.notes-toolbar').hidden, 'Explicit host region policy lost PDF annotation support')

    notes.dispose(); notes = mount()
    await notes.selectionChanged()
    check(document.querySelector('.notes-toolbar').hidden, 'Remount restored an expired text range')
    check(![...document.querySelectorAll('.notes-toolbar button')].some(node => /Ask AI/i.test(node.textContent)), 'Default Ask AI appeared')
    return { assertions, textOnly: true, cell: true, expandedCell: true, hostPolicy: true, lifecycle: true }
  } finally { notes?.dispose(); cells.dispose(); host.remove(); getSelection().removeAllRanges() }
}
