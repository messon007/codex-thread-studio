// Same interaction contract for ordinary DOM and projected/SQL table providers.
export async function tableCellsContract(moduleUrl) {
  const { mountTableCells } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const { readerMessages } = await import(new URL('./ui-messages.mjs',new URL(moduleUrl,location.href)))
  const translate = createTranslator({locale:'zh-CN',messages:readerMessages(key=>({
    'View full cell':'查看完整单元格',
    'View full cell (Enter on a focused cell)':'查看完整单元格（聚焦后按 Enter）'
  })[key] || key)})
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
    check(controls.querySelector('.table-cell-view').textContent==='查看完整单元格','Exported catalog lost full-cell label')
    check(controls.querySelector('.table-cell-view').title==='查看完整单元格（聚焦后按 Enter）','Exported catalog lost full-cell tooltip')
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
    await sourcePreviewContract(mountTableCells, translate, check)
    check(translate.state().subscribers === 0, 'Source dialog disposal retained translation bindings')
    return {
      boundedOriginalSource: true,
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

// The explicit source viewer never promotes a bounded preview to a full cell.
async function sourcePreviewContract(mountTableCells, translate, check) {
  const root = document.createElement('section'), controls = document.createElement('div'), content = document.createElement('div')
  root.append(controls, content); document.body.append(root)
  content.innerHTML = '<table><thead><tr><th data-cell-preview="true">Header prefix</th></tr></thead><tbody><tr><td data-cell-preview="true">Prefix 中文😀</td></tr></tbody></table>'
  const cell = content.querySelector('td'), row = cell.parentElement, header = content.querySelector('thead tr')
  const encode = value => new TextEncoder().encode(value), decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true })
  const chunks = ['\ufeff| 中文😀\r\n' + 'a'.repeat(60000), '中😀' + 'b'.repeat(60000), 'TAIL |\r\n']
  const rowText = chunks.join(''), start = 100, end = start + encode(rowText).length
  const boundaries = [start]
  for (const chunk of chunks) boundaries.push(boundaries.at(-1) + encode(chunk).length)
  for (const tr of [row, header]) Object.assign(tr.dataset, { sourceStart: String(start), sourceEnd: String(end), sourceLine: '4', sourceEndLine: '7' })
  let reader, mode = 'normal', pending = [], calls = [], copied = [], selected = []
  const response = ({ offset, previous }) => {
    const index = boundaries.indexOf(offset) - (previous ? 1 : 0)
    if (index < 0 || index >= chunks.length) throw Error('Unexpected source boundary')
    const text = chunks[index] + (index === chunks.length - 1 ? 'ADJACENT ROW MUST NOT APPEAR\n' : '')
    return { revision: 'revision-1', result: { start: boundaries[index], end: boundaries[index] + encode(text).length, line: 4 + index, endLine: 5 + index, text } }
  }
  const loadSource = params => {
    calls.push(params)
    if (mode === 'pending') return new Promise(resolve => pending.push({ params, resolve }))
    const result = response(params)
    if (mode === 'revision') result.revision = 'revision-2'
    if (mode === 'oversized') {
      result.result.text = 'x'.repeat(128 * 1024 + 1)
      result.result.end = result.result.start + result.result.text.length
    }
    return Promise.resolve(result)
  }
  const wait = async predicate => {
    const until = performance.now() + 3000
    while (!predicate()) {
      if (performance.now() > until) throw Error('Source preview contract timed out')
      await new Promise(resolve => setTimeout(resolve, 0))
    }
  }
  const modal = () => document.querySelector('.table-cell-dialog')
  const open = () => { getSelection().removeAllRanges(); cell.click(); controls.querySelector('.table-cell-view').click() }
  const close = () => modal().querySelector('.dialog-close').click()
  try {
    reader = mountTableCells(content, { controls, translate, loadSource, revision: 'revision-1', locate: () => ({ kind: 'cell', table: 0, row: 0, column: 0 }), copy: async text => copied.push(text), onSelection: value => selected.push(value) })
    reader.update(); cell.click()
    check(root.querySelector('.table-cell-preview-notice')?.textContent.includes('shortened preview'), 'Abbreviated cells lack an explicit omission notice')
    check(reader.selection().quote === cell.textContent && reader.selection().quoteTruncated, 'Preview object was represented as a complete cell')
    check(controls.querySelector('.table-cell-copy').textContent === 'Copy preview' && controls.querySelector('.table-cell-view').textContent === 'View original row', 'Preview actions claim full-cell content')
    controls.querySelector('.table-cell-copy').click()
    await wait(() => copied.length === 1)
    check(copied[0] === 'Prefix 中文😀', 'Preview copy included omission UI or invented complete text')
    open(); await wait(() => modal()?.querySelector('pre').textContent.length > 0)
    check(modal().getAttribute('aria-label') === 'Original source row' && modal().querySelector('pre').textContent === chunks[0], 'Source page lost BOM/CRLF/Unicode or used the truncated cell')
    check(modal().querySelector('.table-source-previous').disabled && !modal().querySelector('.table-source-next').disabled, 'Source paging boundaries are incorrect')
    const range = document.createRange(); range.selectNodeContents(modal().querySelector('pre')); getSelection().addRange(range)
    modal().querySelector('pre').dispatchEvent(new Event('pointerup', { bubbles: true }))
    check(reader.selection() === null && await reader.captureSelection() === null && selected.at(-1) === null, 'Raw source syntax produced a false precise cell anchor')
    const original = modal().querySelector('pre').textContent
    translate.update({ locale: 'zh-CN', messages: readerMessagesForSource() })
    check(modal().querySelector('h2').textContent === '原始行' && modal().querySelector('pre').textContent === original, 'Source translation changed literal bytes or failed to update labels')
    translate.update({ locale: 'en', messages: {} })
    modal().querySelector('.table-source-next').click()
    await wait(() => modal().querySelector('pre').textContent === chunks[1])
    check(calls.at(-1).offset === boundaries[1] && modal().querySelectorAll('pre').length === 1, 'Continuation appended unbounded DOM or lost its byte cursor')
    modal().querySelector('.table-source-previous').click()
    await wait(() => modal().querySelector('pre').textContent === chunks[0])
    check(calls.at(-1).previous === true && calls.at(-1).offset === boundaries[1], 'Previous source page did not use the existing backward-window protocol')
    modal().querySelector('.table-source-next').click(); await wait(() => modal().querySelector('pre').textContent === chunks[1])
    modal().querySelector('.table-source-next').click(); await wait(() => modal().querySelector('pre').textContent === chunks[2])
    check(modal().querySelector('.table-source-next').disabled && !modal().textContent.includes('ADJACENT ROW'), 'Last source page leaked an adjacent row or allowed endless Next')
    modal().querySelector('.table-source-copy').click(); await wait(() => copied.length === 2)
    check(copied[1] === chunks[2], 'Copy source page implied a whole cell or copied adjacent bytes')
    close()
    root.querySelector('.table-cell-preview-notice button').click()
    await wait(() => modal()?.querySelector('pre').textContent === chunks[0])
    check(modal().getAttribute('aria-label') === 'Original source row', 'Abbreviated header has no usable source access')
    close()
    mode = 'revision'; open()
    await wait(() => !modal().querySelector('.table-source-retry').hidden)
    check(modal().querySelector('pre').textContent === '' && modal().textContent.includes('revision'), 'Stale source response painted data from another revision')
    mode = 'normal'; modal().querySelector('.table-source-retry').click()
    await wait(() => modal().querySelector('pre').textContent === chunks[0]); close()
    mode = 'oversized'; open(); await wait(() => !modal().querySelector('.table-source-retry').hidden)
    check(modal().querySelector('pre').textContent === '', 'Oversized source provider response reached the DOM'); close()
    mode = 'pending'; open(); const old = pending.at(-1); close()
    check(old.params.signal.aborted, 'Closing the source dialog did not cancel its provider request')
    open(); const fresh = pending.at(-1); fresh.resolve(response(fresh.params))
    await wait(() => modal().querySelector('pre').textContent === chunks[0])
    old.resolve({ revision: 'revision-1', result: { start, end: start + 5, line: 4, endLine: 4, text: 'STALE' } })
    await new Promise(resolve => setTimeout(resolve, 0))
    check(modal().querySelector('pre').textContent === chunks[0], 'Closed source request overwrote a newly opened dialog')
    close(); open(); const onUpdate = pending.at(-1); reader.update()
    check(onUpdate.params.signal.aborted && !modal(), 'Table replacement retained a source request/dialog')
    open(); const onDispose = pending.at(-1); reader.dispose()
    check(onDispose.params.signal.aborted && !modal() && !root.querySelector('.table-cell-preview-notice'), 'Disposal retained source work or omission controls')
    // A minimal third-party host must disclose that source access is unavailable.
    reader = mountTableCells(content, { controls, locate: () => ({ kind: 'cell', table: 0, row: 0, column: 0 }) })
    reader.update(); open()
    check(modal().textContent.includes('Original source access is unavailable') && modal().querySelector('pre').textContent === '', 'Missing source provider masqueraded as a full-cell viewer')
    close(); delete cell.dataset.cellPreview; delete content.querySelector('th').dataset.cellPreview; reader.update()
    check(!root.querySelector('.table-cell-preview-notice'), 'Omission notice survived a complete table replacement')
    // Validate the test's byte math independently of string UTF-16 offsets.
    check(decoder.decode(encode(rowText)) === rowText && end > start + rowText.length, 'Unicode source fixture did not exercise UTF-8 byte boundaries')
  } finally { reader?.dispose(); root.remove(); getSelection().removeAllRanges() }
}
function readerMessagesForSource() {
  return { 'Original source row': '原始行', 'Copy source page': '复制当前源码页' }
}
