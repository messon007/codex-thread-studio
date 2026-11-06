// Same interaction contract for ordinary DOM and projected/SQL table providers.
export async function tableCellsContract(moduleUrl) {
  const { mountTableCells } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const translate = createTranslator()
  const root = document.createElement('section'),
    controls = document.createElement('div'),
    content = document.createElement('div')
  root.style.cssText = 'position:fixed;inset:10px;background:white;z-index:9999'
  root.append(controls, content)
  document.body.append(root)
  const grid = document.createElement('table'),
    body = grid.createTBody()
  for (let row = 0; row < 3; row++) {
    const tr = body.insertRow()
    for (let col = 0; col < 2; col++) tr.insertCell().textContent = `${row}:${col} 中文😀`
  }
  body.rows[0].cells[0].setAttribute('tabindex', '7')
  content.append(grid)
  let reader,
    copied = [],
    events = [],
    assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0))
  const mount = () =>
    mountTableCells(content, {
      controls,
      translate,
      locate: (cell) => ({
        kind: 'cell',
        table: 2,
        row: 50 + cell.parentElement.sectionRowIndex,
        column: [1, 7][cell.cellIndex]
      }),
      copy: async (text) => copied.push(text),
      onSelection: (value) => events.push(value)
    })
  try {
    reader = mount()
    reader.update()
    const first = body.rows[0].cells[0]
    first.focus()
    check(
      reader.selection().location.row === 50 && reader.selection().location.column === 1,
      'Cell lost its original coordinates'
    )
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    check(
      document.activeElement === body.rows[0].cells[1] && reader.selection().location.column === 7,
      'Keyboard navigation lost a hidden original column'
    )
    document.activeElement.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true })
    )
    check(reader.selection().location.row === 51, 'Cell navigation lost the paged row offset')
    controls.querySelector('button').click()
    await tick()
    check(copied[0] === '1:1 中文😀', 'Copy changed the cell value')
    document.activeElement.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })
    )
    await tick()
    const modal = document.querySelector('.table-cell-dialog')
    check(
      modal?.open && modal.querySelector('pre').textContent === '1:1 中文😀',
      'Full cell did not preserve exact content'
    )
    const cssDeadline = performance.now() + 3000
    while (getComputedStyle(modal).display !== 'flex' && performance.now() < cssDeadline)
      await tick()
    check(getComputedStyle(modal).display === 'flex', 'Shared cell stylesheet did not load')
    const fullBefore = modal.querySelector('pre').textContent
    check(reader.selection() === null && !modal.querySelector('footer'), 'Opening the cell viewer created a selection or a permanent copy action')
    const selectedText = document.createRange()
    selectedText.setStart(modal.querySelector('pre').firstChild, 4)
    selectedText.setEnd(modal.querySelector('pre').firstChild, fullBefore.length)
    getSelection().addRange(selectedText)
    check(reader.selection().quote === '中文😀' && reader.selection().location.column === 7, 'Cell viewer lost the exact Unicode substring or cell identity')
    getSelection().removeAllRanges()
    check(reader.selection() === null, 'Collapsed cell selection retained an action target')
    translate.update({
      locale: 'zh-CN',
      messages: {
        'Copy cell': '复制单元格',
        'Cell content': '单元格内容',
        Close: '关闭',
        'Row {row} · Column {column}': '第 {row} 行 · 第 {column} 列',
        '1:1 中文😀': 'WRONG DATA'
      }
    })
    check(
      modal.getAttribute('aria-label') === '单元格内容' &&
        modal.querySelector('h2').textContent === '单元格内容' &&
        modal.querySelector('.table-cell-address').textContent === '第 52 行 · 第 8 列',
      'Open full-cell dialog did not update its localized labels'
    )
    check(
      modal.querySelector('pre').textContent === fullBefore,
      'Cell locale update changed literal data'
    )
    modal.querySelector('button[aria-label="关闭"]').click()
    check(translate.state().subscribers === 1, 'Cell dialog retained its translator subscription')
    translate.update({ locale: 'en', messages: {} })
    check(!document.querySelector('.table-cell-dialog'), 'Cell dialog did not close')
    const word = document.createRange()
    word.setStart(first.firstChild, 0); word.setEnd(first.firstChild, 1)
    getSelection().removeAllRanges(); getSelection().addRange(word)
    const double = new MouseEvent('dblclick', { bubbles: true, cancelable: true })
    first.dispatchEvent(double)
    check(!double.defaultPrevented && !document.querySelector('.table-cell-dialog') && getSelection().toString() === '0', 'Double click intercepted native word selection')
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    check(!document.querySelector('.table-cell-dialog'), 'Enter intercepted a real text selection')
    getSelection().removeAllRanges()
    const link = document.createElement('a'); link.href = '#native-link'; link.textContent = 'link'; first.append(link)
    const linkEnter = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
    link.dispatchEvent(linkEnter)
    check(!linkEnter.defaultPrevented && !document.querySelector('.table-cell-dialog'), 'Cell viewer intercepted an interactive descendant')
    link.remove()
    first.textContent = '<script>globalThis.compromised=true</script>\n' + '中文😀'.repeat(9000)
    reader.update()
    check(controls.querySelector('.table-cell-view').disabled, 'Full-cell action was enabled without an owned cell')
    first.click()
    controls.querySelector('.table-cell-view').click()
    await tick()
    const longRange = document.createRange()
    longRange.selectNodeContents(document.querySelector('.table-cell-full'))
    longRange.setStart(document.querySelector('.table-cell-full').firstChild, first.textContent.indexOf('中文') + 2)
    getSelection().addRange(longRange)
    const long = reader.selection()
    check(
      long.quoteTruncated &&
        new TextEncoder().encode(long.quote).length <= 32768 &&
        first.textContent.includes(long.quote),
      'Cell excerpt split Unicode or exceeded its budget'
    )
    const bounds = document.querySelector('.table-cell-dialog').getBoundingClientRect()
    check(
      bounds.left >= 0 &&
        bounds.top >= 0 &&
        bounds.right <= innerWidth &&
        bounds.bottom <= innerHeight,
      'Full cell dialog escaped the viewport'
    )
    check(
      document.querySelector('.table-cell-full').textContent === first.textContent &&
        !document.querySelector('.table-cell-full script'),
      'Full cell was truncated or interpreted as HTML'
    )
    document.querySelector('.table-cell-dialog .dialog-close').click()
    controls.querySelector('button').click()
    await tick()
    check(copied.at(-1) === first.textContent, 'Grid Copy cell lost the complete bounded value')
    reader.update()
    check(
      !reader.selection() &&
        controls.querySelector('button').disabled &&
        !document.querySelector('.table-cell-dialog'),
      'Paging retained stale selection/dialog'
    )
    body.rows[1].cells[0].focus()
    const before = events.length
    reader.dispose()
    reader.dispose()
    check(
      first.getAttribute('tabindex') === '7' && !body.rows[1].cells[1].hasAttribute('tabindex'),
      'Dispose did not restore host focus attributes'
    )
    first.click()
    check(
      events.length === before && controls.children.length === 0,
      'Dispose retained handlers or toolbar controls'
    )
    reader = mount()
    reader.update()
    body.rows[2].cells[1].focus()
    check(
      reader.selection().location.row === 52 && controls.children.length === 2,
      'Same-node remount failed or duplicated controls'
    )
    const extra = body.insertRow()
    for (let i = 0; i < 507; i++) extra.insertCell().textContent = 'x'
    let rejected = false
    try {
      reader.update()
    } catch {
      rejected = true
    }
    check(rejected, 'Ordinary DOM silently accepted an oversized unpaged window')
    reader.dispose()
    check(translate.state().subscribers === 0, 'Cell disposal retained translation bindings')
    return {
      assertions,
      unicode: true,
      completeCellCopy: true,
      originalCoordinates: true,
      lifecycle: true
    }
  } finally {
    reader?.dispose()
    root.remove()
  }
}
