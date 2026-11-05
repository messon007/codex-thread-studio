import { dialogClose } from './icons.mjs'
import { componentStyles } from './component-styles.mjs'
import { uiLabels } from './translation.mjs'
import { copyText as copyToClipboard } from './clipboard.mjs'
const noteError = (error) => String(error?.message ?? error ?? 'Notes action failed').slice(0, 800)
// DOM selection, dialogs and clipboard only. Rust owns durable local records.
export function mountNotes({
  owner,
  root,
  toolbar,
  overlayRoot = root.ownerDocument.body,
  storage,
  selection: selectionProvider,
  canSave = () => true,
  queryNotes = false,
  translate,
  copy = copyToClipboard,
  actions = [],
  eventRoot = root,
  presentationRoot = root
}) {
  if (
    !owner ||
    typeof owner.documentId !== 'string' ||
    !owner.documentId.length ||
    owner.documentId.length > 256 ||
    typeof owner.revision !== 'string' ||
    !owner.revision.length ||
    owner.revision.length > 256 ||
    typeof owner.name !== 'string' ||
    owner.name.length > 1024 ||
    !root ||
    !toolbar ||
    typeof storage !== 'function'
  )
    throw new TypeError('Notes require an owning document, roots and scoped storage')
  if (
    !Array.isArray(actions) ||
    actions.length > 8 ||
    actions.some(
      (action) =>
        typeof action.id !== 'string' ||
        action.id.length > 64 ||
        typeof action.label !== 'string' ||
        action.label.length > 80 ||
        typeof action.run !== 'function'
    ) ||
    new Set(actions.map((action) => action.id)).size !== actions.length
  )
    throw new TypeError('Invalid bounded note actions')
  if (typeof selectionProvider !== 'function')
    throw Error('Notes require the public selection provider')
  owner = Object.freeze({
    documentId: owner.documentId,
    revision: owner.revision,
    name: owner.name
  })
  const index = { name: owner.name, revision: owner.revision }
  const decorate = (node) => {
    node.setAttribute('data-marknoria-notes', '')
    const style = getComputedStyle(presentationRoot)
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
      const key = '--reader-' + name,
        value = style.getPropertyValue(key)
      if (value) node.style.setProperty(key, value)
    }
    for (const [name, source] of [
      ['soft', 'soft'],
      ['muted', 'faint'],
      ['brand', 'brand'],
      ['brand-soft', 'brand-soft']
    ]) {
      const value = style.getPropertyValue('--reader-' + source)
      if (value) node.style.setProperty('--notes-' + name, value)
    }
    return node
  }

  const releaseStyles = componentStyles(root.ownerDocument, new URL('./notes.css', import.meta.url))

  const lifetime = new AbortController()
  let labels
  try {
    labels = uiLabels(translate, { signal: lifetime.signal })
  } catch (error) {
    releaseStyles()
    lifetime.abort()
    throw error
  }
  function closeButton(action) {
    const node = dialogClose(action)
    labels.set(node, 'Close', {}, 'aria-label')
    labels.set(node, 'Close', {}, 'title')
    return node
  }
  let stopped = false,
    dialog = null,
    captured = null,
    pendingSave = Promise.resolve()
  let activeRequests = 0
  const noteApi = (request) => {
    if (stopped) return Promise.reject(new DOMException('Notes disposed', 'AbortError'))
    if (activeRequests >= 4) return Promise.reject(Error('Notes request budget exhausted'))
    const prior = pendingSave,
      mutation = ['add', 'update', 'delete', 'clear'].includes(request.operation)
    activeRequests++
    const task = (async () => {
      try {
        if (mutation || request.operation === 'list') await prior
        lifetime.signal.throwIfAborted()
        if (new TextEncoder().encode(JSON.stringify(request)).length > 131072)
          throw Error('Note request exceeds its transport budget')
        const result = await storage(request, { signal: lifetime.signal, owner })
        lifetime.signal.throwIfAborted()
        return result
      } finally {
        activeRequests--
      }
    })()
    if (mutation) pendingSave = task.catch(() => {})
    return task
  }

  const status = document.createElement('div')
  status.className = 'notes-status'
  status.style.boxSizing = 'border-box'
  status.role = 'status'
  decorate(status)
  overlayRoot.append(status)
  const feedback = (text, raw = false) => {
    if (stopped) return
    const modal = document.activeElement?.closest('dialog[open]')
    const area = modal?.getBoundingClientRect(),
      parent = modal || overlayRoot
    if (status.parentElement !== parent) parent.append(status)
    status.style.left = area ? `${area.left + area.width / 2}px` : '50%'
    status.style.bottom = area ? `${Math.max(8, innerHeight - area.bottom + 8)}px` : '16px'
    status.style.maxWidth = `${Math.max(0, (area?.width || innerWidth) - 16)}px`
    if (raw) {
      labels.clear(status)
      status.textContent = text
    } else labels.set(status, text)
    clearTimeout(status.timer)
    status.timer = setTimeout(() => {
      labels.clear(status)
      status.textContent = ''
    }, 2500)
  }
  async function copyText(text) {
    if (
      typeof text !== 'string' ||
      text.length > 131072 ||
      new TextEncoder().encode(text).length > 131072
    )
      throw Error('Copied note exceeds its text budget')
    await copy(text)
    feedback('Copied')
  }
  const controls = document.createElement('div')
  controls.className = 'notes-toolbar'
  decorate(controls)
  controls.style.boxSizing = 'border-box'
  controls.hidden = true
  controls.setAttribute('role', 'toolbar')
  labels.set(controls, 'Selection actions', {}, 'aria-label')
  const button = (label, action) => {
    const b = document.createElement('button')
    b.type = 'button'
    if (label) labels.set(b, label)
    b.addEventListener(
      'click',
      () => {
        try {
          Promise.resolve(action()).catch((e) => feedback(noteError(e), true))
        } catch (error) {
          feedback(noteError(error), true)
        }
      },
      { signal: lifetime.signal }
    )
    return b
  }
  controls.append(
    button('Copy', () => copyText(captured.quote)),
    button('Comment', () => edit()),
    button('Favorite', async () => {
      if (!canSave()) {
        feedback('Save your edits before adding a favorite')
        return
      }
      const task = noteApi({
        operation: 'add',
        kind: 'favorite',
        quote: captured.quote,
        anchor: captured.anchor,
        revision: index.revision
      })
      await task
      controls.hidden = true
      feedback('Saved favorite')
    })
  )
  overlayRoot.append(controls)
  for (const action of actions) {
    const item = button(action.label, () =>
      action.run(structuredClone(captured.reference), { signal: lifetime.signal })
    )
    item.dataset.action = action.id
    controls.append(item)
  }
  controls.addEventListener('pointerdown', (e) => e.preventDefault())
  const header = button('Notes', () => list())
  header.className = 'reader-action notes-open'
  header.setAttribute('data-marknoria-notes', '')
  header.title = 'Comments and favorites'
  labels.set(header, 'Notes', {}, 'aria-label')
  const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  icon.setAttribute('viewBox', '0 0 24 24')
  icon.setAttribute('aria-hidden', 'true')
  const path = document.createElementNS(icon.namespaceURI, 'path')
  path.setAttribute('d', 'M5 3h14v18H5Z M9 7h6 M9 11h6 M9 15h3')
  icon.append(path)
  const label = document.createElement('span')
  labels.set(label, 'Notes')
  header.replaceChildren(icon, label)
  toolbar.append(header)
  let selectedId = null,
    filterKind = null,
    listOffset = 0
  let confirmation = null,
    previousFocus = null
  function detachOverlay(target) {
    if (!target) return
    // Feedback/selection controls temporarily live in the active modal's top
    // layer. They belong to this component, not to that disposable dialog.
    if (target.contains(status)) overlayRoot.append(status)
    if (target.contains(controls)) {
      controls.hidden = true
      overlayRoot.append(controls)
    }
  }
  function close(restoreFocus = true) {
    detachOverlay(confirmation)
    detachOverlay(dialog)
    confirmation?.remove()
    confirmation = null
    dialog?.remove()
    dialog = null
    if (restoreFocus) {
      if (!stopped && previousFocus?.isConnected) previousFocus.focus({ preventScroll: true })
      previousFocus = null
    }
  }
  function clearAll() {
    if (confirmation) return
    const kind = filterKind,
      scope = kind === 'comment' ? 'comments' : kind === 'favorite' ? 'favorites' : 'notes'
    const owner = dialog,
      target = document.createElement('dialog')
    confirmation = target
    target.className = 'notes-dialog notes-confirm'
    decorate(target)
    labels.set(target, `Clear all ${scope}?`, {}, 'aria-label')
    const heading = document.createElement('header')
    heading.className = 'notes-heading'
    const title = document.createElement('h2')
    labels.set(title, `Clear all ${scope}?`)
    heading.append(
      title,
      closeButton(() => target.close())
    )
    const body = document.createElement('p')
    body.className = 'notes-confirm-body'
    labels.set(
      body,
      `Delete all ${kind ? scope : 'comments and favorites'} for this document, across every page? This cannot be undone.`
    )
    const actions = document.createElement('footer')
    actions.className = 'notes-editor-actions'
    const message = document.createElement('span')
    message.role = 'status'
    const cancel = button('Cancel', () => target.close()),
      erase = button('Clear all', async () => {
        erase.disabled = true
        cancel.disabled = true
        try {
          await noteApi({ operation: 'clear', kind })
          selectedId = null
          target.close()
          if (dialog === owner) await list(0, filterKind)
          feedback(`Cleared all ${scope} for this document`)
        } catch (e) {
          labels.clear(message)
          message.textContent = noteError(e)
          erase.disabled = false
          cancel.disabled = false
        }
      })
    erase.className = 'notes-danger'
    actions.append(message, cancel, erase)
    target.append(heading, body, actions)
    overlayRoot.append(target)
    target.addEventListener('close', () => {
      detachOverlay(target)
      target.remove()
      if (confirmation === target) confirmation = null
    })
    target.showModal()
    cancel.focus()
  }
  function shell(title, mode = 'library') {
    previousFocus ||= document.activeElement
    close(false)
    controls.hidden = true
    dialog = document.createElement('dialog')
    dialog.className = `notes-dialog notes-${mode}`
    decorate(dialog)
    labels.set(dialog, title, {}, 'aria-label')
    const bar = document.createElement('header')
    bar.className = 'notes-heading'
    const titles = document.createElement('div'),
      h = document.createElement('h2'),
      subtitle = document.createElement('p')
    labels.set(h, title)
    subtitle.textContent = index.name
    titles.append(h, subtitle)
    const exit = closeButton(close)
    exit.classList.add('notes-close')
    bar.append(titles, exit)
    dialog.append(bar)
    overlayRoot.append(dialog)
    dialog.addEventListener('cancel', close)
    dialog.showModal()
    return dialog
  }
  function renderLocation(node, note, showStale = false) {
    const anchor = note.anchor || {},
      parts = []
    const add = (key, values) => {
      const span = document.createElement('span')
      labels.set(span, key, values)
      parts.push(span)
    }
    if (anchor.query) {
      add('SQL result')
      add('Row {row}', { row: anchor.query.row + 1 })
      add('Column {column}', { column: anchor.query.column + 1 })
      if (anchor.query.columnName) {
        const span = document.createElement('span')
        span.textContent = anchor.query.columnName
        parts.push(span)
      }
    } else {
      if (anchor.line) add('Line {line}', { line: anchor.line })
      if (anchor.table != null) add('Table {table}', { table: anchor.table + 1 })
      if (anchor.row != null) add('Row {row}', { row: anchor.row + 1 })
      if (anchor.column != null) add('Column {column}', { column: anchor.column + 1 })
      if (anchor.page != null && !anchor.line) add('Page {page}', { page: anchor.page + 1 })
      if (!parts.length) {
        if (anchor.location?.kind === 'pdf')
          add('PDF page {page}', { page: anchor.location.pages[0].page })
        else if (anchor.location?.kind === 'epub') add('EPUB selection')
        else if (anchor.label) {
          const span = document.createElement('span')
          span.textContent = anchor.label
          parts.push(span)
        } else add('Document selection')
      }
    }
    if (showStale && note.stale) add('Earlier file revision')
    node.replaceChildren(...parts.flatMap((part, index) => (index ? [' · ', part] : [part])))
  }

  function edit(note) {
    if (stopped) return
    if (!note && !canSave()) {
      feedback('Save your edits before adding a comment')
      return
    }
    const selection = note ? { quote: note.quote, anchor: note.anchor } : captured
    if (!selection) return
    const target = shell(note ? 'Edit comment' : 'Add comment', 'editor')
    const content = document.createElement('section')
    content.className = 'notes-editor-body'
    const quoteTitle = document.createElement('h3')
    labels.set(quoteTitle, 'Selected text')
    const quote = document.createElement('blockquote')
    quote.textContent = selection.quote
    const label = document.createElement('label')
    labels.set(label, 'Comment')
    label.htmlFor = 'note-comment-' + crypto.randomUUID()
    const input = document.createElement('textarea')
    input.id = label.htmlFor
    input.value = note?.comment || ''
    input.maxLength = 16384
    labels.set(input, 'Comment', {}, 'aria-label')
    input.placeholder = 'Add your thoughts about this passage…'
    const message = document.createElement('span')
    message.role = 'status'
    const actions = document.createElement('footer')
    actions.className = 'notes-editor-actions'
    const save = button('Save comment', async () => {
      save.disabled = true
      try {
        await noteApi(
          note
            ? { operation: 'update', id: note.id, comment: input.value }
            : {
                operation: 'add',
                kind: 'comment',
                quote: selection.quote,
                comment: input.value,
                anchor: selection.anchor,
                revision: index.revision
              }
        )
        feedback('Saved comment')
        if (dialog === target) await list(listOffset, filterKind)
      } catch (e) {
        labels.clear(message)
        message.textContent = noteError(e)
        save.disabled = false
      }
    })
    save.className = 'notes-primary'
    actions.append(
      message,
      button('Cancel', () => list(listOffset, filterKind)),
      save
    )
    content.append(quoteTitle, quote, label, input)
    target.append(content, actions)
    input.focus()
  }
  async function list(offset = 0, kind = null) {
    if (stopped) return
    listOffset = offset
    filterKind = kind
    const target = shell('Notes'),
      filters = document.createElement('nav')
    filters.className = 'notes-filters'
    labels.set(filters, 'Note type', {}, 'aria-label')
    for (const [label, value] of [
      ['All', null],
      ['Comments', 'comment'],
      ['Favorites', 'favorite']
    ]) {
      const b = button(label, () => list(0, value))
      b.setAttribute('aria-pressed', String(value === kind))
      filters.append(b)
    }
    const erase = button(
      kind === 'comment' ? 'Clear comments' : kind === 'favorite' ? 'Clear favorites' : 'Clear all',
      clearAll
    )
    erase.className = 'notes-danger notes-clear'
    erase.disabled = true
    filters.append(erase)
    const workspace = document.createElement('div')
    workspace.className = 'notes-workspace'
    const sidebar = document.createElement('section')
    sidebar.className = 'notes-list'
    labels.set(sidebar, 'Saved notes', {}, 'aria-label')
    const detail = document.createElement('section')
    detail.className = 'notes-detail'
    const footer = document.createElement('footer')
    footer.className = 'notes-footer'
    const message = document.createElement('span')
    message.role = 'status'
    labels.set(message, 'Loading…')
    footer.append(message)
    workspace.append(sidebar, detail)
    target.append(filters, workspace, footer)
    try {
      const result = await noteApi({ operation: 'list', offset, kind })
      if (stopped || dialog !== target) return
      if (
        !result ||
        !Array.isArray(result.items) ||
        result.items.length > 64 ||
        !Number.isSafeInteger(result.total) ||
        result.total < 0 ||
        !Number.isInteger(result.limit) ||
        result.limit < 1 ||
        result.limit > 64 ||
        result.items.some(
          (note) =>
            !note ||
            typeof note.id !== 'string' ||
            note.id.length > 256 ||
            !['comment', 'favorite'].includes(note.kind) ||
            typeof note.quote !== 'string' ||
            note.quote.length > 32768 ||
            new TextEncoder().encode(note.quote).length > 32768 ||
            typeof note.comment !== 'string' ||
            note.comment.length > 16384 ||
            new TextEncoder().encode(note.comment).length > 16384 ||
            !Number.isSafeInteger(note.created) ||
            note.created < 0
        )
      )
        throw Error('Invalid bounded note storage page')
      erase.disabled = result.total === 0
      labels.set(message, result.total === 1 ? '{count} saved item' : '{count} saved items', {
        count: result.total
      })
      function show(note) {
        selectedId = note.id
        for (const row of sidebar.children)
          row.setAttribute('aria-pressed', String(row.dataset.noteId === note.id))
        const card = document.createElement('article')
        card.className = 'note-card'
        card.dataset.noteId = note.id
        const bar = document.createElement('div')
        bar.className = 'note-card-heading'
        const badge = document.createElement('strong')
        badge.className = `note-kind ${note.kind}`
        labels.set(badge, note.kind === 'favorite' ? '★ Favorite' : 'Comment')
        const time = document.createElement('time')
        labels.date(time, note.created)
        bar.append(badge, time)
        const origin = document.createElement('p')
        origin.className = 'note-location'
        renderLocation(origin, note, true)
        const quoteLabel = document.createElement('h3')
        labels.set(
          quoteLabel,
          note.anchor?.quoteTruncated ? 'Selected text excerpt' : 'Selected text'
        )
        const quote = document.createElement('blockquote')
        quote.textContent = note.quote
        const commentLabel = document.createElement('h3')
        labels.set(commentLabel, 'Comment')
        const body = document.createElement('p')
        body.className = 'note-comment'
        if (note.comment) body.textContent = note.comment
        else labels.set(body, 'No comment added.')
        const actions = document.createElement('div')
        actions.className = 'notes-actions'
        const copy = button('Copy reference', async () => {
          const value = await noteApi({ operation: 'copy', id: note.id })
          await copyText(value.text)
        })
        copy.className = 'notes-primary'
        const remove = button('Delete', async () => {
          await noteApi({ operation: 'delete', id: note.id })
          selectedId = null
          if (dialog === target) await list(offset, kind)
        })
        remove.className = 'notes-danger'
        actions.append(
          copy,
          button('Edit comment', () => edit(note)),
          remove
        )
        card.append(bar, origin, quoteLabel, quote, commentLabel, body, actions)
        detail.replaceChildren(card)
      }
      // Keep row summaries compact; full passages belong in the detail pane.
      for (const note of result.items) {
        const row = button('', () => show(note))
        row.className = 'note-row'
        row.dataset.noteId = note.id
        const title = document.createElement('strong')
        labels.set(title, note.kind === 'favorite' ? '★ Favorite' : 'Comment')
        const excerpt = document.createElement('span')
        excerpt.textContent = note.quote.slice(0, 320)
        const meta = document.createElement('small')
        renderLocation(meta, note)
        row.append(title, excerpt, meta)
        sidebar.append(row)
      }
      if (result.items.length)
        show(result.items.find((n) => n.id === selectedId) || result.items[0])
      else {
        const empty = document.createElement('div')
        empty.className = 'notes-empty'
        const title = document.createElement('h3')
        labels.set(title, 'No saved notes')
        const help = document.createElement('p')
        labels.set(help, 'Select text in the document, then choose Comment or Favorite.')
        empty.append(title, help)
        detail.append(empty)
      }
      const pager = document.createElement('div')
      pager.className = 'notes-actions'
      if (offset > 0)
        pager.append(button('Previous', () => list(Math.max(0, offset - result.limit), kind)))
      if (offset + result.limit < result.total)
        pager.append(button('Next', () => list(offset + result.limit, kind)))
      footer.append(pager)
    } catch (e) {
      if (!stopped && dialog === target) {
        labels.clear(message)
        message.textContent = noteError(e)
      }
    }
  }
  let selectionTicket = 0,
    selectionRequest = null
  async function selected() {
    const ticket = ++selectionTicket
    if (stopped || dialog) return
    controls.hidden = true
    selectionRequest?.abort()
    selectionRequest = new AbortController()
    try {
      const value = await selectionProvider(selectionRequest.signal)
      if (ticket !== selectionTicket || stopped || dialog) return
      if (
        !value ||
        value.documentId !== owner.documentId ||
        value.revision !== owner.revision ||
        typeof value.quote !== 'string' ||
        value.quote.length > 32768 ||
        !value.location ||
        (!value.quote.trim() && value.location?.kind !== 'pdf') ||
        new TextEncoder().encode(value.quote).length > 32768
      ) {
        controls.hidden = true
        return
      }
      const encodedLocation = JSON.stringify(value.location)
      if (
        encodedLocation.length > 32768 ||
        new TextEncoder().encode(encodedLocation).length > 32768
      )
        throw Error('Note location exceeds its transport budget')
      const location = JSON.parse(encodedLocation)
      const reference = {
        documentId: owner.documentId,
        revision: owner.revision,
        quote: value.quote,
        quoteTruncated: Boolean(value.quoteTruncated),
        location,
        precision: value.precision,
        viewportRect: value.viewportRect
          ? {
              x: value.viewportRect.x,
              y: value.viewportRect.y,
              width: value.viewportRect.width,
              height: value.viewportRect.height
            }
          : null
      }
      const anchor = {
        location,
        ...(value.quoteTruncated ? { quoteTruncated: true } : {})
      }
      if (value.location.kind === 'line') anchor.line = value.location.line
      if (value.location.kind === 'cell')
        Object.assign(anchor, {
          table: value.location.table,
          row: value.location.row,
          column: value.location.column
        })
      captured = { quote: value.quote, anchor, reference }
      // A capable storage adapter resolves the live result into durable SQL
      // provenance. Never turn result coordinates into original source cells.
      labels.set(controls.children[0], value.quoteTruncated ? 'Copy excerpt' : 'Copy')
      controls.children[1].hidden = controls.children[2].hidden =
        value.location.kind === 'queryCell' && !queryNotes
      decorate(controls)
      const bounds = value.viewportRect
      if (
        !bounds ||
        ![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite) ||
        bounds.width < 0 ||
        bounds.height < 0
      ) {
        controls.hidden = true
        return
      }
      const modal = document.activeElement?.closest('dialog[open]')
      const surface = root.getBoundingClientRect()
      const area = modal?.getBoundingClientRect() || {
        left: Math.max(0, surface.left),
        top: Math.max(0, surface.top),
        right: Math.min(innerWidth, surface.right),
        bottom: Math.min(innerHeight, surface.bottom),
        width: Math.max(0, Math.min(innerWidth, surface.right) - Math.max(0, surface.left))
      }
      const parent = modal || overlayRoot
      // Reparent only across overlays. Moving a pressed button cancels its click.
      if (controls.parentElement !== parent) parent.append(controls)
      controls.style.maxWidth = `${Math.max(0, area.width - 16)}px`
      controls.style.flexWrap = 'wrap'
      controls.style.maxHeight = `${Math.max(0, area.bottom - area.top - 16)}px`
      controls.style.overflow = 'auto'
      controls.hidden = false
      controls.style.left = `${Math.max(area.left + 8, Math.min(area.right - controls.offsetWidth - 8, bounds.x))}px`
      controls.style.top = `${Math.max(area.top + 8, Math.min(area.bottom - controls.offsetHeight - 8, bounds.y - controls.offsetHeight - 6))}px`
    } catch (error) {
      if (!stopped && ticket === selectionTicket && error?.name !== 'AbortError') {
        controls.hidden = true
        feedback(noteError(error), true)
      }
    }
  }
  eventRoot.addEventListener('pointerup', selected, { signal: lifetime.signal })
  eventRoot.addEventListener('keyup', selected, { signal: lifetime.signal })
  root.ownerDocument.defaultView.addEventListener(
    'scroll',
    () => {
      selectionTicket++
      selectionRequest?.abort()
      controls.hidden = true
    },
    { capture: true, signal: lifetime.signal }
  )
  const themeObserver = new MutationObserver(() => {
    for (const node of [status, controls, dialog, confirmation]) if (node) decorate(node)
  })
  themeObserver.observe(presentationRoot, {
    attributes: true,
    attributeFilter: ['class', 'style', 'data-theme']
  })
  themeObserver.observe(root.ownerDocument.documentElement, {
    attributes: true,
    attributeFilter: ['class', 'style', 'data-theme']
  })
  return {
    state: () => ({
      busy: activeRequests > 0,
      pendingRequests: activeRequests,
      disposed: stopped,
      open: Boolean(dialog),
      filter: filterKind
    }),
    open: list,
    selectionChanged: selected,
    dispose() {
      if (stopped) return
      stopped = true
      releaseStyles()
      themeObserver.disconnect()
      lifetime.abort()
      selectionRequest?.abort()
      clearTimeout(status.timer)
      close()
      header.remove()
      controls.remove()
      status.remove()
    }
  }
}

export function notesController(index, api, canSave = () => true, selectionProvider, options = {}) {
  const root = document.querySelector('.reader-shell')
  return mountNotes({
    owner: { documentId: options.documentId, revision: index.revision, name: index.name },
    root,
    toolbar: root.querySelector('.reader-document-tools'),
    eventRoot: document,
    storage: (request, { signal }) =>
      api('/api/notes', signal, { method: 'POST', body: JSON.stringify(request) }),
    selection: selectionProvider,
    canSave,
    queryNotes: true,
    translate: options.translate
  })
}

export function readerNoteStorage(client, owner) {
  const identity = { documentId: owner.documentId, revision: owner.revision }
  return (request, { signal }) => client.request('notes', { ...identity, request }, { signal })
}
