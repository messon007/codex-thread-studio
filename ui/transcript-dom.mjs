// Only the currently mounted transcript is retained. No detached session DOMs.
export function createTranscriptDom({ maxBytes = 4 * 1024 * 1024 } = {}) {
  let context = ''
  let entries = new Map()
  return {
    invalidate(id) { entries.delete(String(id)) },
    render(container, key, chunks) {
      if (context !== key) entries.clear()
      context = key
      const next = new Map()
      const desired = []
      let bytes = 0
      let reused = 0
      for (const chunk of chunks) {
        const cached = entries.get(chunk.id)
        let nodes
        if (cached?.html === chunk.html && cached.nodes.every((node) => node.parentNode === container)) {
          nodes = cached.nodes
          if (!chunk.id.startsWith('__')) reused += 1
        } else {
          const template = container.ownerDocument.createElement('template')
          template.innerHTML = chunk.html
          nodes = [...template.content.childNodes]
        }
        desired.push(...nodes)
        const size = chunk.html.length * 2
        if (bytes + size <= maxBytes) {
          bytes += size
          next.set(chunk.id, { html: chunk.html, nodes })
        }
      }
      // Remove stale nodes first so changing one Turn does not detach/move all
      // subsequent Turns (which can reset focus, selection and embedded views).
      const retained = new Set(desired)
      for (const node of [...container.childNodes]) {
        if (!retained.has(node)) node.remove()
      }
      let cursor = container.firstChild
      for (const node of desired) {
        if (node === cursor) cursor = cursor.nextSibling
        else container.insertBefore(node, cursor)
      }
      entries = next
      return { reusedTurns: reused, cachedDomBytes: bytes }
    },
  }
}
