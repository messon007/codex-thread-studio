// Trusted DOM-dependent layout/CFI work; runs only in an isolated content world.
(() => {
  'use strict'
  const CFI = StudioEpubCFI.CFI
  let config = null, revision = 0, timer = null, readerStyle = null, pendingRestore = null, lastCfi = ''
  let selectedRange = null
  const post = value => window.webkit.messageHandlers.reader.postMessage(JSON.stringify({ ...value, href: location.href }))
  const root = () => document.body || document.documentElement
  const paged = () => config?.flow === 'paginated'
  const horizontalDirection = () => getComputedStyle(root()).direction === 'rtl' ? -1 : 1
  const fraction = () => {
    const element = document.scrollingElement || document.documentElement
    const size = paged() ? element.scrollWidth - innerWidth : element.scrollHeight - innerHeight
    return size <= 0 ? 0 : Math.max(0, Math.min(1, (paged() ? Math.abs(scrollX) : scrollY) / size))
  }
  function position() {
    if (!config?.base || document.hidden) return null
    let range = document.caretRangeFromPoint?.(horizontalDirection() < 0 ? innerWidth - 44 : 44, 36)
    if (!range || !root().contains(range.startContainer)) {
      const first = document.createTreeWalker(root(), NodeFilter.SHOW_TEXT).nextNode()
      if (!first) return null
      range = document.createRange(); range.setStart(first, 0); range.collapse(true)
    }
    try { return new CFI(range, config.base).toString() } catch { return null }
  }
  function publish() {
    const cfi = position()
    if (cfi && cfi.length <= 8192) { lastCfi = cfi; post({ kind: 'position', cfi, fraction: fraction() }) }
  }
  function restore(cfi, fragment = '', end = false) {
    if (document.hidden) { pendingRestore = { cfi, fragment, end }; return }
    pendingRestore = null
    try {
      let target = null
      if (cfi) target = new CFI(cfi).toRange(document)
      if (config?.quote) {
        const matched = !!target && target.toString().trim() === config.quote.trim()
        if (!matched) target = null
        post({ kind: 'anchor', matched })
        if (!matched) config.quote = ''
      }
      if (target) {
        if (config?.quote && !target.collapsed) {
          const selected = getSelection(); selected.removeAllRanges(); selected.addRange(target)
          selectedRange = target.cloneRange()
        }
        config.quote = ''
        const rect = target.getBoundingClientRect()
        scrollTo(paged() ? Math.floor((rect.left + scrollX) / innerWidth) * innerWidth : 0,
          paged() ? 0 : Math.max(0, rect.top + scrollY - 32))
      } else if (fragment) document.getElementById(decodeURIComponent(fragment))?.scrollIntoView()
      else if (end) scrollTo(paged() ? horizontalDirection() * document.documentElement.scrollWidth : 0, paged() ? 0 : document.documentElement.scrollHeight)
      else scrollTo(0, 0)
    } catch { if (config?.quote) post({ kind: 'anchor', matched: false }); scrollTo(0, 0) }
    publish()
  }
  function styles() {
    // Keep our own node reference: book IDs must never be treated as app nodes.
    if (!readerStyle) {
      readerStyle = document.createElementNS(document.documentElement.namespaceURI, 'style')
      ;(document.head || document.documentElement).append(readerStyle)
    }
    const colors = config.theme === 'dark' ? ['#1e1e1e', '#eeeeec', '#83cbbf'] : config.theme === 'sepia' ? ['#f4ecd8', '#453b2d', '#28766c'] : ['#fffdfa', '#263234', '#137c70']
    readerStyle.textContent = `html,body{background:${colors[0]}!important;color:${colors[1]}!important}body{font-size:${18 * config.fontScale}px!important;line-height:1.7!important}a{color:${colors[2]}!important}img,svg{max-width:100%;height:auto}pre{white-space:pre-wrap;overflow-wrap:anywhere}` +
      (config.fixed ? '' : paged() ? 'html{overflow:hidden!important}body{box-sizing:border-box!important;margin:24px 40px!important;width:calc(100vw - 80px)!important;max-width:none!important;height:calc(100vh - 48px)!important;column-width:calc(100vw - 80px)!important;column-gap:80px!important;column-fill:auto!important}' : 'html{overflow:auto!important}body{box-sizing:border-box!important;margin:24px auto!important;padding:0 28px!important;max-width:960px!important;height:auto!important;column-width:auto!important}')
  }
  // EPUB.js encodes element child offsets as character offsets. Normalize
  // element boundaries (Select All and programmatic selections) to text first.
  function textBoundary(node, offset, end) {
    if (node.nodeType === Node.TEXT_NODE) return [node, offset]
    const adjacent = value => {
      while (value && value !== root()) {
        const sibling = end ? value.previousSibling : value.nextSibling
        if (sibling) return sibling
        value = value.parentNode
      }
      return null
    }
    let current = node.childNodes[end ? offset - 1 : offset] || adjacent(node)
    for (let visits = 0; current && visits < 4096; visits++) {
      if (!root().contains(current)) return null
      if (current.nodeType === Node.TEXT_NODE) return [current, end ? current.length : 0]
      current = (end ? current.lastChild : current.firstChild) || adjacent(current)
    }
    return null
  }
  const violations = []
  document.addEventListener('securitypolicyviolation', event => { if (violations.length < 16) violations.push({ directive: event.violatedDirective, uri: event.blockedURI }) })
  window.StudioReader = {
    violations,
    init(value) {
      document.querySelectorAll('script,iframe,frame,object,embed,form,input,button,textarea,select,meta[http-equiv="refresh" i]').forEach(node => node.remove())
      config = value
      const token = ++revision
      styles()
      Promise.race([document.fonts?.ready || Promise.resolve(), new Promise(resolve => setTimeout(resolve, 1500))]).then(() => {
        let restored = false
        const finish = () => {
          if (token === revision && !restored) { restored = true; restore(value.cfi, value.fragment, value.end); post({ kind: 'laidOut' }) }
        }
        requestAnimationFrame(() => requestAnimationFrame(finish))
        setTimeout(finish, 250) // Covered Wayland windows can suspend frame callbacks.
      })
    },
    configure(value) { const cfi = position(); this.init({ ...config, ...value, cfi, fragment: '', end: false }) },
    step(direction) {
      const before = fraction()
      if (direction > 0 && before >= 0.999 || direction < 0 && before <= 0.001) { publish(); post({ kind: 'edge', direction }); return }
      scrollBy(paged() ? direction * horizontalDirection() * innerWidth : 0, paged() ? 0 : direction * innerHeight * 0.85)
      publish()
    },
    anchor(fragment) { restore('', fragment) },
    publish,
    snapshot() {
      const cfi = position()
      return cfi ? JSON.stringify({ kind: 'position', href: location.href, cfi, fraction: fraction() }) : ''
    },
    commentSelection() {
      const selected = getSelection()
      const range = selected?.rangeCount && !selected.isCollapsed ? selected.getRangeAt(0) : selectedRange
      if (!range || range.collapsed || range.startContainer.ownerDocument !== document ||
          !root().contains(range.startContainer) || !root().contains(range.endContainer)) return ''
      const excerpt = range.toString()
      if (!excerpt.trim() || excerpt.length > 16384 || new TextEncoder().encode(excerpt).length > 16384) return ''
      let cfiRange = ''
      try {
        if (config?.base) {
          const start = textBoundary(range.startContainer, range.startOffset, false)
          const end = textBoundary(range.endContainer, range.endOffset, true)
          if (!start || !end) return ''
          const normalized = document.createRange()
          normalized.setStart(...start); normalized.setEnd(...end)
          if (normalized.toString() !== excerpt) return ''
          cfiRange = new CFI(normalized, config.base).toString()
          if (new CFI(cfiRange).toRange(document)?.toString() !== excerpt) return ''
        }
      } catch { return '' }
      if (cfiRange.length > 8192) return ''
      return JSON.stringify({ excerpt, cfiRange, href: location.href })
    },
    copySelection() {
      const selected = getSelection()
      const text = selected && !selected.isCollapsed ? selected.toString() : selectedRange?.toString() || ''
      return text.length <= 2097152 ? text : null
    },
    translationSelection() {
      const selected = getSelection()
      const text = selected && !selected.isCollapsed ? selected.toString() : selectedRange?.toString() || ''
      return text.length <= 32000 && new TextEncoder().encode(text).length <= 64000 ? text : null
    },
    clearSelection() { selectedRange = null },
  }
  // Native toolbar focus can collapse WebKit's live selection. Retain its range
  // until the next interaction in this document, without copying the text.
  document.addEventListener('selectionchange', () => {
    const selected = getSelection()
    if (selected?.rangeCount && !selected.isCollapsed) selectedRange = selected.getRangeAt(0).cloneRange()
  })
  document.addEventListener('pointerdown', () => { selectedRange = null }, true)
  document.addEventListener('keydown', () => { selectedRange = null }, true)
  window.addEventListener('scroll', () => { clearTimeout(timer); timer = setTimeout(publish, 180) }, { passive: true })
  window.addEventListener('resize', () => { clearTimeout(timer); timer = setTimeout(() => lastCfi ? restore(lastCfi) : publish(), 180) })
  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) {
      if (pendingRestore) restore(pendingRestore.cfi, pendingRestore.fragment, pendingRestore.end)
      else if (lastCfi) restore(lastCfi)
      else publish()
    }
  })
  post({ kind: 'ready' })
})()
