// Small shared controls for web and WebView surfaces.
export function favoriteIcon() {
  const namespace = 'http://www.w3.org/2000/svg'
  const icon = document.createElementNS(namespace, 'svg')
  icon.classList.add('notes-favorite-icon')
  icon.setAttribute('viewBox', '0 0 24 24')
  icon.setAttribute('aria-hidden', 'true')
  icon.setAttribute('focusable', 'false')
  const path = document.createElementNS(namespace, 'path')
  // The same five-point star used for favorites, in a centered vector box.
  path.setAttribute('d', 'M12 3 14.78 8.93 21 9.88 16.5 14.5 17.56 21 12 17.93 6.44 21 7.5 14.5 3 9.88 9.22 8.93Z')
  icon.append(path)
  return icon
}

export function dialogClose(close, label = 'Close') {
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'reader-icon-button dialog-close'
  const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  icon.setAttribute('viewBox', '0 0 24 24')
  icon.setAttribute('aria-hidden', 'true')
  icon.setAttribute('focusable', 'false')
  icon.style.cssText = 'width:16px;height:16px;fill:none;stroke:currentColor;stroke-width:1.8;stroke-linecap:round;flex:none'
  const path = document.createElementNS(icon.namespaceURI, 'path')
  path.setAttribute('d', 'M6 6l12 12M18 6 6 18')
  icon.append(path)
  button.append(icon)
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
