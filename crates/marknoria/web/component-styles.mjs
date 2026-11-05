// One owned stylesheet per URL/document, shared across component instances.
const documents = new WeakMap()
export function componentStyles(document, url) {
  let sheets = documents.get(document)
  if (!sheets) {
    sheets = new Map()
    documents.set(document, sheets)
  }
  const key = new URL(String(url), document.baseURI).href
  let entry = sheets.get(key)
  if (!entry) {
    const existing = [...document.querySelectorAll('link[rel="stylesheet"]')].find(
      (link) => link.href === key && !link.disabled && !link.media
    )
    const link = existing || document.createElement('link')
    if (!existing) {
      link.rel = 'stylesheet'
      link.href = key
      document.head.append(link)
    }
    entry = { link, count: 0, owned: !existing }
    sheets.set(key, entry)
  }
  entry.count++
  let released = false
  return () => {
    if (released) return
    released = true
    if (--entry.count === 0) {
      if (entry.owned) entry.link.remove()
      sheets.delete(key)
      if (!sheets.size) documents.delete(document)
    }
  }
}
