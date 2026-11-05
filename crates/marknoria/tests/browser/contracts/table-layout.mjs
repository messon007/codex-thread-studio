// Reusable public DOM contract; shared by upstream and downstream browser/WebView hosts.
export async function tableComponentContract(moduleUrl) {
  const { tableLayout } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const translate = createTranslator()
  const shell = document.createElement('div')
  shell.style.cssText = 'position:fixed;inset:0;background:white;z-index:9999;--column-width:180px'
  const viewport = shell.appendChild(document.createElement('div'))
  viewport.style.cssText = 'height:500px;overflow:auto;position:relative'
  document.body.append(shell)
  const article = viewport.appendChild(document.createElement('article'))
  article.dataset.table = 'shared-contract'
  const content = article.appendChild(document.createElement('div'))
  content.className = 'table-body'
  const table = content.appendChild(document.createElement('table'))
  const head = table.createTHead().insertRow()
  for (const name of ['标识', 'value', '第三列'])
    head.appendChild(document.createElement('th')).textContent = name
  const body = table.createTBody()
  for (let row = 0; row < 260; row++) {
    const tr = body.insertRow()
    for (let col = 0; col < 3; col++) tr.insertCell().textContent = `${row}:${col} 中文😀`
  }
  body.rows[220].cells[0].textContent = 'outside sample '.repeat(3000)
  body.rows[0].cells[1].textContent = 'W'.repeat(20000)
  const original = table.textContent
  let manual = new Map(),
    measured = 0,
    longest = 0,
    checks = 0
  const check = (ok, message) => {
    if (!ok) throw Error(message)
    checks++
  }
  const measure = CanvasRenderingContext2D.prototype.measureText
  CanvasRenderingContext2D.prototype.measureText = function (text) {
    measured++
    longest = Math.max(longest, text.length)
    return measure.call(this, text)
  }
  const state = { columns: [0, 1, 2], horizontal: 0 }
  const mount = (fillWidth) =>
    tableLayout(
      article,
      content,
      state,
      () => new Map(manual),
      (widths) => (manual = widths),
      () => {},
      { viewport, workspace: shell, shell, fillWidth, translate }
    )
  let layout
  try {
    layout = mount(false)
    layout.update()
    check(measured <= 200 * 3, 'Natural measurement exceeded the row budget')
    check(longest <= 16384, 'Natural measurement exceeded the cell budget')
    check(table.querySelectorAll('colgroup').length === 1, 'Missing column group')
    const literal = table.textContent,
      measureBefore = measured
    translate.update({
      locale: 'zh-CN',
      messages: {
        'Resize column {column}': '调整第 {column} 列',
        'Table horizontal scroll': '表格横向滚动',
        'Drag to resize column · Double-click to fit sampled contents': '拖动调整列宽 · 双击适配'
      }
    })
    check(
      content.querySelector('.table-column-resize').getAttribute('aria-label') === '调整第 1 列' &&
        shell.querySelector('.table-scrollbar').getAttribute('aria-label') === '表格横向滚动',
      'Table layout accessibility did not update its locale'
    )
    check(
      measured === measureBefore && table.textContent === literal,
      'Locale update remeasured or changed table data'
    )
    const before = [...head.cells].map((cell) => cell.getBoundingClientRect().width)
    const handle = content.querySelector('.table-column-resize')
    handle.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    const after = [...head.cells].map((cell) => cell.getBoundingClientRect().width)
    check(Math.abs(after[0] - before[0] - 16) < 2, 'Keyboard resize failed')
    check(
      after.slice(1).every((v, i) => Math.abs(v - before[i + 1]) < 1),
      'Resize changed other columns'
    )
    handle.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, clientY: 100 }))
    check(head.cells[0].getBoundingClientRect().width < 1000, 'Fit scanned beyond the sampled rows')
    layout.update()
    check(
      table.querySelectorAll('colgroup').length === 1,
      'Update duplicated generated column groups'
    )
    const saved = new Map(manual)
    layout.dispose()
    layout.dispose()
    check(
      !content.querySelector('.table-column-resize') && !table.querySelector('colgroup'),
      'Dispose left generated table controls'
    )
    check(!shell.querySelector('.table-scrollbar'), 'Dispose left the floating scrollbar')
    layout = mount(false)
    layout.update()
    check(
      [...saved].every(([key, value]) => manual.get(key) === value),
      'Remount lost manual widths'
    )
    const resized = head.cells[0].getBoundingClientRect().width
    content
      .querySelector('.table-column-resize')
      .dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    check(
      Math.abs(head.cells[0].getBoundingClientRect().width - resized - 16) < 2,
      'Remount duplicated input listeners'
    )
    layout.dispose()
    check(table.textContent === original, 'Measurement or resizing changed source text')
    manual.clear()
    for (const row of table.rows) for (const cell of row.cells) cell.textContent = 'short'
    layout = mount(true)
    layout.update()
    check(
      Math.abs(table.getBoundingClientRect().width - content.clientWidth) < 2,
      'Fill-width did not fill a short ordinary DOM table'
    )
    layout.dispose()
    table.replaceChildren()
    state.columns = Array.from({ length: 128 }, (_, i) => i)
    const wideHead = table.createTHead().insertRow(),
      wideBody = table.createTBody()
    for (const column of state.columns)
      wideHead.appendChild(document.createElement('th')).textContent = String(column)
    for (let row = 0; row < 32; row++) {
      const tr = wideBody.insertRow()
      for (let column = 0; column < 128; column++) tr.insertCell().textContent = '\n'.repeat(1024)
    }
    const calls = measured
    manual.clear()
    layout = mount(false)
    layout.update()
    check(measured - calls <= 8192, 'Wide multiline cells exceeded the total canvas-work budget')
    layout.dispose()
    table.replaceChildren()
    state.columns = Array.from({ length: 512 }, (_, i) => i)
    const fullHead = table.createTHead().insertRow()
    for (const column of state.columns)
      fullHead.appendChild(document.createElement('th')).textContent = String(column)
    layout = mount(false)
    layout.update()
    check(
      content.querySelectorAll('.table-column-resize').length === 512,
      'Permitted 512-column layout exceeded translation bindings'
    )
    layout.dispose()
    check(translate.state().subscribers === 0, 'Layout disposal retained translation subscriptions')
    const fill = Array.from({ length: 255 }, () => translate.subscribe(() => {}))
    let rejected = false
    try {
      mount(false)
    } catch {
      rejected = true
    }
    check(
      rejected && translate.state().subscribers === 255 && !shell.querySelector('.table-scrollbar'),
      'Failed layout setup leaked scope or scrollbar'
    )
    for (const remove of fill) remove()
    return { assertions: checks, rows: 260, sampleRows: 200, longestMeasuredCell: longest }
  } finally {
    layout?.dispose()
    CanvasRenderingContext2D.prototype.measureText = measure
    shell.remove()
  }
}
