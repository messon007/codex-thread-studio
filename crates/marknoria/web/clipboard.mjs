// Shared WebView/browser clipboard adapter. The caller owns any UI feedback.
export async function copyText(text) {
  try {
    if (!navigator.clipboard?.writeText) throw Error('Clipboard API unavailable')
    await navigator.clipboard.writeText(text)
  } catch {
    const previous = document.activeElement, field = document.createElement('textarea')
    field.value = text; field.style.cssText = 'position:fixed;opacity:0;pointer-events:none'; (previous?.closest('dialog[open]') || document.body).append(field)
    try { field.select(); if (!document.execCommand('copy')) throw Error('Clipboard is unavailable') }
    finally { field.remove(); if (previous?.isConnected) previous.focus({ preventScroll: true }) }
  }
}
