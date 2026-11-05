// Studio owns file authorization, gateway transport and comment/AI integration.
// Rendering and interactions come from the reviewed public reader package.
export async function createPdfReader({ container, bytes, initialPage = 1, search = '', onSelection, onPageChange, signal, translate = value => value }) {
  if (!container || !(bytes instanceof ArrayBuffer) || !bytes.byteLength || bytes.byteLength > 128 * 1024 * 1024) throw new Error('PDF requires a bounded original byte buffer')
  signal?.throwIfAborted()
  const { mountPdfReader } = await import('./reader-assets/pdf-reader.mjs')
  const revision = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(value => value.toString(16).padStart(2, '0')).join('')
  signal?.throwIfAborted()
  if (!container.isConnected) throw new DOMException('PDF host closed', 'AbortError')
  const reader = mountPdfReader(container, {
    documentId: crypto.randomUUID(), revision, resourceId: revision,
    data: new Uint8Array(bytes), viewport: container.closest('.artifact-content') || container.parentElement,
    translate, onSelection,
    onState: state => onPageChange?.(state.page),
  })
  const abort = () => { void reader.dispose().catch(() => {}) }
  signal?.addEventListener('abort', abort, { once: true })
  try {
    await reader.ready
    signal?.throwIfAborted()
    const outline = []
    let page = await reader.outline()
    while (true) {
      for (const item of page.items) outline.push({ id: `pdf-${item.id}`, depth: item.level, label: item.label,
        target: item.location ? { kind: 'pdf', page: item.location.pages[0].page } : null })
      if (outline.length >= page.total) break
      page = await reader.outline({ offset: outline.length })
    }
    const goToPage = page => reader.navigate({ kind: 'pdf', pages: [{ page, rects: [{ x: 0, y: 0, width: 1, height: 1 }] }] })
    if (initialPage !== 1) await goToPage(initialPage)
    if (search.trim()) {
      const results = await reader.search(search)
      if (results.matches.length) await reader.navigate(results.matches[0].location)
    }
    return {
      pageCount: reader.state().pages,
      outline: () => outline,
      goToPage,
      state: reader.state,
      selection: reader.selection,
      settings: reader.settings,
      async navigateReference(reference) {
        if (reference.revision !== revision) throw Error('The PDF has changed since this reference was captured')
        return reader.navigate(reference.location)
      },
      destroy: () => { signal?.removeEventListener('abort', abort); return reader.dispose() },
    }
  } catch (error) {
    signal?.removeEventListener('abort', abort)
    await reader.dispose()
    throw error
  }
}
