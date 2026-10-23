(() => {
  const key = 'marknoria:theme'
  const media = matchMedia('(prefers-color-scheme: dark)')
  let preference = 'system'
  try { preference = localStorage.getItem(key) || 'system' } catch {}
  const isDark = () => preference === 'dark' || preference === 'system' && media.matches
  function apply() {
    if (!['system', 'light', 'dark'].includes(preference)) preference = 'system'
    const dark = isDark()
    document.documentElement.dataset.theme = dark ? 'dark' : 'light'
    document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
    const stylesheet = document.getElementById('syntax-theme')
    if (stylesheet) stylesheet.setAttribute('href', dark ? './syntax-dark.css' : './syntax-light.css')
    const button = document.getElementById('theme-toggle')
    if (button) {
      button.title = `Theme: ${preference}. Switch to ${dark ? 'light' : 'dark'} theme.`
      button.setAttribute('aria-label', `Switch to ${dark ? 'light' : 'dark'} theme`)
      button.setAttribute('aria-pressed', String(dark))
    }
  }
  apply()
  addEventListener('DOMContentLoaded', () => {
    if (!document.getElementById('theme-toggle')) {
      const button = document.createElement('button')
      button.id = 'theme-toggle'
      button.type = 'button'
      button.className = 'reader-icon-button'
      button.innerHTML = '<svg aria-hidden="true" viewBox="0 0 24 24"><path d="M20 14A8 8 0 0 1 10 4a8 8 0 1 0 10 10Z"/></svg>'
      document.querySelector('.reader-header-actions,.compare-actions')?.append(button)
    }
    apply()
    document.getElementById('theme-toggle')?.addEventListener('click', () => {
      preference = isDark() ? 'light' : 'dark'
      try { localStorage.setItem(key, preference) } catch {}
      apply()
    })
  })
  addEventListener('storage', event => { if (event.key === key) { preference = event.newValue || 'system'; apply() } })
  media.addEventListener('change', apply)
})()
