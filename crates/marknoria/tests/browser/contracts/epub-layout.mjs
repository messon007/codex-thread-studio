export async function epubLayoutContract({ package: packagePath, chapter, diagnose, synthetic }) {
  const { createEpubReader } = await import('/epub-reader.mjs')
  const root = document.createElement('main'); root.style.cssText = 'position:fixed;inset:10px;width:1100px;height:800px'; document.body.append(root)
  const errors = [], positions = []
  const reader = await createEpubReader({ container: root, source: new URL('/book/' + packagePath, location.href).href, resourceRoot: new URL('/book/', location.href).href, onError: e => errors.push(String(e)), onRelocate: p => positions.push(p) })
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
  const until = async (condition, label) => { const end = performance.now() + 10000; while (!condition() && performance.now() < end) await pause(20); if (!condition()) throw Error(label + ': ' + JSON.stringify(errors)) }
  await reader.display(new URL('/book/' + chapter, location.href).href)
  const first = reader.state().cfi
  const seen = new Set(), count = new Set()
  function coverage() {
    const viewport = root.querySelector('.epub-viewer').getBoundingClientRect()
    for (const frame of root.querySelectorAll('iframe')) {
      if (!frame.contentDocument?.querySelector('pre')) continue
      const fr = frame.getBoundingClientRect(), wrapper = frame.parentElement.getBoundingClientRect()
      for (const [id, pre] of [...frame.contentDocument.querySelectorAll('pre')].entries()) {
        const walker = frame.contentDocument.createTreeWalker(pre, NodeFilter.SHOW_TEXT)
        for (let node, nodeIndex = 0; (node = walker.nextNode()); nodeIndex++) {
          let offset = 0
          for (const line of node.textContent.split('\n')) {
            if (line.trim()) {
              const key = frame.contentDocument.querySelector('h1')?.textContent + ':' + id + ':' + nodeIndex + ':' + offset
              count.add(key)
              const range = frame.contentDocument.createRange(); range.setStart(node, offset); range.setEnd(node, offset + line.length)
              if ([...range.getClientRects()].some(r => fr.left + r.left >= viewport.left - 1 && fr.left + r.right <= viewport.right + 1 && fr.left + r.left >= wrapper.left - 1 && fr.left + r.right <= wrapper.right + 1 && fr.top + r.top >= viewport.top - 1 && fr.top + r.bottom <= viewport.bottom + 1)) seen.add(key)
            }
            offset += line.length + 1
          }
        }
      }
    }
  }
  const geometry = () => [...root.querySelectorAll('iframe')].map(frame => ({ frame: { width: frame.clientWidth, height: frame.clientHeight }, wrapper: { width: frame.parentElement.clientWidth, height: frame.parentElement.clientHeight }, pre: [...frame.contentDocument.querySelectorAll('pre')].map(pre => ({ height: pre.getBoundingClientRect().height, width: pre.getBoundingClientRect().width, breakInside: frame.contentWindow.getComputedStyle(pre).breakInside, overflow: frame.contentWindow.getComputedStyle(pre).overflow })) }))
  const initialGeometry = geometry()
  for (let n = 0; n < 80; n++) {
    coverage()
    const before = reader.state().cfi
    root.querySelector('.epub-next').click()
    await until(() => reader.state().cfi !== before, 'next page did not relocate')
    if (!reader.state().href.endsWith(chapter.split('/').at(-1))) break
  }
  const paginated = { total: count.size, seen: seen.size, missing: [...count].filter(key => !seen.has(key)).slice(0, 8), initialGeometry }
  if (!diagnose && paginated.seen !== paginated.total) throw Error('Paginated code lines skipped: ' + JSON.stringify(paginated))
  await reader.display(first)
  const toggle = root.querySelector('.epub-continuous')
  if (!toggle || toggle.closest('.epub-settings-popover')) throw Error('Continuous reading is not exposed in the toolbar')
  const oldFrame = root.querySelector('iframe')
  toggle.click()
  await until(() => root.querySelector('iframe') !== oldFrame && reader.state().flow === 'scrolled' && !toggle.disabled && root.querySelector('.epub-container')?.scrollHeight > root.querySelector('.epub-container')?.clientHeight, 'continuous flow did not finish mounting')
  const scrolledGeometry = geometry()
  let continuous = null
  if (synthetic) {
    seen.clear(); count.clear()
    let maxFrames = 0, maxElements = 0, maxStepMs = 0, reachedEnd = false, endGeometry = null
    for (let step = 0; step < 160; step++) {
      const started = performance.now()
      coverage()
      const frames = [...root.querySelectorAll('iframe')]
      maxFrames = Math.max(maxFrames, frames.length)
      maxElements = Math.max(maxElements, ...frames.map(frame => frame.contentDocument?.querySelectorAll('*').length || 0))
      const view = root.querySelector('.epub-viewer').getBoundingClientRect()
      reachedEnd = frames.some(frame => {
        const tail = frame.contentDocument?.getElementById('after')
        if (tail?.textContent !== 'AFTER_LISTING_7') return false
        const rect = tail.getBoundingClientRect(), outer = frame.getBoundingClientRect()
        endGeometry = { top: outer.top + rect.top, bottom: outer.top + rect.bottom, viewTop: view.top, viewBottom: view.bottom, scrollTop: root.querySelector('.epub-container').scrollTop, scrollHeight: root.querySelector('.epub-container').scrollHeight, clientHeight: root.querySelector('.epub-container').clientHeight }
        // epub.js rounds iframe height to integer CSS pixels; WebKit's last
        // paragraph box can extend a fraction of a pixel beyond that boundary.
        return outer.top + rect.top >= view.top - 1 && outer.top + rect.bottom <= view.bottom + 1
      })
      if (reachedEnd) break
      const scroller = root.querySelector('.epub-container')
      scroller.scrollTop += scroller.clientHeight * 0.65
      await pause(80)
      maxStepMs = Math.max(maxStepMs, performance.now() - started)
    }
    continuous = { reachedEnd, lines: count.size, seen: seen.size, maxFrames, maxElements, maxStepMs, endGeometry }
    if (!reachedEnd || seen.size !== 8 * 103 || maxFrames > 4) throw Error('Continuous spine coverage failed: ' + JSON.stringify(continuous))
    const needle = 'auto line_050 = value_050; // chapter 7 中文😀'
    const match = (await reader.search(needle)).matches[0]
    if (!match || await reader.quoteForCfi(match.cfi) !== needle) throw Error('Original code CFI quote was not exact')
    const saved = reader.state()
    root.style.width = '760px'; root.style.height = '620px'
    window.dispatchEvent(new Event('resize'))
    await until(() => [...root.querySelectorAll('iframe')].every(frame => frame.clientWidth <= 760), 'resized EPUB keeps its old width')
    await reader.settings({ theme: 'sepia', fontScale: 1.25, flow: 'paginated' })
    await reader.display(match.cfi)
    if (await reader.quoteForCfi(match.cfi) !== needle) throw Error('Flow/type/resize changed the original CFI tree')
    await reader.settings({ flow: 'scrolled' })
    await reader.display(saved.cfi)
    if (reader.state().flow !== 'scrolled' || !reader.state().href.endsWith('listing-7.xhtml')) throw Error('Flow switch lost the original chapter')
  }
  reader.destroy()
  return { paginated, scrolledGeometry, continuous, errors, positions: positions.length }
}
