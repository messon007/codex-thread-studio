// Shared semantic checks. Input comes from Playwright or the private Wayland seat.
window.runTableChecks = async () => {
  const pointerEvents = []
  for (const type of ['pointerdown', 'pointermove', 'pointerup']) document.addEventListener(type, event => {
    pointerEvents.push({ type, x: event.clientX, y: event.clientY, target: event.target.className, trusted: event.isTrusted })
    if (pointerEvents.length > 12) pointerEvents.shift()
  }, { capture: true })
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
  const wait = async predicate => {
    const caller = new Error('Table wait').stack
    const deadline = performance.now() + 15000
    while (!predicate()) { if (performance.now() > deadline) throw Error(`Table interaction did not settle: ${caller}`); await pause(20) }
  }
  const frames = async () => { for (let i = 0; i < 3; i++) await new Promise(requestAnimationFrame) }
  let assertions = 0
  const check = (ok, message) => { assertions++; if (!ok) throw Error(message) }
  const input = async value => { await window.marknoriaTableInput(value); await frames() }
  const viewport = document.querySelector('#viewport')
  const reader = window.readerExperiment
  const first = reader.index.tables[0]
  const article = () => document.querySelector('article[data-table="0"]')
  const ready = () => wait(() => article()?.dataset.ready === 'true')
  const widths = () => [...article().querySelectorAll('thead th')].map(cell => cell.getBoundingClientRect().width)
  const same = (a, b) => a.length === b.length && a.every((width, i) => Math.abs(width - b[i]) < 1.5)
  const body = () => article().querySelector('.table-content')
  const handle = column => article().querySelector(`.table-column-resize[data-column="${column}"]`)
  const drag = async (column, amount) => {
    const bounds = handle(column).getBoundingClientRect()
    const y = Math.min(viewport.getBoundingClientRect().bottom - 50, Math.max(bounds.top + 20, viewport.getBoundingClientRect().top + 140))
    const x = bounds.left + bounds.width / 2
    await input({ type: 'button', x, y, amount: 1 })
    await input({ type: 'move', x: x + amount / 2, y })
    await input({ type: 'move', x: x + amount, y })
    await input({ type: 'button', x: x + amount, y, amount: 0 })
  }
  await reader.jump(first.firstPage, null, 0); await ready(); await frames()
  const automatic = widths()
  check(automatic[0] <= 100 && automatic[1] <= 100 && automatic[3] <= 100, `Short columns too wide: ${automatic}`)
  check(body().scrollHeight - body().clientHeight <= 1, 'Table has an inner vertical scroll range')
  check(parseFloat(getComputedStyle(article().querySelector('td')).borderRightWidth) >= 1, 'Column separator missing')
  viewport.scrollTop += 260; await frames()
  const tableIdentity = article().querySelector('table')
  const anchor = [...body().querySelectorAll('tbody tr')].find(row => row.getBoundingClientRect().top > viewport.getBoundingClientRect().top + 30)
  const anchorTop = anchor.getBoundingClientRect().top
  await drag(2, -220)
  let sized = widths()
  check(Math.abs(sized[2] - automatic[2] + 220) < 2, `Dragged column width incorrect: ${automatic} -> ${sized}; input ${JSON.stringify(pointerEvents)}`)
  check([0, 1, 3].every(i => Math.abs(sized[i] - automatic[i]) < 1.5), 'Dragging redistributed neighboring widths')
  check(article().querySelector('table') === tableIdentity, 'Dragging rebuilt the table')
  check(Math.abs(anchor.getBoundingClientRect().top - anchorTop) < 2, 'Dragging moved the visible row')
  const edge = handle(2).getBoundingClientRect().left + 4
  check([...body().querySelectorAll('tr')].every(row => Math.abs(row.cells[2].getBoundingClientRect().right - edge) < 1.5), 'Header/body column edges differ')
  await drag(0, -100)
  const minimum = widths()
  check(Math.abs(minimum[0] - 48) < 1.5, 'Minimum column width was not enforced')
  check([1, 2, 3].every(i => Math.abs(minimum[i] - sized[i]) < 1.5), 'Minimum-width drag changed neighbors')
  sized = minimum
  viewport.scrollTop = article().offsetTop; await frames()
  article().querySelector('[aria-label="Next page"]').click(); await ready()
  check(same(widths(), sized), 'Row paging lost widths')
  article().querySelector('[aria-label="Previous page"]').click(); await ready()
  check(same(widths(), sized), 'Previous page lost widths')
  let picker = article().querySelector('.column-picker')
  const setUnit = async active => {
    picker.open = true
    const box = picker.querySelector('input[value="3"]'); box.checked = active; box.dispatchEvent(new Event('change'))
    picker.querySelector('.column-panel-footer button').click(); await ready()
  }
  await setUnit(false); await setUnit(true)
  check(same(widths(), sized), 'Hiding/showing original column IDs lost widths')
  // Keep the old cells interactive during a slow column-selection request.
  const originalFetch = window.fetch
  window.fetch = async (...args) => {
    if (String(args[0]).includes('/api/table?')) await pause(1500)
    return originalFetch(...args)
  }
  try {
    const firstBox = picker.querySelector('input[value="0"]')
    picker.open = true; firstBox.checked = false; firstBox.dispatchEvent(new Event('change'))
    picker.querySelector('.column-panel-footer button').click(); await frames()
    await drag(0, 32); await ready()
    check(same(widths(), sized.slice(1)), 'Pending column selection assigned an old cell width to the wrong original column')
    picker.open = true; firstBox.checked = true; firstBox.dispatchEvent(new Event('change'))
    picker.querySelector('.column-panel-footer button').click(); await ready()
    sized[0] += 32
    check(same(widths(), sized), 'Resizing the outgoing projection lost its original column width')
  } finally { window.fetch = originalFetch }
  document.querySelector('#source-mode').click()
  await wait(() => reader.source.visible && document.querySelector('#source-view').dataset.ready); await frames()
  document.querySelector('#preview-mode').click()
  await wait(() => !reader.source.visible && reader.pendingNavigation == null); await ready(); await frames()
  check(same(widths(), sized), 'Source/Preview lost widths')
  if (!document.querySelector('#edit-mode').hidden && !document.querySelector('#edit-mode').disabled) {
    document.querySelector('#edit-mode').click()
    await wait(() => reader.editor?.visible && !reader.editor.busy && document.querySelector('#edit-mode').getAttribute('aria-pressed') === 'true')
    document.querySelector('#preview-mode').click()
    await wait(() => !reader.editor.visible && reader.pendingNavigation == null); await ready(); await frames()
    check(same(widths(), sized), 'Edit/Preview lost widths')
  }
  for (let toggle = 0; toggle < 2; toggle++) {
    document.querySelector('#wrap').click()
    await wait(() => reader.pendingNavigation == null); await ready(); await frames()
    check(same(widths(), sized), 'Changing wrapping lost manual widths')
  }
  await reader.jump(reader.index.pages.length - 1)
  await reader.jump(first.firstPage, null, 0); await ready()
  check(same(widths(), sized), 'Virtual unmount/remount lost widths')
  const bounds = handle(2).getBoundingClientRect()
  await input({ type: 'double', x: bounds.left + 4, y: bounds.top + 20 })
  const fitted = widths()
  check(fitted[2] > 350 && fitted[2] < 1500, `Content fit width incorrect: ${fitted}`)
  check([0, 1, 3].every(i => Math.abs(fitted[i] - sized[i]) < 1.5), 'Content fit changed neighbors')
  picker = article().querySelector('.column-picker')
  picker.open = true; picker.querySelector('.column-width-reset').click(); await frames()
  check(same(widths(), automatic), `Reset did not restore automatic widths: ${automatic} -> ${widths()}`)
  // Force overflow; scroll controls must remain accessible before the table end.
  await drag(0, 400)
  sized = widths()
  let track = document.querySelector('.table-scrollbar[aria-controls="table-content-0"]')
  check(!track.hidden, 'Horizontal control missing')
  viewport.scrollTop += 250; await frames()
  const trackBounds = track.getBoundingClientRect(), viewBounds = viewport.getBoundingClientRect()
  check(body().getBoundingClientRect().bottom > viewBounds.bottom + 100, 'Fixture does not exercise an offscreen table end')
  check(Math.abs(trackBounds.bottom - viewBounds.bottom) < 2, `Floating scrollbar drifted: ${trackBounds.bottom} vs ${viewBounds.bottom}`)
  const thumb = track.firstElementChild.getBoundingClientRect()
  const rootTop = viewport.scrollTop
  await input({ type: 'button', x: thumb.left + 10, y: thumb.top + 4, amount: 1 })
  await input({ type: 'move', x: thumb.left + 160, y: thumb.top + 4 })
  await input({ type: 'button', x: thumb.left + 160, y: thumb.top + 4, amount: 0 })
  check(body().scrollLeft > 10, 'Floating thumb did not scroll table')
  check(Math.abs(viewport.scrollTop - rootTop) < 2, 'Horizontal drag scrolled the document')
  document.querySelector('#source-mode').click()
  await wait(() => reader.source.visible && document.querySelector('#source-view').dataset.ready); await frames()
  check(track.hidden, 'Floating scrollbar leaked into Source mode')
  document.querySelector('#preview-mode').click()
  await wait(() => !reader.source.visible && reader.pendingNavigation == null); await ready(); await frames()
  check(same(widths(), sized), 'Source mode lost overflow widths')
  await reader.jump(first.firstPage, null, 0); await ready()
  viewport.scrollTop += 250; await frames()
  track = document.querySelector('.table-scrollbar[aria-controls="table-content-0"]')
  body().scrollLeft = 0; await frames()
  const point = () => ({ x: body().getBoundingClientRect().left + 60, y: viewport.getBoundingClientRect().top + 100 })
  await input({ type: 'axis', ...point(), axis: 0, amount: 12, shifted: true })
  check(body().scrollLeft > 0, 'Shift+wheel did not move horizontally')
  body().scrollLeft = (body().scrollWidth - body().clientWidth) / 2; await frames()
  const left = body().scrollLeft
  await input({ type: 'axis', ...point(), axis: 1, amount: 80, smooth: true })
  check(Math.abs(body().scrollLeft - left) > 5, 'Horizontal touchpad input did not move table')
  const pageStart = article().dataset.rowStart
  let top = viewport.scrollTop
  await input({ type: 'axis', ...point(), axis: 0, amount: 12 })
  check(viewport.scrollTop > top && body().scrollTop === 0, 'Vertical input was trapped in the table')
  top = viewport.scrollTop
  await input({ type: 'axis', ...point(), axis: 0, amount: -12 })
  check(viewport.scrollTop < top && article().dataset.rowStart === pageStart, 'Upward input did not reach document or changed the table page')
  check(same(widths(), sized), 'Scrolling changed column widths')
  // At the final table row, further input must reach following document content.
  viewport.scrollTop += body().getBoundingClientRect().bottom - viewport.getBoundingClientRect().bottom + 10; await frames()
  check(Math.abs(track.getBoundingClientRect().bottom - body().getBoundingClientRect().bottom + 1) < 2, `Scrollbar did not return to the visible table bottom: ${JSON.stringify({ track: track.getBoundingClientRect(), table: body().getBoundingClientRect(), viewport: viewport.getBoundingClientRect(), hidden: track.hidden })}`)
  top = viewport.scrollTop
  const bottomPoint = { x: body().getBoundingClientRect().left + 60, y: Math.min(body().getBoundingClientRect().bottom - 20, viewport.getBoundingClientRect().bottom - 35) }
  await input({ type: 'axis', ...bottomPoint, axis: 0, amount: 1 })
  check(viewport.scrollTop > top && body().scrollTop === 0, 'Vertical input was trapped at table bottom')
  check(article().dataset.rowStart === pageStart, 'Wheel input paged the table')
  await reader.jump(first.firstPage, null, 42); await ready()
  const row = body().querySelectorAll('tbody tr')[42 - Number(article().dataset.rowStart)]?.getBoundingClientRect()
  const visible = viewport.getBoundingClientRect()
  check(row && row.bottom > visible.top && row.top < visible.bottom, 'Exact source-row navigation lost its target')
  await reader.jump(first.firstPage, null, 0); await ready()
  check(same(widths(), sized), 'Source-row navigation lost column widths')
  top = viewport.scrollTop
  await input({ type: 'axis', x: body().getBoundingClientRect().left + 60, y: body().getBoundingClientRect().top + 60, axis: 0, amount: -8 })
  check(viewport.scrollTop < top && body().scrollTop === 0 && article().dataset.rowStart === '0', 'Vertical input was trapped at table top')
  body().scrollLeft=0
  const cell=body().querySelector('tbody td');cell.scrollIntoView({block:'center',inline:'nearest'});await frames()
  const cellBounds=cell.getBoundingClientRect()
  await input({type:'double',x:cellBounds.left+20,y:cellBounds.top+cellBounds.height/2})
  check(!document.querySelector('.table-cell-dialog') && getSelection()?.toString().trim() && cell.contains(getSelection().anchorNode),'Double-click did not preserve native word selection')
  article().querySelector('.table-cell-view').click()
  await wait(()=>document.querySelector('.table-cell-dialog')?.open)
  check(document.querySelector('.table-cell-full').textContent===cell.textContent,'Full cell changed the source value')
  check(reader.tables.selection()===null,'Opening the full-cell viewer selected the whole cell');const range=document.createRange();range.selectNodeContents(document.querySelector('.table-cell-full'));getSelection().removeAllRanges();getSelection().addRange(range);document.querySelector('.table-cell-full').dispatchEvent(new MouseEvent('pointerup',{bubbles:true}))
  check(reader.tables.selection().location.row===0&&reader.tables.selection().location.column===0,'Physical cell selection lost original coordinates')
  document.querySelector('.table-cell-dialog .dialog-close').click();article().querySelector('.table-cell-copy').click()
  await wait(()=>article().querySelector('.table-cell-copy').textContent==='Copied')
  check(article().querySelector('.table-cell-copy').textContent==='Copied','Cell clipboard write failed')
  check(document.activeElement===cell,'Full cell close did not restore keyboard focus')
  check(same(widths(),sized),'Cell interaction changed column widths')
  // Exercise the actual Notes UI and storage on the same native WebView.
  const button=(root,label)=>[...root.querySelectorAll('button')].find(node=>node.textContent===label)
  cell.click()
  const annotated=document.createRange();annotated.selectNodeContents(cell)
  getSelection().removeAllRanges();getSelection().addRange(annotated)
  document.dispatchEvent(new Event('selectionchange'))
  await wait(()=>!document.querySelector('.notes-toolbar').hidden)
  button(document.querySelector('.notes-toolbar'),'Comment').click()
  await wait(()=>document.querySelector('.notes-dialog textarea'))
  document.querySelector('.notes-dialog textarea').value='Persistent Wayland comment'
  button(document.querySelector('.notes-dialog'),'Save comment').click()
  await wait(()=>document.querySelector('.note-card'))
  document.querySelector('.notes-dialog .dialog-close').click()
  getSelection().removeAllRanges()
  const markerCount=()=>Number(document.querySelector('.reader-shell').dataset.commentMarks||0)
  await wait(()=>markerCount()>0)
  check(!cell.querySelector('[data-comment-ids]'),'Comment marker rewrote cell text')
  check(cell.textContent==='K0','Comment marker changed cell value')
  await reader.jump(first.firstPage,null,75);await ready();await wait(()=>markerCount()===0)
  await reader.jump(first.firstPage,null,0);await ready();await wait(()=>markerCount()>0)
  check(markerCount()>0,'Table pagination lost persistent comment marker')
  document.querySelector('.notes-open').click();await wait(()=>document.querySelector('.note-card'))
  button(document.querySelector('.notes-dialog'),'Delete').click()
  await wait(()=>document.querySelector('.notes-empty'));await wait(()=>markerCount()===0)
  document.querySelector('.notes-dialog .dialog-close').click()
  check(markerCount()===0,'Deleted comment remained underlined')
  return { assertions, physicalInput: true, floatingScrollbar: true, columns: sized, sourceBytes: reader.index.sourceBytes }
}
