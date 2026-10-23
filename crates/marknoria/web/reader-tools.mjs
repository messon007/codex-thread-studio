export function readerTools(frame, container, currentReader) {
  let dispose = () => {}
  let bound = null

  function clear() {
    dispose()
    dispose = () => {}
    bound = null
    container.replaceChildren()
    container.hidden = true
  }

  return {
    clear,
    bind() {
      const reader = currentReader()
      if (!reader || reader === bound) return
      clear()
      bound = reader
      const child = frame.contentDocument
      const observer = new MutationObserver(sync)
      const entries = []
      for (const id of ['contents-toggle', 'search-toggle']) {
        const original = child.getElementById(id)
        const button = original.cloneNode(true)
        button.removeAttribute('id')
        button.removeAttribute('aria-controls')
        button.dataset.readerTool = id
        button.addEventListener('click', () => original.click())
        observer.observe(original, { attributes: true, attributeFilter: ['disabled', 'aria-expanded'] })
        entries.push([original, button])
        container.append(button)
      }
      const originalSettings = child.querySelector('.reader-settings')
      const settings = originalSettings.cloneNode(true)
      settings.open = false
      for (const element of settings.querySelectorAll('[id]')) {
        const id = element.id
        element.removeAttribute('id')
        if (id === 'column-width') element.classList.add('reader-column-width')
        const original = child.getElementById(id)
        const update = () => {
          original.value = element.value
          original.checked = element.checked
          original.dispatchEvent(new frame.contentWindow.Event('change', { bubbles: true }))
          element.value = original.value
          element.checked = original.checked
        }
        element.value = original.value
        element.checked = original.checked
        element.addEventListener('change', update)
      }
      settings.dataset.readerTool = 'settings'
      container.append(settings)
      observer.observe(originalSettings, { attributes: true, attributeFilter: ['hidden'] })
      function sync() {
        for (const [original, button] of entries) {
          button.disabled = original.disabled
          button.setAttribute('aria-expanded', original.getAttribute('aria-expanded'))
        }
        settings.hidden = originalSettings.hidden
        if (settings.hidden) settings.open = false
      }
      const close = event => {
        if (event.type === 'keydown' ? event.key === 'Escape' : !settings.contains(event.target)) settings.open = false
      }
      for (const document of [container.ownerDocument, child]) {
        document.addEventListener('pointerdown', close)
        document.addEventListener('keydown', close)
      }
      dispose = () => {
        observer.disconnect()
        for (const document of [container.ownerDocument, child]) {
          document.removeEventListener('pointerdown', close)
          document.removeEventListener('keydown', close)
        }
      }
      sync()
      container.hidden = false
    },
  }
}
