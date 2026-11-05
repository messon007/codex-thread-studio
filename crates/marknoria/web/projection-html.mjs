import DOMPurify from './vendor/purify.mjs'

// Both ordinary hosts and the virtual reader mount the same inert projection.
// This accepts trusted Rust projection metadata, never arbitrary host identities.
export function projectionHtml(html, document = globalThis.document) {
  if (typeof html !== 'string' || new TextEncoder().encode(html).length > 131072)
    throw Error('Projection HTML exceeds its 128 KiB budget')
  const fragment = DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['img', 'video', 'audio', 'iframe', 'form', 'style'],
    RETURN_DOM_FRAGMENT: true
  })
  const walker = document.createTreeWalker(fragment, NodeFilter.SHOW_ALL)
  let count = 0
  while (walker.nextNode()) if (++count > 16384) throw Error('Projection exceeds its DOM budget')
  for (const link of fragment.querySelectorAll('a')) {
    link.target = '_blank'
    link.rel = 'noopener noreferrer'
  }
  for (const row of fragment.querySelectorAll('tr[data-source-line]'))
    row.title = `Source line ${row.dataset.sourceLine}`
  return fragment
}
