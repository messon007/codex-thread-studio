// Fixed reader chrome only. Never scan rendered pages, values or source text.
import { uiLabels } from './translation.mjs'
export function readerUiLabels(root, translate, options) {
  const labels = uiLabels(translate, options)
  const textSelectors = [
    '#preview-mode',
    '#source-mode',
    '.reader-header-actions > [data-open-server] > span',
    '#compare-files > span',
    '.reader-settings-panel > strong',
    '#search > button:not([type])',
    '#edit-go > button',
    '#edit-undo',
    '#edit-redo',
    '#edit-save',
    '#edit-reload',
    '#edit-keep-open',
    '#edit-discard-close',
    '#source-navigation > button',
    '#file-browser-cancel',
    '.reader-diagnostics > summary',
    '.reader-empty-actions > button',
    '.file-browser-heading p',
    '#edit-close-dialog > p'
  ]
  for (const selector of textSelectors)
    for (const node of root.querySelectorAll(selector)) labels.set(node, node.textContent.trim())
  // Only known static labels contain an input/count after their leading text.
  for (const selector of [
    '.reader-settings-panel > label',
    '#edit-go > label',
    '#source-navigation > label',
    '.reader-outline-heading h2'
  ])
    for (const node of root.querySelectorAll(selector)) {
      const text = [...node.childNodes].find(
        (child) => child.nodeType === Node.TEXT_NODE && child.textContent.trim()
      )
      if (!text) continue
      const span = document.createElement('span'),
        key = text.textContent.trim()
      text.replaceWith(span, document.createTextNode(' '))
      labels.set(span, key)
    }
  const attributeSelectors = [
    '#compare-files',
    '.reader-header-actions > [data-open-server]',
    '#view-modes',
    '.reader-document-tools',
    '#contents-toggle',
    '#search-toggle',
    '.reader-settings > summary',
    '#query',
    '#search-close',
    '#contents-backdrop',
    '#contents-close',
    '#toc',
    '#edit-line',
    '#edit-undo',
    '#edit-redo',
    '#edit-previous',
    '#edit-next',
    '#edit-reload',
    '#source-line',
    '#source-previous',
    '#source-next',
    '#matches',
    '#file-dismiss',
    '#file-browser-close',
    '#file-browser-up',
    '#file-browser-list'
  ]
  for (const selector of attributeSelectors)
    for (const node of root.querySelectorAll(selector))
      for (const attribute of ['aria-label', 'title', 'placeholder']) {
        const key = node.getAttribute(attribute)
        if (key) labels.set(node, key, {}, attribute)
      }
  return labels
}
