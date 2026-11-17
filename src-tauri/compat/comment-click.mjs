  // Temporary MarkNoria #39: activate exact painted anchors without wrapping text.
  // Delay single-click dispatch so native double-click/drag selection can win.
  let activation = null, pressed = null
  const hasSelection = realm => Boolean(realm.getSelection?.()?.toString() || document.getSelection()?.toString())
  const interactive = event => event.composedPath().some(node => node.nodeType === 1 && node.matches('a,button,input,textarea,select,summary,[role="button"],[role="link"],[contenteditable]:not([contenteditable="false"])'))
  const plainClick = event => event.button === 0 && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey
  function marksAt(event, frame) {
    const box = frame?.getBoundingClientRect(), x = event.clientX + (box?.left || 0), y = event.clientY + (box?.top || 0)
    return [...new Set(hitboxes.filter(box => x >= box.left && x <= box.right && y >= box.top && y <= box.bottom).map(box => box.id))]
  }
  function pressMark(event, frame) {
    if (popup.contains(event.target)) return
    clearHover()
    if (plainClick(event) && !interactive(event)) pressed = { realm: event.currentTarget, x: event.clientX, y: event.clientY, ids: marksAt(event, frame) }
  }
  function clickMark(event, frame) {
    const start = pressed; pressed = null
    if (!start || event.defaultPrevented || !plainClick(event) || event.detail !== 1 || interactive(event)
      || start.realm !== event.currentTarget || hasSelection(start.realm)
      || Math.hypot(event.clientX - start.x, event.clientY - start.y) > 5) return
    const ids = marksAt(event, frame).filter(id => start.ids.includes(id))
    if (!ids.length) return
    clearHover()
    if (ids.length > 1) { pointerMove(event, frame); return }
    activation = setTimeout(() => {
      if (disposed || !hitboxes.some(box => box.id === ids[0]) || hasSelection(start.realm) || !records.some(note => note.id === ids[0])) return
      clearHover(); open(ids[0])
    }, 400)
  }
