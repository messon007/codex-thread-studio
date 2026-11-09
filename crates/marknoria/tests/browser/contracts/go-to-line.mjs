export async function run() {
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const wait = async (predicate, name) => {
    const deadline = performance.now() + 30000
    while (!predicate()) {
      if (performance.now() > deadline) throw Error(`Timeout ${name}: ${document.querySelector('#status')?.textContent}; ${document.querySelector('.reader-go-to-line output')?.textContent}; words=${JSON.stringify(window.readerExperiment?.wordHighlights?.state())}`)
      await new Promise(resolve => setTimeout(resolve, 25))
    }
  }
  await wait(() => window.readerExperiment?.metrics.firstContentMs != null, 'reader')
  const e = () => window.readerExperiment
  const changeFont = async (id, value) => {
    const control = document.getElementById(id)
    check(!control.disabled, `${id} unexpectedly disabled`)
    control.value = value; control.dispatchEvent(new Event('change'))
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    await wait(() => e().pendingNavigation == null, 'font relayout')
  }
  const dialog = document.querySelector('.reader-go-to-line')
  const input = dialog.querySelector('input'), form = dialog.querySelector('form')
  function open() {
    document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'g', ctrlKey: true, bubbles: true, cancelable: true }))
    check(dialog.open, 'Ctrl+G did not open the shared absolute-line dialog')
  }
  async function go(line) {
    open(); input.value = String(line); form.requestSubmit()
    await wait(() => !dialog.open, `go ${line}`)
    await wait(() => e().pendingNavigation == null && !e().editor?.busy && !e().source?.busy && !document.getElementById('preview-mode').disabled, 'navigation settled')
  }
  const api = async path => {
    const response = await fetch(path, { headers: { 'x-reader-token': location.hash.slice(1), 'x-reader-document': e().documentId } })
    if (!response.ok) throw Error(await response.text())
    return response.json()
  }
  open(); input.value = '0'; form.requestSubmit()
  check(dialog.open && input.getAttribute('aria-invalid') === 'true', 'Invalid line was not rejected')
  input.value = String(e().index.sourceLines + 1); form.requestSubmit()
  check(dialog.open && /1/.test(dialog.querySelector('output').textContent), 'Out-of-range line was not rejected')
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  dialog.querySelector('header button').click()
  if (e().index.name.endsWith('.json')) {
    check(e().index.sourceLines <= 2, 'Fixture must be compact original JSON')
    await go(1)
    check(e().position().view === 'preview', 'JSON go-to-line switched to Source')
    check(document.querySelector('#viewport').scrollTop < 10, 'Original JSON line 1 did not resolve to beginning of formatted content')
    return { assertions, format: 'json' }
  }
  await go(125)
  check(e().position().view === 'preview', 'Preview go-to-line switched view')
  const beforeSize = e().position().line
  await changeFont('reading-text-size', '22')
  check(getComputedStyle(document.querySelector('#space [data-marknoria-prose]')).fontSize === '22px', 'Text size not applied')
  check(Math.abs(e().position().line - beforeSize) <= 2, 'Text size lost physical reading anchor')
  await changeFont('reading-code-size', '24')
  check(getComputedStyle(document.querySelector('#space pre')).fontSize === '24px', 'Preview code size not applied')
  check(Math.abs(e().position().line - beforeSize) <= 2, 'Code size lost physical reading anchor')
  await changeFont('reading-text-size', '15')
  await changeFont('reading-code-size', '')
  check(!document.querySelector('.reader-shell').style.getPropertyValue('--code-font-size'), 'Empty code size did not restore Auto')
  const invalidSize = document.getElementById('reading-text-size')
  invalidSize.value = '99'; invalidSize.dispatchEvent(new Event('change'))
  check(invalidSize.value === '15', 'Out-of-range text size was accepted')
  await go(125)
  const code = document.querySelector('#space [data-source-code-line]')
  check(Boolean(code), 'Target code block not mounted')
  const viewportTop = document.querySelector('#viewport').getBoundingClientRect().top
  // The source line is within a long fenced block; navigation must not stop at its heading.
  check(code.getBoundingClientRect().top < viewportTop - 500, 'Preview jumped to start of code block, not requested source line')
  const highlightToggle = document.getElementById('highlight-selection-matches')
  highlightToggle.checked = true; highlightToggle.dispatchEvent(new Event('change'))
  // Native selection on an actually visible syntax token exercises Reader glue,
  // independently of the engine's isolated bounds/cancellation contract.
  const keyword = [...code.querySelectorAll('span')].find(node => node.textContent === 'let' && node.getBoundingClientRect().top >= viewportTop)
  check(Boolean(keyword?.firstChild), 'Visible syntax token missing')
  const selected = document.createRange(); selected.selectNodeContents(keyword)
  getSelection().removeAllRanges(); getSelection().addRange(selected)
  document.dispatchEvent(new Event('selectionchange'))
  if (CSS.highlights) await wait(() => [...CSS.highlights].some(([name, marks]) => name.startsWith('marknoria-selection-word-') && marks.size > 1), 'Preview matching words')
  highlightToggle.checked = false; highlightToggle.dispatchEvent(new Event('change'))
  if (CSS.highlights) check(![...CSS.highlights].some(([name]) => name.startsWith('marknoria-selection-word-')), 'Toggle off retained Preview marks')
  getSelection().removeAllRanges()
  document.getElementById('source-mode').click()
  await wait(() => e().source?.visible && e().source.code && !e().source.busy && !document.getElementById('preview-mode').disabled, 'Source')
  await go(135)
  check(e().source.code.cursorLine() === 135, 'Source used a window-relative line number')
  check(!document.querySelector('.reader-settings').hidden, 'Source cannot access matching-word settings')
  highlightToggle.checked = true; highlightToggle.dispatchEvent(new Event('change'))
  const sourceCode = e().source.code
  const firstLet = sourceCode.text.indexOf('let ')
  await sourceCode.select(firstLet, firstLet + 3)
  await wait(() => sourceCode.wordHighlightState().matches > 1, 'Source matching words')
  highlightToggle.checked = false; highlightToggle.dispatchEvent(new Event('change'))
  check(sourceCode.wordHighlightState().marks === 0, 'Toggle off retained Source marks')
  const sourceSelection = sourceCode.view.state.selection.main
  await changeFont('reading-code-size', '32')
  check(sourceCode.view.state.selection.main.from === sourceSelection.from && sourceCode.view.state.selection.main.to === sourceSelection.to, 'Source font resize changed selection')
  check(parseFloat(getComputedStyle(sourceCode.view.scrollDOM).lineHeight) >= 32, 'Large Source lines overlap')
  await changeFont('reading-code-size', '')
  const hits = await api('/api/search?query=table-needle-287')
  const rowLine = hits.matches[0].line
  await go(rowLine)
  document.getElementById('preview-mode').click()
  await wait(() => !e().source.visible && e().pendingNavigation == null && !document.getElementById('preview-mode').disabled, 'Preview')
  await go(rowLine)
  const row = document.querySelector(`#space tbody tr[data-source-line="${rowLine}"]`)
  check(Boolean(row?.textContent.includes('table-needle-287')), 'Preview did not open target table row page')
  const rowBox = row.getBoundingClientRect(), viewBox = document.querySelector('#viewport').getBoundingClientRect()
  check(rowBox.bottom > viewBox.top && rowBox.top < viewBox.bottom, 'Requested table row is offscreen')
  await changeFont('reading-text-size', '22')
  check(getComputedStyle(document.querySelector(`#space tbody tr[data-source-line="${rowLine}"] td`)).fontSize === '22px', 'Table text ignored reading font size')
  await changeFont('reading-text-size', '15')
  document.getElementById('edit-mode').click()
  await wait(() => e().editor?.visible && e().editor.code && !e().editor.busy && !document.getElementById('preview-mode').disabled, 'Edit')
  await go(20)
  const editor = e().editor
  editor.code.view.dispatch({ changes: { from: 0, insert: '// unsaved draft\n' } })
  const draft = editor.code.text, before = editor.revision
  await go(25)
  check(editor.dirty && editor.code.text === draft && editor.revision === before, 'In-window jump wrote or lost draft')
  check(editor.code.canUndo, 'In-window jump discarded undo history')
  const selection = editor.code.view.state.selection.main
  check(!document.querySelector('.reader-settings').hidden, 'Edit has no font settings')
  await changeFont('reading-code-size', '32')
  check(editor.dirty && editor.code.text === draft && editor.revision === before && editor.code.canUndo, 'Font change altered draft, revision or undo')
  check(editor.code.view.state.selection.main.from === selection.from && editor.code.view.state.selection.main.to === selection.to, 'Font change moved Edit selection')
  check(getComputedStyle(editor.code.view.scrollDOM).fontSize === '32px' && parseFloat(getComputedStyle(editor.code.view.scrollDOM).lineHeight) >= 32, 'Edit font/line height did not update')
  await changeFont('reading-code-size', '')
  const fetchOriginal = window.fetch
  window.fetch = (resource, options) => options?.method === 'POST' && new URL(resource, location.href).pathname.endsWith('/api/edit')
    ? Promise.resolve(new Response('Injected revision conflict', { status: 409 })) : fetchOriginal(resource, options)
  try {
    open(); input.value = String(rowLine); form.requestSubmit()
    await wait(() => dialog.querySelector('output').textContent.includes('Injected revision conflict'), 'save conflict')
    check(dialog.open && editor.dirty && editor.code.text === draft, 'Failed cross-page save lost draft or dismissed error')
    dialog.querySelector('header button').click()
  } finally { window.fetch = fetchOriginal }
  check(editor.code.undo(), 'Undo after failed navigation unavailable')
  check(!editor.dirty, 'Undo did not restore original page')
  await go(rowLine)
  check(editor.code.cursorLine() === rowLine, 'Cross-page Edit jump failed')
  return { assertions, format: 'markdown' }
}
