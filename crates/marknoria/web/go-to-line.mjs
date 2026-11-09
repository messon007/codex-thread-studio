import { uiLabels } from './translation.mjs'

// Continued/formatted source lines may span many projection pages. Prefer the
// first page beginning on the requested line, not the last such continuation.
export function sourcePageForLine(pages, line) {
  let low = 0, high = pages.length
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    if (pages[middle].line < line) low = middle + 1
    else high = middle
  }
  return low < pages.length && pages[low].line === line ? low : Math.max(0, low - 1)
}

// One absolute-source-line command for Preview and the bounded editor windows.
// Capture identity when opened so an old dialog cannot navigate a new document.
export function mountGoToLine(root, { state, navigate, translate, signal }) {
  const lifetime = new AbortController()
  const labels = uiLabels(translate, { signal: lifetime.signal })
  const dialog = document.createElement('dialog')
  dialog.className = 'reader-go-to-line'
  dialog.innerHTML = '<form novalidate><header><h2></h2><button type="button" class="reader-icon-button">×</button></header><label><span></span><input type="number" min="1" step="1" required inputmode="numeric"></label><p></p><output role="status"></output><footer><button type="button"></button><button type="submit" class="reader-primary"></button></footer></form>'
  root.append(dialog)
  const form = dialog.querySelector('form'), input = dialog.querySelector('input')
  const output = dialog.querySelector('output'), submit = dialog.querySelector('[type=submit]')
  const close = dialog.querySelector('header button'), cancel = dialog.querySelector('footer [type=button]')
  const title = dialog.querySelector('h2'), hint = dialog.querySelector('p')
  title.id = 'reader-go-to-line-title'
  input.id = 'reader-go-to-line-input'
  output.id = 'reader-go-to-line-error'
  dialog.setAttribute('aria-labelledby', title.id)
  input.setAttribute('aria-describedby', output.id)
  labels.set(title, 'Go to source line')
  labels.set(dialog.querySelector('label span'), 'Source line')
  labels.set(close, 'Close', {}, 'aria-label')
  labels.set(cancel, 'Cancel'); labels.set(submit, 'Go')
  let captured = null, busy = false, stopped = false, previousFocus = null, generation = 0
  const on = (node, type, fn, options = {}) => node.addEventListener(type, fn, { ...options, signal: lifetime.signal })
  function reset() { generation++; captured = null; busy = false; input.disabled = submit.disabled = false; dialog.removeAttribute('aria-busy') }
  function dismiss() { reset(); if (dialog.open) dialog.close() }
  on(close, 'click', dismiss); on(cancel, 'click', dismiss)
  on(dialog, 'cancel', () => reset())
  on(dialog, 'close', () => {
    // A queued close event from the previous opening can arrive after reopening.
    if (dialog.open) return
    reset(); if (previousFocus?.isConnected) previousFocus.focus()
  })
  on(input, 'input', () => { labels.clear(output); output.textContent = ''; input.removeAttribute('aria-invalid') })
  on(form, 'submit', async event => {
    event.preventDefault()
    if (busy || !captured) return
    const current = state(), target = Number(input.value)
    if (!current || current.identity !== captured.identity || current.view !== captured.view) { dismiss(); return }
    if (!/^\d+$/u.test(input.value) || !Number.isSafeInteger(target) || target < 1 || target > current.total) {
      labels.set(output, 'Enter a source line from 1 to {total}.', { total: current.total })
      input.setAttribute('aria-invalid', 'true'); input.focus(); return
    }
    const ticket = ++generation
    busy = true; input.disabled = submit.disabled = true; dialog.setAttribute('aria-busy', 'true')
    try {
      await navigate(target, captured)
      if (ticket === generation) { previousFocus = null; dismiss() }
    } catch (error) {
      if (ticket !== generation) return
      labels.clear(output); output.textContent = String(error.message).slice(0, 1200)
    } finally {
      if (ticket === generation) { busy = false; input.disabled = submit.disabled = false; dialog.removeAttribute('aria-busy'); input.focus() }
    }
  })
  function open() {
    const current = state()
    if (stopped || !current || current.busy || !current.total || root.ownerDocument.querySelector('dialog[open]')) return false
    captured = current; previousFocus = root.ownerDocument.activeElement
    input.value = String(Math.max(1, Math.min(current.total, current.line)))
    input.max = String(current.total)
    labels.clear(output); output.textContent = ''; input.removeAttribute('aria-invalid')
    labels.set(hint, 'Original source file · {total} lines', { total: current.total })
    dialog.showModal(); input.focus(); input.select(); return true
  }
  // Capture before CodeMirror's document-relative Go-to-line binding.
  on(root.ownerDocument, 'keydown', event => {
    if (event.defaultPrevented || event.isComposing || !event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || event.key.toLowerCase() !== 'g') return
    if (open()) { event.preventDefault(); event.stopPropagation() }
  }, { capture: true })
  function dispose() { if (stopped) return; stopped = true; reset(); lifetime.abort(); dialog.remove() }
  signal?.addEventListener('abort', dispose, { once: true })
  return { open, close: dismiss, dispose }
}
