(() => {
  const report = (value) => {
    const token = window.__CODEX_THREAD_STUDIO_GATEWAY__?.token
    if (!token) return
    fetch('/studio/client-log', {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'text/plain' },
      body: String(value || 'Studio bootstrap error').slice(0, 16 * 1024),
    }).catch(() => {})
  }
  window.addEventListener('error', (event) => {
    const location = event.filename ? `\n${event.filename}:${event.lineno || 0}:${event.colno || 0}` : ''
    report(`${event.error?.stack || event.message || 'Studio bootstrap error'}${location}`)
  })
  window.addEventListener('unhandledrejection', (event) => report(event.reason?.stack || event.reason))
})()
