// Studio owns storage and UI. Only exact, revision-owned references are handed
// to the shared painter; no DOM matching or implicit reader notebook reads.
export function readerCommentMark(draft, file, owner) {
  const source = draft?.source, anchor = source?.anchor, reference = anchor?.readerReference
  if (!['document', 'epub'].includes(source?.provider) || anchor?.filePath !== file?.path
    || anchor.root && file.root && anchor.root !== file.root || !reference
    || reference.revision !== owner?.revision || reference.precision === 'draft'
    || !reference.textRange && !['source', 'pdf', 'epub'].includes(reference.location?.kind)) return null
  return { id: draft.id, kind: 'comment', revision: reference.revision,
    quote: reference.quote ?? draft.excerpt,
    anchor: { location: reference.location, quoteTruncated: reference.quoteTruncated,
      ...(reference.textRange ? { textRange: reference.textRange } : {}) } }
}

export function createReaderCommentHost({ onError = () => {}, open = () => {} } = {}) {
  let disposed = false, owner = null, host = null, file = null, drafts = [], signature = '', ids = new Set()
  let generation = 0, running = false, pending = null, request = null
  async function drain() {
    if (running) return
    running = true
    try {
      while (pending && !disposed) {
        const work = pending; pending = null
        if (work.generation !== generation) continue
        const operation = request = new AbortController()
        try {
          await work.host.request('setCommentMarks', { documentId: work.owner.documentId, revision: work.owner.revision, marks: work.marks }, { signal: operation.signal })
        } catch (error) {
          if (!disposed && work.generation === generation && !operation.signal.aborted && error?.code !== 'CANCELLED') onError(error)
        }
      }
    } finally { running = false }
  }
  function replace() {
    if (disposed || !host || !owner?.ready || owner.capabilities?.commentMarks?.version !== 1) return
    let marks = drafts.map(draft => readerCommentMark(draft, file, owner)).filter(Boolean)
    const next = JSON.stringify([owner.documentId, owner.revision, marks])
    if (signature === next) return
    signature = next; ids = new Set(marks.map(mark => mark.id))
    request?.abort()
    const own = ++generation
    if (marks.length > Math.min(512, owner.capabilities.commentMarks.records || 512) || new TextEncoder().encode(JSON.stringify(marks)).length > 2 * 1024 * 1024) {
      ids.clear(); marks = []; onError(Error('Comment marks exceed the reader budget'))
    }
    pending = { generation: own, owner: { documentId: owner.documentId, revision: owner.revision }, host, marks }
    void drain()
  }
  return {
    update(next, transport, records, document) {
      if (disposed) return
      const unchanged = host === transport && file === document && (drafts === records || !drafts.length && !records.length)
        && owner?.documentId === next.documentId && owner?.revision === next.revision && owner?.ready === next.ready
      owner = next; host = transport; drafts = records; file = document
      if (!unchanged) replace()
    },
    refresh(records, document) { if (disposed || file === document && (drafts === records || !drafts.length && !records.length)) return; drafts = records; file = document; replace() },
    open(payload) {
      if (disposed || !owner || payload.documentId !== owner.documentId || payload.revision !== owner.revision) return
      if (payload.id === null ? ids.size > 0 : ids.has(payload.id)) open(payload.id)
    },
    dispose() { disposed = true; ++generation; request?.abort(); pending = null; ids.clear(); host = owner = null; drafts = [] },
  }
}
