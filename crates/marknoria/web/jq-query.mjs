// Rust/provider owns the complete input and immutable jq results.
import { jqHelp, queryError } from './query-help.mjs'
import { queryOperation } from './query-operation.mjs'
import { dialogClose } from './icons.mjs'
import { copyText } from './clipboard.mjs'
import { componentStyles } from './component-styles.mjs'
import { uiLabels } from './translation.mjs'
export const JQ_WINDOW_PROFILE = Object.freeze({
  maxTextBytes: 48 * 1024,
  maxFilterUnits: 16384,
  maxFilterBytes: 16384,
  maxPages: Math.ceil((256 * 1024 * 1024) / (48 * 1024 - 3))
})
const validId = (id) =>
  typeof id === 'string' && /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(id)
function filterText(value) {
  if (
    typeof value !== 'string' ||
    !value.trim() ||
    value.length > JQ_WINDOW_PROFILE.maxFilterUnits ||
    new TextEncoder().encode(value).length > JQ_WINDOW_PROFILE.maxFilterBytes
  )
    throw TypeError('Enter a jq filter of at most 16 KiB')
  return value
}
function validate(value, revision) {
  if (
    !value ||
    value.kind !== 'jqQuery' ||
    !validId(value.id) ||
    value.revision !== revision ||
    !Number.isSafeInteger(value.pages) ||
    value.pages < 1 ||
    value.pages > JQ_WINDOW_PROFILE.maxPages ||
    !Number.isSafeInteger(value.page) ||
    value.page < 0 ||
    value.page >= value.pages ||
    !Number.isSafeInteger(value.totalValues) ||
    value.totalValues < 0 ||
    !Number.isFinite(value.elapsedMs) ||
    value.elapsedMs < 0 ||
    value.elapsedMs > Number.MAX_SAFE_INTEGER ||
    typeof value.text !== 'string' ||
    value.text.length > JQ_WINDOW_PROFILE.maxTextBytes ||
    new TextEncoder().encode(value.text).length > JQ_WINDOW_PROFILE.maxTextBytes
  )
    throw Error('Invalid bounded jq result window or revision')
  return value
}
export function mountJqQuery(
  container,
  {
    provider,
    revision,
    translate,
    autoRun = false,
    presentationRoot = container,
    copy = copyText,
    onState = () => {}
  } = {}
) {
  if (
    !(container instanceof HTMLElement) ||
    typeof revision !== 'string' ||
    !revision.length ||
    revision.length > 256 ||
    !provider ||
    ['run', 'page', 'close'].some((name) => typeof provider[name] !== 'function') ||
    typeof autoRun !== 'boolean'
  )
    throw TypeError('jq requires an owning revision and complete-input provider')
  const lifetime = new AbortController(),
    signal = lifetime.signal,
    labels = uiLabels(translate, { signal })
  const releases = [
    componentStyles(container.ownerDocument, new URL('./table-query.css', import.meta.url)),
    componentStyles(container.ownerDocument, new URL('./jq-query.css', import.meta.url))
  ]
  const previousFocus = document.activeElement,
    dialog = document.createElement('dialog'),
    shell = document.createElement('section')
  dialog.className = 'table-query-dialog jq-query-dialog'
  shell.className = 'table-query-shell jq-query-shell'
  labels.set(dialog, 'Query JSON with jq', {}, 'aria-label')
  const style = getComputedStyle(presentationRoot)
  for (const name of ['panel', 'soft', 'text', 'muted', 'faint', 'border', 'brand', 'brand-soft']) {
    const property = '--reader-' + name,
      value = style.getPropertyValue(property)
    if (value) shell.style.setProperty(property, value)
  }
  for (const property of [
    '--ui-font-family',
    '--ui-font-size',
    '--code-font-family',
    '--code-font-size'
  ]) {
    const value = style.getPropertyValue(property)
    if (value) shell.style.setProperty(property, value)
  }
  const heading = document.createElement('header'),
    title = document.createElement('h2')
  heading.className = 'table-query-heading'
  labels.set(title, 'Query JSON with jq')
  const close = dialogClose(() => {
    void dispose()
  })
  labels.set(close, 'Close', {}, 'aria-label')
  labels.set(close, 'Close', {}, 'title')
  heading.append(title, close)
  const editor = document.createElement('textarea')
  editor.className = 'table-query-editor'
  editor.value = '.'
  editor.maxLength = JQ_WINDOW_PROFILE.maxFilterUnits
  editor.spellcheck = false
  labels.set(editor, 'jq filter', {}, 'aria-label')
  const help = document.createElement('details'),
    helpSummary = document.createElement('summary'),
    helpBody = document.createElement('div')
  help.className = 'table-query-schema jq-help'
  helpBody.className = 'query-help-content'
  labels.set(helpSummary, 'jq syntax and query help')
  jqHelp(helpBody, labels, (filter) => {
    editor.value = filter
    editor.focus()
  })
  help.append(helpSummary, helpBody)
  const actions = document.createElement('div')
  actions.className = 'table-query-actions'
  const run = document.createElement('button'),
    reset = document.createElement('button'),
    slurpLabel = document.createElement('label'),
    slurp = document.createElement('input'),
    slurpText = document.createElement('span'),
    status = document.createElement('span')
  run.className = 'jq-run'
  labels.set(run, 'Run filter')
  labels.set(reset, 'Reset filter')
  slurp.type = 'checkbox'
  labels.set(slurpText, 'Slurp input into an array')
  slurpLabel.append(slurp, slurpText)
  status.role = 'status'
  actions.append(run, reset, slurpLabel, status)
  const result = document.createElement('pre')
  result.className = 'jq-results'
  result.tabIndex = 0
  labels.set(result, 'jq results', {}, 'aria-label')
  labels.set(result, 'Run a filter to view formatted results.')
  const footer = document.createElement('footer')
  footer.className = 'table-query-actions'
  const previous = document.createElement('button'),
    next = document.createElement('button'),
    position = document.createElement('span'),
    copyButton = document.createElement('button')
  labels.set(previous, 'Previous')
  labels.set(next, 'Next')
  labels.set(position, 'No results')
  labels.set(copyButton, 'Copy result page')
  for (const button of [run, reset, previous, next, copyButton]) button.type = 'button'
  footer.append(previous, position, next, copyButton)
  shell.append(heading, editor, help, actions, result, footer)
  dialog.append(shell)
  let current = null,
    stopped = false,
    busy = false,
    request = null,
    generation = 0,
    closing
  const release = (id) =>
    Promise.resolve()
      .then(() => provider.close({ id }))
      .catch(() => {})
  const state = () => ({
    stopped,
    busy,
    id: current?.id ?? null,
    page: current?.page ?? null,
    pages: current?.pages ?? null,
    totalValues: current?.totalValues ?? null,
    message: status.textContent
  })
  const update = () => {
    run.disabled = reset.disabled = slurp.disabled = busy
    previous.disabled = busy || !current || current.page === 0
    next.disabled = busy || !current || current.page + 1 >= current.pages
    copyButton.disabled = busy || !current
    editor.setAttribute('aria-busy', String(busy))
    if (!stopped) onState(state())
  }
  async function load(page, filter) {
    if (stopped) throw new DOMException('jq disposed', 'AbortError')
    if (busy) throw Error('jq is busy')
    if (filter !== undefined) filterText(filter)
    const ticket = ++generation,
      old = current,
      newResult = filter !== undefined
    busy = true
    labels.set(status, 'Evaluating…')
    request = new AbortController()
    update()
    let candidate
    try {
      candidate = newResult
        ? await provider.run({ filter, slurp: slurp.checked, signal: request.signal })
        : await provider.page({ id: old.id, page, signal: request.signal })
      if (stopped || ticket !== generation) {
        if (newResult && validId(candidate?.id) && candidate.id !== old?.id)
          await release(candidate.id)
        return
      }
      const value = validate(candidate, revision)
      if (
        newResult
          ? value.page !== 0
          : value.id !== old.id ||
            value.pages !== old.pages ||
            value.totalValues !== old.totalValues ||
            value.page !== page
      )
        throw Error('jq page changed immutable result identity or position')
      current = value
      labels.clear(result)
      if (value.text) result.textContent = value.text
      else labels.set(result, 'No matching values')
      result.scrollTop = 0
      labels.set(position, 'Page {page} / {pages} · {values} values', {
        page: value.page + 1,
        pages: value.pages,
        values: value.totalValues
      })
      labels.set(status, 'Read-only results · {milliseconds} ms', {
        milliseconds: Math.round(value.elapsedMs)
      })
      if (old && old.id !== value.id) await release(old.id)
    } catch (error) {
      if (newResult && validId(candidate?.id) && candidate.id !== old?.id)
        await release(candidate.id)
      if (!stopped && ticket === generation && error.name !== 'AbortError') {
        labels.clear(status)
        status.textContent = queryError(error, 'jq')
      }
      throw error
    } finally {
      if (!stopped && ticket === generation) {
        busy = false
        update()
      }
    }
  }
  async function dispose() {
    if (stopped) return closing
    stopped = true
    generation++
    request?.abort()
    lifetime.abort()
    dialog.remove()
    for (const releaseStyle of releases) releaseStyle()
    if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true })
    const id = current?.id
    current = null
    closing = id ? release(id) : Promise.resolve()
    return closing
  }
  const report = (error) => {
    if (!stopped && error.name !== 'AbortError') {
      labels.clear(status)
      status.textContent = queryError(error, 'jq')
    }
  }
  const on = (node, type, handler) => node.addEventListener(type, handler, { signal })
  on(run, 'click', () => {
    void load(0, editor.value).catch(report)
  })
  on(reset, 'click', () => {
    editor.value = '.'
    slurp.checked = false
    void load(0, '.').catch(report)
  })
  on(previous, 'click', () => {
    if (current && !previous.disabled) void load(current.page - 1).catch(report)
  })
  on(next, 'click', () => {
    if (current && !next.disabled) void load(current.page + 1).catch(report)
  })
  on(copyButton, 'click', async () => {
    if (!current) return
    const value = current
    try {
      await copy(value.text)
      if (!stopped && current === value) labels.set(status, 'Copied result page')
    } catch (error) {
      if (!stopped && current === value) labels.set(status, 'Clipboard unavailable')
    }
  })
  on(editor, 'keydown', (event) => {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey) && !event.isComposing) {
      event.preventDefault()
      run.click()
    }
  })
  on(dialog, 'cancel', (event) => {
    event.preventDefault()
    void dispose()
  })
  on(dialog, 'close', () => {
    void dispose()
  })
  try {
    container.append(dialog)
    dialog.showModal()
    editor.focus()
    update()
  } catch (error) {
    void dispose()
    throw error
  }
  const ready = autoRun ? load(0, '.') : Promise.resolve()
  void ready.catch(() => {})
  if (!autoRun) labels.set(status, 'Enter a filter to narrow the results, then Run filter.')
  return {
    ready,
    state,
    dispose,
    isOpen: () => !stopped,
    execute: async (filter, { slurp: asArray = slurp.checked } = {}) => {
      if (typeof asArray !== 'boolean') throw TypeError('Invalid jq slurp option')
      if (stopped || busy) throw Error(stopped ? 'jq disposed' : 'jq is busy')
      filterText(filter)
      editor.value = filter
      slurp.checked = asArray
      await load(0, filter)
    },
    go: async (page) => {
      if (!current || !Number.isSafeInteger(page) || page < 0 || page >= current.pages)
        throw TypeError('jq page out of range')
      await load(page)
    }
  }
}
export function readerJqProvider(client, owner) {
  const identity = { documentId: owner.documentId, revision: owner.revision }
  return {
    run: ({ filter, slurp, signal }) =>
      client.request('jq', { ...identity, filter, slurp }, { signal }),
    page: ({ id, page, signal }) => client.request('jqPage', { ...identity, id, page }, { signal }),
    close: ({ id }) => client.request('jqClose', { ...identity, id })
  }
}
export function jqController(index, api, options = {}) {
  if (!/\.(json|jsonl|ndjson)$/i.test(index.name)) return { dispose() {} }
  const lifetime = new AbortController(),
    labels = uiLabels(options.translate, { signal: lifetime.signal }),
    button = document.createElement('button')
  button.type = 'button'
  button.className = 'reader-action'
  labels.set(button, 'Query JSON with jq', {}, 'title')
  button.setAttribute('aria-label', 'jq')
  const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  icon.setAttribute('viewBox', '0 0 24 24')
  icon.setAttribute('aria-hidden', 'true')
  const path = document.createElementNS(icon.namespaceURI, 'path')
  path.setAttribute('d', 'M8 4H6v5l-3 3 3 3v5h2 M16 4h2v5l3 3-3 3v5h-2')
  icon.append(path)
  const caption = document.createElement('span')
  caption.textContent = 'jq'
  button.append(icon, caption)
  document.querySelector('.reader-document-tools').append(button)
  const provider = {
    run: ({ filter, slurp, signal }) =>
      queryOperation(api, index.revision, signal, (operationId) =>
        api('/api/jq-query', signal, {
          method: 'POST',
          body: JSON.stringify({ revision: index.revision, filter, slurp, operationId })
        })
      ),
    page: ({ id, page, signal }) =>
      api('/api/jq-query', signal, {
        method: 'POST',
        body: JSON.stringify({ revision: index.revision, id, page })
      }),
    close: ({ id }) =>
      api('/api/jq-query', undefined, {
        method: 'POST',
        body: JSON.stringify({ revision: index.revision, id, close: true })
      })
  }
  let dialog
  button.addEventListener(
    'click',
    () => {
      void dialog?.dispose()
      dialog = mountJqQuery(document.body, {
        ...options,
        provider,
        revision: index.revision,
        autoRun: index.sourceBytes <= 1024 * 1024,
        presentationRoot: document.querySelector('.reader-shell')
      })
    },
    { signal: lifetime.signal }
  )
  return {
    dispose() {
      lifetime.abort()
      void dialog?.dispose()
      button.remove()
    }
  }
}
