const frame = () => new Promise(requestAnimationFrame)
const delay = () => new Promise(resolve => setTimeout(resolve, 0))

export async function measure(config) {
  const { renderMarkdownHtml } = await import('/actual-render.mjs')
  const host = document.querySelector('main')
  const outline = document.querySelector('aside')
  const unsafe = document.createElement('div')
  unsafe.innerHTML = renderMarkdownHtml('', { dirtyHtml: '<script>alert(1)</script><iframe src="https://example.com"></iframe><img src=x onerror="alert(1)"><a href="javascript:alert(1)" onclick="alert(1)">unsafe</a>', documentImages: true })
  if (unsafe.querySelector('script,iframe,[onerror],[onclick],a[href^="javascript:"]')) throw Error('Unsafe fixture survived sanitization')
  const samples = []
  let headings = []
  for (const mode of ['whole-window', 'yield-between-pages']) {
    for (let round = 0; round < config.rounds; round++) {
      for (const name of ['initial', 'jump-1', 'jump-2', 'jump-3']) {
        host.replaceChildren()
        outline.replaceChildren()
        await frame()
        let previous = performance.now()
        let maxGap = 0
        const heartbeat = setInterval(() => {
          const now = performance.now()
          maxGap = Math.max(maxGap, now - previous)
          previous = now
          document.querySelector('input').value = String(now)
        }, 10)
        const start = performance.now()
        const response = await fetch(`/payload/${name}.json`)
        const payload = await response.json()
        const transferDecodeMs = performance.now() - start
        if (!payload.readOnly || payload.pages.length > 5) throw Error('Invalid context window')
        if (name === 'initial') headings = payload.toc
        globalThis.pagingFixtureState = { toc: headings, pages: payload.pages }
        const selected = name === 'jump-1' ? headings[Math.floor(headings.length / 2)] : null
        if (selected && selected.page !== payload.targetPage) throw Error('TOC page mapping differs')
        let sanitizeMs = 0
        let mountMs = 0
        for (const page of payload.pages) {
          const sanitizeStart = performance.now()
          const html = renderMarkdownHtml('', { dirtyHtml: page.html, documentImages: true })
          sanitizeMs += performance.now() - sanitizeStart
          const mountStart = performance.now()
          const wrapper = document.createElement('section')
          wrapper.dataset.page = String(page.id)
          wrapper.innerHTML = html
          host.append(wrapper)
          mountMs += performance.now() - mountStart
          if (mode === 'yield-between-pages') { await frame(); await delay() }
        }
        const tocStart = performance.now()
        const selectedIndex = selected?.id || 0
        for (const heading of headings.slice(Math.max(0, selectedIndex - 10), Math.max(0, selectedIndex - 10) + 50)) {
          const item = document.createElement('button')
          item.textContent = heading.label
          item.dataset.page = String(heading.page)
          outline.append(item)
        }
        const tocMountMs = performance.now() - tocStart
        if (selected) {
          const target = [...host.querySelectorAll('h1,h2,h3,h4,h5,h6')].find(node => Number(node.getAttribute('data-sourcepos')?.split(':')[0]) === selected.line)
          if (!target) throw Error('Target heading lost its original source position')
          host.scrollTop = target.offsetTop - host.offsetTop
        } else host.scrollTop = 0
        const layoutStart = performance.now()
        host.getBoundingClientRect()
        host.scrollHeight
        const layoutMs = performance.now() - layoutStart
        await frame()
        await frame()
        await delay()
        maxGap = Math.max(maxGap, performance.now() - previous)
        clearInterval(heartbeat)
        const readyMs = performance.now() - start
        if (host.querySelector('script,iframe,object,embed,[onerror],[onclick],a[href^="javascript:"]')) throw Error('Unsafe HTML survived sanitization')
        for (const table of host.querySelectorAll('table')) {
          if (table.querySelectorAll(':scope > thead').length !== 1) throw Error('Table header lost')
        }
        samples.push({ name, mode, round, readyMs, transferDecodeMs, sanitizeMs, mountMs, layoutMs, tocMountMs, maxMainThreadGapMs: maxGap, pages: payload.pages.length, tocCount: headings.length, tocDomCount: outline.childElementCount, nodes: host.querySelectorAll('*').length, tables: host.querySelectorAll('table').length, htmlBytes: payload.pages.reduce((sum, page) => sum + new TextEncoder().encode(page.html).length, 0) })
      }
    }
  }
  return { userAgent: navigator.userAgent, samples, sourceAnchorsChecked: true, unsafeFixtureChecked: true, readOnly: true, diagramsRendered: false }
}
