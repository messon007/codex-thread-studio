export function fileController(token, options = {}) {
  const dialog = document.querySelector('#file-browser')
  const list = document.querySelector('#file-browser-list')
  const feedback = document.querySelector('#file-feedback')
  let opening = false
  let folder = ''
  const folderKey = `reader-folder:${token}`
  try { folder = sessionStorage.getItem(folderKey) || '' } catch {}
  let browsing = 0
  let request = null
  let selection = null

  async function api(path, options = {}) {
    const response = await fetch(new URL(`.${path}`, import.meta.url), { ...options, headers: { ...options.headers, 'x-reader-token': token } })
    if (!response.ok) throw new Error((await response.text()).slice(0, 800) || `Request failed (${response.status})`)
    return response.json()
  }

  async function openServer(path) {
    if (opening) return
    if (selection) {
      const selected = selection
      selection = null
      dialog.close()
      try { await selected(path) } catch (error) {
        document.querySelector('#file-message').textContent = error.message
        feedback.hidden = false
      }
      return
    }
    if (options.beforeChange && !options.beforeChange()) return
    opening = true
    dialog.setAttribute('aria-busy', 'true')
    for (const button of dialog.querySelectorAll('button')) button.disabled = true
    try {
      await api(`/api/open?path=${encodeURIComponent(path)}`, { method: 'POST' })
      options.leaving?.()
      location.reload()
    } catch (error) {
      opening = false
      dialog.removeAttribute('aria-busy')
      dialog.close()
      for (const button of dialog.querySelectorAll('button')) button.disabled = false
      document.querySelector('#file-message').textContent = error.message
      feedback.hidden = false
    }
  }

  async function browse(path) {
    const ticket = ++browsing
    request?.abort()
    request = new AbortController()
    list.textContent = 'Loading files…'
    try {
      const response = await api(`/api/files?path=${encodeURIComponent(path)}`, { signal: request.signal })
      if (ticket !== browsing) return
      folder = response.path.replaceAll('\\', '/')
      try { sessionStorage.setItem(folderKey, folder) } catch {}
      const location = [response.root, folder].filter(Boolean).join('/')
      const rootName = response.root.split(/[\\/]/u).filter(Boolean).at(-1) || '/'
      document.querySelector('#file-browser-location').textContent = folder ? `${rootName} / ${folder.split('/').join(' / ')}` : response.root
      document.querySelector('#file-browser-location').title = location
      document.querySelector('#file-browser-up').disabled = !folder
      document.querySelector('#file-browser-note').textContent = response.truncated ? 'Showing a limited list. Choose a smaller folder.' : 'Only Markdown files are shown. Browsing stays inside this server folder.'
      list.replaceChildren()
      for (const entry of response.entries) {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'file-entry'
        button.dataset.path = entry.path
        button.dataset.directory = entry.directory
        const icon = document.createElement('span')
        icon.className = 'file-entry-icon'
        icon.innerHTML = entry.directory
          ? '<svg aria-hidden="true" viewBox="0 0 24 24"><path d="M3 7h7l2 2h9v11H3Z M3 7V4h7l2 3"/></svg>'
          : '<svg aria-hidden="true" viewBox="0 0 24 24"><path d="M14 3H6v18h13V8Z M14 3v5h5 M9 12h6 M9 16h6"/></svg>'
        const name = document.createElement('span')
        name.className = 'file-entry-name'
        name.textContent = entry.name
        const size = document.createElement('span')
        size.className = 'file-entry-size'
        size.textContent = entry.directory ? 'Folder' : entry.bytes >= 1048576 ? `${(entry.bytes / 1048576).toFixed(1)} MiB` : `${Math.max(1, Math.ceil(entry.bytes / 1024))} KiB`
        button.append(icon, name, size)
        button.addEventListener('click', () => { if (!opening) entry.directory ? browse(entry.path) : openServer(entry.path) })
        list.append(button)
      }
      if (!response.entries.length) list.textContent = 'No Markdown files or folders here.'
    } catch (error) { if (error.name !== 'AbortError' && ticket === browsing) list.textContent = error.message }
  }

  function chooseServer(selected = null, title = 'Open from server', initialFile = null) {
    if (opening || dialog.open) return
    selection = typeof selected === 'function' ? selected : null
    document.querySelector('.reader-settings')?.removeAttribute('open')
    dialog.querySelector('h2').textContent = title
    dialog.showModal()
    browse(initialFile == null ? folder : initialFile.split('/').slice(0, -1).join('/'))
  }

  const close = () => { if (!opening) dialog.close() }
  dialog.addEventListener('close', () => { if (!dialog.open) { browsing += 1; request?.abort() } })
  dialog.addEventListener('cancel', event => { if (opening) event.preventDefault() })
  for (const button of document.querySelectorAll('[data-open-server]')) button.addEventListener('click', () => chooseServer())
  document.querySelector('#file-browser-close').addEventListener('click', close)
  document.querySelector('#file-browser-cancel').addEventListener('click', close)
  document.querySelector('#file-browser-up').addEventListener('click', () => { if (!opening) browse(folder.split('/').slice(0, -1).join('/')) })
  document.querySelector('#file-dismiss').addEventListener('click', () => { feedback.hidden = true })
  document.addEventListener('keydown', event => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'o') { event.preventDefault(); chooseServer() } })
  return {
    compare() {
      const target = new URL('./compare.html', import.meta.url)
      target.hash = token
      location.assign(target.href)
    },
    choose: chooseServer,
    state(state, error = '') {
      const ready = state === 'ready'
      document.querySelector('#welcome').hidden = ready
      document.querySelector('#viewport').hidden = !ready
      document.querySelector('#welcome-title').textContent = state === 'indexing' ? 'Preparing your document…' : state === 'error' ? 'Could not open this document' : 'Open a Markdown document'
      document.querySelector('#welcome-description').textContent = error || (state === 'indexing' ? 'Building the outline and preparing the first pages. You can choose another file at any time.' : 'Choose a document on the server. Read large files, explore sections, and choose the columns you need.')
      document.querySelector('#document-summary').textContent = state === 'idle' ? 'Choose a file to start reading' : state === 'indexing' ? 'Preparing document…' : state === 'error' ? 'Choose another file to continue' : ''
      for (const id of ['contents-toggle', 'search-toggle']) document.getElementById(id).disabled = !ready
    },
    dispose() { request?.abort() },
  }
}
