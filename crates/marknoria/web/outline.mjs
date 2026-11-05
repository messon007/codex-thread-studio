// Hierarchy/filter semantics adapted from Codex Thread Studio's
// ui-src/document-outline.mts (MIT). See third-party/README.md.
// Input is indexed metadata, never document source. Work yields every 512 rows;
// only viewport rows enter the DOM. Filtering preserves ancestor context and
// temporarily ignores collapsed branches without changing their saved state.
import { Virtualizer, observeElementRect, observeElementOffset, elementScroll } from './vendor/virtual-core.mjs'
import { uiLabels } from './translation.mjs'

export function mountOutline({ viewport, space, filter, feedback, navigate, translate = key => key, onError = () => {} }) {
  const lifetime = new AbortController(), labels = uiLabels(translate, { signal: lifetime.signal })
  let rows = [], visible = [], active = null, ticket = 0, itemTicket = 0, disposed = false, pending = false, focusId = null
  const collapsed = new Set(), mounted = new Map()
  const pause = () => new Promise(resolve => setTimeout(resolve, 0))
  const on = (node, type, listener) => node.addEventListener(type, listener, { signal: lifetime.signal })
  labels.set(filter, 'Filter contents', {}, 'aria-label')
  labels.set(filter, 'Filter contents…', {}, 'placeholder')
  const virtual = new Virtualizer({ count: 0, getScrollElement: () => viewport, estimateSize: () => 32,
    observeElementRect, observeElementOffset, scrollToFn: elementScroll, overscan: 3, onChange: render })
  const unsubscribe = translate.subscribe?.(() => render(), { signal: lifetime.signal })
  const stop = virtual._didMount()
  virtual._willUpdate()

  function render() {
    if (disposed) return
    const items = virtual.getVirtualItems(), keep = new Set(items.map(item => visible[item.index]?.id))
    for (const [id, node] of mounted) if (!keep.has(id)) { node.remove(); mounted.delete(id) }
    for (const item of items) {
      const row = visible[item.index]
      if (!row) continue
      let node = mounted.get(row.id)
      if (!node) {
        node = document.createElement('div'); node.className = 'toc-row'; node.dataset.outlineId = String(row.id)
        const toggle = document.createElement('button'), link = document.createElement('button')
        toggle.className = 'toc-branch'; link.className = 'toc-link'
        toggle.type = link.type = 'button'
        toggle.setAttribute('aria-label', translate('Toggle section'))
        link.textContent = row.label || translate('Untitled section'); link.title = row.label
        link.disabled = row.navigable === false
        // Removed virtual rows must not be retained by AbortSignal listeners.
        toggle.onclick = () => { collapsed.has(row.id) ? collapsed.delete(row.id) : collapsed.add(row.id); void refresh() }
        link.onclick = () => activate(row.id)
        node.append(toggle, link); space.append(node); mounted.set(row.id, node)
      }
      const [toggle, link] = node.children
      toggle.style.visibility = row.hasChildren ? 'visible' : 'hidden'
      toggle.disabled = !row.hasChildren
      toggle.textContent = filter.value.trim() || !collapsed.has(row.id) ? '▾' : '▸'
      if (row.hasChildren) toggle.setAttribute('aria-expanded', String(Boolean(filter.value.trim()) || !collapsed.has(row.id)))
      else toggle.removeAttribute('aria-expanded')
      if (row.id === active) link.setAttribute('aria-current', 'location'); else link.removeAttribute('aria-current')
      node.classList.toggle('toc-context', Boolean(row.contextOnly))
      node.style.paddingLeft = `${4 + Math.min(12, row.depth) * 12}px`
      node.style.transform = `translateY(${item.start}px)`
      toggle.setAttribute('aria-label', translate('Toggle section'))
      if (row.id === focusId) { focusId = null; link.focus({ preventScroll: true }) }
    }
    space.style.height = `${virtual.getTotalSize()}px`
  }

  function activate(id) {
    if (disposed || pending) return
    const current = ticket
    Promise.resolve().then(() => { if (!disposed && !pending && current === ticket) return navigate(id) }).catch(onError)
  }
  function reveal(id, focus = false) {
    const position = visible.findIndex(row => row.id === id)
    if (position < 0) return
    if (focus) focusId = id
    const top = position * 32, bottom = top + 32
    if (top < viewport.scrollTop || bottom > viewport.scrollTop + viewport.clientHeight)
      virtual.scrollToIndex(position, { align: 'auto', behavior: 'auto' })
    render()
  }
  async function refresh() {
    const current = ++ticket, needle = filter.value.trim().toLocaleLowerCase(), included = new Uint8Array(rows.length)
    pending = true
    viewport.setAttribute('aria-busy', 'true')
    let matches = 0
    for (let end = rows.length; end > 0;) {
      const start = Math.max(0, end - 512)
      for (let i = end - 1; i >= start; i--) {
        const row = rows[i], match = !needle || row.search.includes(needle)
        if (match) { included[i] = 1; matches++ }
        if (needle && included[i] && row.parent >= 0) included[row.parent] = 1
      }
      end = start
      if (end) { await pause(); if (disposed || ticket !== current) return }
    }
    const next = []; let hiddenDepth = Infinity
    for (let start = 0; start < rows.length; start += 512) {
      for (let i = start; i < Math.min(rows.length, start + 512); i++) {
        const row = rows[i]
        if (!needle) {
          if (row.depth > hiddenDepth) continue
          hiddenDepth = collapsed.has(row.id) ? row.depth : Infinity
        }
        if (included[i]) next.push({ ...row, contextOnly: Boolean(needle) && !row.search.includes(needle) })
      }
      if (start + 512 < rows.length) { await pause(); if (disposed || ticket !== current) return }
    }
    if (disposed || ticket !== current) return
    visible = next
    virtual.setOptions({ ...virtual.options, count: visible.length })
    virtual._willUpdate(); render()
    pending = false
    viewport.setAttribute('aria-busy', 'false')
    labels.set(feedback, needle ? '{count} matching sections' : '{count} sections', { count: needle ? matches : rows.length })
  }
  on(filter, 'input', () => { viewport.scrollTop = 0; void refresh() })
  on(filter, 'keydown', event => {
    if (event.key !== 'Enter') return
    event.preventDefault()
    const row = visible.find(row => !row.contextOnly && row.navigable !== false)
    if (row) activate(row.id)
  })
  on(viewport, 'keydown', event => {
    if (!['ArrowUp','ArrowDown','Home','End'].includes(event.key) || pending || !visible.length) return
    event.preventDefault()
    const id = event.target.closest?.('[data-outline-id]')?.dataset.outlineId
    const current = visible.findIndex(row => String(row.id) === id)
    const target = event.key === 'Home' ? 0 : event.key === 'End' ? visible.length - 1
      : Math.max(0, Math.min(visible.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)))
    reveal(visible[target].id, true)
  })
  return {
    async setItems(items) {
      const current = ++itemTicket
      ++ticket; pending = true
      const next = [], ancestors = [], ids = new Set()
      for (let start = 0; start < items.length; start += 512) {
        for (const item of items.slice(start, start + 512)) {
          if (ids.has(item.id)) throw Error('Duplicate outline identity')
          ids.add(item.id)
          const depth = Math.min(12, Math.max(0, Number(item.level || 1) - 1))
          while (ancestors.length && next[ancestors.at(-1)].depth >= depth) ancestors.pop()
          const parent = ancestors.at(-1) ?? -1
          if (parent >= 0) next[parent].hasChildren = true
          const label = String(item.label || '').slice(0, 1024)
          next.push({ id: item.id, label, search: label.toLocaleLowerCase(), depth, parent, hasChildren: false, navigable: item.navigable })
          ancestors.push(next.length - 1)
        }
        if (start + 512 < items.length) { await pause(); if (disposed || current !== itemTicket) return }
      }
      if (disposed || current !== itemTicket) return
      rows = next; visible = []; collapsed.clear(); mounted.clear(); space.replaceChildren(); filter.value = ''
      await refresh()
    },
    active(id) { const changed = active !== id; active = id; render(); if (changed) reveal(id) },
    show() { virtual._willUpdate(); reveal(active); render() },
    dispose() { disposed = true; ++ticket; ++itemTicket; lifetime.abort(); labels.dispose(); unsubscribe?.(); stop(); mounted.clear(); rows = visible = []; space.replaceChildren(); filter.value = ''; feedback.textContent = '' }
  }
}
