// One dismissal control for all web and WebView dialogs.
export function dialogClose(close, label = 'Close') {
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'reader-icon-button dialog-close'
  button.textContent = '×'
  button.setAttribute('aria-label', label)
  button.title = label
  button.addEventListener('click', close)
  return button
}

export function pagerIcon(direction) {
  const namespace = 'http://www.w3.org/2000/svg'
  const icon = document.createElementNS(namespace, 'svg')
  icon.classList.add('pager-icon')
  icon.setAttribute('viewBox', '0 0 24 24')
  icon.setAttribute('aria-hidden', 'true')
  icon.setAttribute('focusable', 'false')
  const path = document.createElementNS(namespace, 'path')
  path.setAttribute('d', direction === 'previous' ? 'M15 6l-6 6 6 6' : 'M9 6l6 6-6 6')
  icon.append(path)
  return icon
}
