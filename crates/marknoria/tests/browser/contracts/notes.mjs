export async function notesContract(moduleUrl) {
  const { mountNotes } = await import(moduleUrl)
  const host = document.createElement('section')
  host.style.cssText =
    'position:fixed;inset:10px;background:white;z-index:9999;display:flex;gap:20px'
  document.body.append(host)
  const area = () => {
    const root = document.createElement('section'),
      toolbar = document.createElement('nav')
    root.style.cssText = 'width:500px;height:700px;max-height:90vh;border:1px solid #ccc'
    root.append(toolbar)
    host.append(root)
    return { root, toolbar, overlayRoot: root }
  }
  const left = area(),
    right = area(),
    owner = { documentId: 'document-a', revision: 'revision-a', name: 'Fixture 中文😀.md' },
    ownerB = { ...owner, documentId: 'document-b' }
  let value = {
      ...owner,
      quote: 'Literal <script> 中文😀',
      precision: 'exact',
      location: { kind: 'source', startByte: 0, endByte: 20 },
      viewportRect: { x: 30, y: 100, width: 100, height: 20 }
    },
    notes,
    other,
    mode = 'normal',
    late,
    lateSignal,
    lateSelection,
    lateGet,
    finishUpdate,
    finishNavigation,
    navigationMode = 'normal',
    navigationSignal,
    navigated = [],
    selectionSignal,
    actionReference,
    copy = []
  const records = new Map([
      [owner.documentId, []],
      [ownerB.documentId, []]
    ]),
    calls = [],
    actions = [],
    initialStyles = document.querySelectorAll('link[href$="/notes.css"]').length
  let assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const tick = () => new Promise((resolve) => setTimeout(resolve, 25))
  const wait = async (predicate) => {
    const deadline = performance.now() + 5000
    while (!predicate()) {
      if (performance.now() > deadline)
        throw Error(
          'Notes contract timed out: ' +
            [...host.querySelectorAll('[role="status"]')]
              .map((node) => node.textContent)
              .join(' | ')
        )
      await tick()
    }
  }
  const storage = async (request, { owner, signal }) => {
    calls.push({ request: structuredClone(request), documentId: owner.documentId })
    const items = records.get(owner.documentId)
    if (mode === 'mutation-error' && ['update','delete'].includes(request.operation)) throw Error('Write failed; retry')
    if (mode === 'update-pending' && request.operation === 'update') await new Promise(resolve => { finishUpdate = resolve })
    if (request.operation === 'get') {
      const note = structuredClone(items.find(item => item.id === request.id))
      if (mode === 'get-pending') return new Promise(resolve => { lateGet = () => resolve(note) })
      if (mode === 'get-error') throw Error('Comment fetch failed')
      if (mode === 'get-stale') note.revision = 'different-revision'
      return note
    }
    if (mode === 'pending' && request.operation === 'add') {
      lateSignal = signal
      return await new Promise((resolve) => (late = resolve))
    }
    if (request.operation === 'add') {
      items.push({
        id: crypto.randomUUID(),
        kind: request.kind,
        quote: request.quote,
        comment: request.comment || '',
        anchor: structuredClone(request.anchor),
        revision: request.revision,
        created: Date.now(),
        stale: false
      })
      return { added: true }
    }
    if (request.operation === 'list') {
      const matches = items.filter((item) => !request.kind || item.kind === request.kind)
      return {
        items: matches.slice(request.offset, request.offset + 64),
        total: matches.length,
        limit: 64
      }
    }
    if (request.operation === 'clear') {
      records.set(
        owner.documentId,
        items.filter((item) => request.kind && item.kind !== request.kind)
      )
      return { cleared: true }
    }
    if (request.operation === 'copy') return { text: 'Copied reference 中文😀' }
    if (request.operation === 'delete') {
      records.set(
        owner.documentId,
        items.filter((item) => item.id !== request.id)
      )
      return { deleted: true }
    }
    if (request.operation === 'update') {
      items.find((item) => item.id === request.id).comment = request.comment
      return { updated: true }
    }
    throw Error('Unexpected storage action')
  }
  const button = (root, label) =>
    [...root.querySelectorAll('button')].find((node) => node.textContent === label)
  const mount = (queryNotes = false, translate) =>
    mountNotes({
      ...left,
      owner,
      storage,
      queryNotes,
      translate,
      navigate: async (note, {signal}) => {
        navigationSignal = signal
        navigated.push(note)
        if (navigationMode === 'error') throw Error('Passage is unavailable')
        if (navigationMode === 'pending') await new Promise(resolve => { finishNavigation = resolve })
        signal.throwIfAborted()
        return {focus:()=>check(!left.root.querySelector('.notes-dialog'),'Navigation focused behind the Notes dialog')}
      },
      // This synthetic provider models a remote textOnly selection response.
      selectionAllowed: () => true,
      selection: async (signal) => {
        if (mode === 'selection-pending') {
          selectionSignal = signal
          return await new Promise((resolve) => (lateSelection = resolve))
        }
        return value
      },
      copy: async (text) => copy.push(text),
      actions: [
        {
          id: 'host-action',
          label: 'Host action',
          run: async (reference, { signal }) => {
            actionReference = reference
            actions.push(signal)
          }
        },
        {
          id: 'failing-action',
          label: 'Failing action',
          run: () => {
            throw 'Host action failed as expected'
          }
        }
      ]
    })
  try {
    notes = mount()
    other = mountNotes({
      ...right,
      owner: ownerB,
      storage,
      selection: async () => ({ ...value, ...ownerB })
    })
    await notes.selectionChanged()
    const menu = left.root.querySelector('.notes-toolbar')
    await wait(() => getComputedStyle(menu).display === 'flex')
    check(
      !menu.hidden && right.root.querySelector('.notes-toolbar').hidden,
      'A selection opened another document menu'
    )
    left.root.style.setProperty('--reader-panel', '#123456')
    await tick()
    check(
      getComputedStyle(menu).backgroundColor === 'rgb(18, 52, 86)',
      'Presentation palette did not update an existing control'
    )
    left.root.style.removeProperty('--reader-panel')
    await tick()
    button(menu, 'Copy').click()
    await tick()
    check(copy[0] === value.quote, 'Injected clipboard changed the quote')
    button(menu, 'Host action').click()
    await tick()
    check(
      actionReference.documentId === owner.documentId &&
        actionReference.location.kind === 'source' &&
        actionReference.quote === value.quote &&
        !actions[0].aborted,
      'Host action did not receive a typed scoped reference'
    )
    button(menu, 'Failing action').click()
    await tick()
    check(
      left.root.querySelector('.notes-status').textContent === 'Host action failed as expected',
      'Synchronous non-Error host rejection escaped feedback'
    )
    mode = 'selection-pending'
    const staleSelection = notes.selectionChanged()
    await tick()
    mode = 'normal'
    await notes.selectionChanged()
    lateSelection({ ...value, quote: 'stale quote' })
    await staleSelection
    button(menu, 'Copy').click()
    await tick()
    check(
      selectionSignal.aborted && copy.at(-1) === value.quote,
      'A superseded selection request replaced the active reference'
    )
    value = { ...value, documentId: 'foreign' }
    await notes.selectionChanged()
    check(menu.hidden, 'Foreign document selection opened a menu')
    value = { ...value, documentId: owner.documentId, revision: 'foreign' }
    await notes.selectionChanged()
    check(menu.hidden, 'Foreign revision selection opened a menu')
    value = { ...value, revision: owner.revision }
    await notes.selectionChanged()
    button(menu, 'Favorite').click()
    await wait(() => records.get(owner.documentId).length === 1)
    await notes.selectionChanged()
    button(menu, 'Comment').click()
    const editor = left.root.querySelector('.notes-editor')
    check(
      editor?.open &&
        editor.querySelector('blockquote').textContent === value.quote &&
        !editor.querySelector('script'),
      'Comment preview changed or executed quote text'
    )
    editor.querySelector('textarea').value = 'Comment 中文😀'
    button(editor, 'Save comment').click()
    await wait(() => !left.root.querySelector('.notes-dialog'))
    check(!left.root.querySelector('.notes-library'),'Saving a comment unexpectedly opened Notes')
    await notes.open()
    check(
      records.get(owner.documentId).length === 2 && records.get(ownerB.documentId).length === 0,
      'Storage crossed owning documents'
    )
    if (innerWidth <= 620) {
      const row = left.root.querySelector('.note-row'),
        summary = row.querySelector('span'),
        meta = row.querySelector('small'),
        bounds = row.getBoundingClientRect()
      check(
        summary.getBoundingClientRect().height >=
          parseFloat(getComputedStyle(summary).lineHeight) - 1 &&
          meta.getBoundingClientRect().bottom <= bounds.bottom - 3,
        'Compact note summary clipped text or metadata'
      )
    }
    const library = () => left.root.querySelector('.notes-library')
    const savedComment = records.get(owner.documentId).find(note => note.kind === 'comment')
    navigationMode = 'error'
    button(library(), 'Go to passage').click()
    await wait(()=>library()?.textContent.includes('Passage is unavailable'))
    check(Boolean(library()),'Failed navigation closed Notes')
    navigationMode = 'normal'
    button(library(), 'Go to passage').click()
    await wait(()=>!library())
    check(navigated.at(-1).anchor.location.kind==='source','Navigation lost the typed saved anchor')
    await notes.open()
    navigationMode='pending'
    button(library(),'Go to passage').click()
    await wait(()=>finishNavigation)
    library().querySelector('.notes-close').click()
    finishNavigation();await tick()
    check(navigationSignal.aborted&&!library(),'Canceled navigation reopened Notes')
    navigationMode='normal'
    const beforeOpen = calls.length
    await notes.openComment(savedComment.id)
    check(left.root.querySelector('.notes-editor textarea')?.value === 'Comment 中文😀' && !library(),
      'Marked comment did not open directly in its full-body editor')
    check(calls.slice(beforeOpen).map(call => call.request.operation).join() === 'get',
      'Opening one comment fetched the Notes list')
    left.root.querySelector('.notes-editor textarea').value='Discard this draft'
    button(left.root.querySelector('.notes-editor'),'Cancel').click()
    check(!left.root.querySelector('.notes-dialog'),'Cancel returned to the Notes manager')
    check(records.get(owner.documentId).find(note=>note.id===savedComment.id).comment==='Comment 中文😀','Cancel persisted an unsaved draft')
    mode = 'get-pending'
    const pendingOpen = notes.openComment(savedComment.id)
    await wait(() => lateGet)
    check(!left.root.querySelector('.notes-editor textarea'), 'Loading exposed an empty editable draft')
    left.root.querySelector('.notes-close').click()
    lateGet(); await pendingOpen
    check(!left.root.querySelector('.notes-dialog'), 'Late comment fetch reopened a closed editor')
    mode = 'get-error'
    await notes.openComment(savedComment.id)
    check(left.root.querySelector('.notes-editor [role="status"]')?.textContent === 'Comment fetch failed' &&
      !left.root.querySelector('.notes-editor textarea'), 'Failed fetch enabled a blank editor')
    mode = 'get-stale'
    await notes.openComment(savedComment.id)
    check(!left.root.querySelector('.notes-editor textarea'), 'Foreign revision entered the editor')
    mode = 'normal'
    await notes.openComment(savedComment.id)
    check(left.root.querySelector('.notes-editor textarea')?.value === savedComment.comment,
      'Comment could not reopen after a failed fetch')
    mode='mutation-error'
    const draft=left.root.querySelector('.notes-editor textarea')
    draft.value='Draft retained after failure'
    button(left.root.querySelector('.notes-editor'),'Save comment').click()
    await wait(()=>left.root.querySelector('.notes-editor [role=status]')?.textContent.includes('Write failed'))
    check(draft.value==='Draft retained after failure'&&!library(),'Failed save discarded draft or opened manager')
    mode='update-pending'
    button(left.root.querySelector('.notes-editor'),'Save comment').click()
    await wait(()=>finishUpdate)
    check(left.root.querySelector('.notes-editor textarea').readOnly,'Pending save allowed draft edits that would be discarded')
    button(left.root.querySelector('.notes-editor'),'Cancel').click()
    const nextEditor=notes.openComment(savedComment.id)
    finishUpdate();await nextEditor
    check(left.root.querySelector('.notes-editor textarea')?.value==='Draft retained after failure',
      'Late save closed or replaced a newer editor')
    mode='normal'
    button(left.root.querySelector('.notes-editor'),'Cancel').click()
    const disposable={...savedComment,id:crypto.randomUUID()}
    records.get(owner.documentId).push(disposable)
    await notes.openComment(disposable.id)
    mode='mutation-error'
    button(left.root.querySelector('.notes-editor'),'Delete').click()
    await wait(()=>left.root.querySelector('.notes-editor [role=status]')?.textContent.includes('Write failed'))
    check(records.get(owner.documentId).some(note=>note.id===disposable.id),'Failed delete lost comment')
    mode='normal'
    button(left.root.querySelector('.notes-editor'),'Delete').click()
    await wait(()=>!left.root.querySelector('.notes-dialog'))
    check(!records.get(owner.documentId).some(note=>note.id===disposable.id),'Editor delete did not persist')
    await notes.open()
    button(library(), 'Comments').click()
    await wait(() => library()?.querySelector('.notes-footer')?.textContent.includes('1 saved'))
    button(library(), 'Clear comments').click()
    let confirm = left.root.querySelector('.notes-confirm')
    check(
      confirm?.open && confirm.getBoundingClientRect().height < 350,
      'Clear confirmation expanded to library height'
    )
    button(confirm, 'Clear all').click()
    await wait(
      () => records.get(owner.documentId).length === 1 && !left.root.querySelector('.notes-confirm')
    )
    check(
      records.get(owner.documentId)[0].kind === 'favorite' &&
        calls.filter((item) => item.request.operation === 'clear').at(-1).request.kind ===
          'comment',
      'Clear comments deleted favorites'
    )
    button(library(), 'Favorites').click()
    await wait(() => library()?.querySelector('.notes-footer')?.textContent.includes('1 saved'))
    button(library(), 'Copy reference').click()
    await tick()
    check(copy.at(-1) === 'Copied reference 中文😀', 'Stored reference copy bypassed adapter')
    button(library(), 'Clear favorites').click()
    confirm = left.root.querySelector('.notes-confirm')
    button(confirm, 'Clear all').click()
    await wait(() => records.get(owner.documentId).length === 0)
    check(
      calls.filter((item) => item.request.operation === 'clear').at(-1).request.kind === 'favorite',
      'Favorites clear lost its category'
    )
    library().querySelector('[aria-label="Close"]').click()
    await notes.selectionChanged()
    button(menu, 'Copy').click()
    await tick()
    check(
      left.root.querySelector('.notes-status')?.textContent === 'Copied',
      'Closing a modal removed the component feedback surface'
    )
    value = {
      ...value,
      location: { kind: 'queryCell', queryId: crypto.randomUUID(), row: 1, column: 2 }
    }
    await notes.selectionChanged()
    check(
      button(menu, 'Comment').hidden &&
        button(menu, 'Favorite').hidden &&
        !button(menu, 'Host action').hidden,
      'Transient query references invented durable source notes or lost host actions'
    )
    value = { ...value, location: { kind: 'source', startByte: 0, endByte: 20 } }
    await notes.selectionChanged()
    mode = 'pending'
    button(menu, 'Comment').click()
    const closingEditor = left.root.querySelector('.notes-editor')
    closingEditor.querySelector('textarea').value = 'Late closed comment'
    button(closingEditor, 'Save comment').click()
    await wait(() => lateSignal)
    closingEditor.querySelector('[aria-label="Close"]').click()
    late({ added: true })
    await tick()
    await tick()
    check(
      !left.root.querySelector('.notes-dialog') && !notes.state().open && !notes.state().busy,
      'A late comment save reopened a closed editor'
    )
    lateSignal = null
    await notes.selectionChanged()
    mode = 'pending'
    button(menu, 'Favorite').click()
    await wait(() => lateSignal)
    notes.dispose()
    notes.dispose()
    check(lateSignal.aborted && actions[0].aborted, 'Dispose did not abort storage/actions')
    late({ added: true })
    await tick()
    await tick()
    check(
      !left.root.querySelector('[data-marknoria-notes]') && right.root.querySelector('.notes-open'),
      'Late storage resurrected notes or removed another owner'
    )
    mode = 'normal'
    notes = mount(true)
    check(
      left.toolbar.querySelectorAll('.notes-open').length === 1,
      'Same-root remount duplicated toolbar controls'
    )
    value = {
      ...value,
      quote: 'Copy',
      location: { kind: 'queryCell', queryId: crypto.randomUUID(), row: 1, column: 2 }
    }
    await notes.selectionChanged()
    const queryMenu = left.root.querySelector('.notes-toolbar')
    check(
      !button(queryMenu, 'Favorite').hidden && !button(queryMenu, 'Comment').hidden,
      'Explicit durable query-note storage did not expose note actions'
    )
    button(queryMenu, 'Favorite').click()
    await wait(() => !notes.state().busy)
    check(
      calls.at(-1).request.anchor.location.queryId === value.location.queryId &&
        !calls.at(-1).request.anchor.table,
      'Query notes replaced live result provenance with a source-cell guess'
    )
    notes.dispose()
    const { createTranslator } = await import(
      new URL('./translation.mjs', new URL(moduleUrl, location.href))
    )
    const translator = createTranslator({
      locale: 'zh-CN',
      messages: {
        Notes: '笔记',
        Copy: '复制',
        '★ Favorite': '★ 收藏',
        '{count} saved item': '已保存 {count} 项',
        'Document selection': '文档选区'
      }
    })
    notes = mount(true, translator)
    await notes.selectionChanged()
    check(
      left.toolbar.querySelector('.notes-open').textContent === '笔记' &&
        left.root.querySelector('.notes-toolbar button').textContent === '复制',
      'Notes ignored the host translator'
    )
    await notes.open()
    check(
      left.root.querySelector('.notes-library h2').textContent === '笔记' &&
        left.root.querySelector('.note-card blockquote').textContent === 'Copy',
      'Notes translated quote data or missed the open library'
    )
    const record = records.get(owner.documentId)[0]
    const callsBefore = calls.length
    translator.update({ locale: 'en-US', messages: { Notes: 'Notebook', Copy: 'Copy text' } })
    check(
      left.root.querySelector('.notes-library h2').textContent === 'Notebook' &&
        left.root.querySelector('.note-card time').textContent ===
          new Intl.DateTimeFormat('en-US').format(record.created) &&
        calls.length === callsBefore,
      'Locale updates refetched records or missed existing date/text bindings'
    )
    const fullTranslator = createTranslator()
    const removers = Array.from({ length: 256 }, () => fullTranslator.subscribe(() => {}))
    let failed = false
    try {
      mount(false, fullTranslator)
    } catch {
      failed = true
    }
    check(
      failed && fullTranslator.state().subscribers === 256,
      'Failed Notes setup changed a full translation scope'
    )
    for (const remove of removers) remove()
    notes.dispose()
    check(translator.state().subscribers === 0, 'Notes disposal retained a translation subscriber')
    other.dispose()
    check(
      document.querySelectorAll('link[href$="/notes.css"]').length === initialStyles,
      'Dispose retained styles or removed another owner'
    )
    return {
      assertions,
      scopedStorage: true,
      hostActions: true,
      typedSelections: true,
      categoryClear: true,
      cancellation: true,
      lifecycle: true
    }
  } finally {
    notes?.dispose()
    other?.dispose()
    host.remove()
  }
}
