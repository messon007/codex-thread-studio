// Selection only; isolated content world, with no message handlers or Studio API.
(() => {
  let range = null
  document.addEventListener('selectionchange', () => {
    const selected = getSelection()
    if (selected?.rangeCount && !selected.isCollapsed) range = selected.getRangeAt(0).cloneRange()
  })
  document.addEventListener('pointerdown', () => { range = null }, true)
  document.addEventListener('keydown', () => { range = null }, true)
  Object.defineProperty(window, 'StudioBrowserSelection', { value: Object.freeze({
    text() {
      const selected = getSelection()
      const current = selected?.rangeCount && !selected.isCollapsed ? selected.getRangeAt(0) : range
      if (!current || current.startContainer.ownerDocument !== document ||
          current.endContainer.ownerDocument !== document || !current.startContainer.isConnected ||
          !current.endContainer.isConnected || current.collapsed) return ''
      const text = current.toString()
      return text.length <= 16384 && new TextEncoder().encode(text).length <= 16384 ? text : null
    },
  }) })
})()
