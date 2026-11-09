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
      const settingsControls = []
      for (const element of settings.querySelectorAll('[id]')) {
        const id = element.id
        element.removeAttribute('id')
        if (id === 'column-width') element.classList.add('reader-column-width')
        const original = child.getElementById(id)
        if (element.hasAttribute('aria-describedby')) element.removeAttribute('aria-describedby')
        if (element.tagName === 'BUTTON') {
          element.addEventListener('click', () => { settings.open = false; original.click() })
          settingsControls.push([original, element])
          continue
        }
        if (!element.matches('input,select,textarea')) continue
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
        settingsControls.push([original, element])
      }
      settings.dataset.readerTool = 'settings'
      container.append(settings)
      observer.observe(originalSettings, { attributes: true, attributeFilter: ['hidden'] })
      // Synchronize before opening. The asynchronous toggle event may arrive
      // after typing has begun and overwrite the user's uncommitted input.
      settings.querySelector('summary').addEventListener('click', () => { if (!settings.open) sync() })
      function sync() {
        for (const [original, button] of entries) {
          button.disabled = original.disabled
          button.setAttribute('aria-expanded', original.getAttribute('aria-expanded'))
        }
        settings.hidden = originalSettings.hidden
        for (const [original, control] of settingsControls) {
          control.disabled = original.disabled
          control.value = original.value
          control.checked = original.checked
        }
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
