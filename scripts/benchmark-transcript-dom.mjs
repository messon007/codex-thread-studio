import { readFileSync } from 'node:fs'
// Optional developer benchmark; no browser dependency is added to the app.
// STUDIO_PLAYWRIGHT_MODULE: absolute path to an installed Playwright index.mjs.
// STUDIO_CHROMIUM_PATH: optional browser executable override.
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const source = readFileSync(new URL('../ui/transcript-dom.mjs', import.meta.url), 'utf8')
const browser = await chromium.launch({ headless: true, executablePath:process.env.STUDIO_CHROMIUM_PATH })
try {
  const page = await browser.newPage()
  await page.setContent('<style>#transcript {height:400px;overflow:auto} section {min-height:150px}</style><div id="transcript"></div>')
  const report = await page.evaluate(async (url) => {
    const { createTranscriptDom } = await import(url)
    const container = document.getElementById('transcript')
    const dom = createTranscriptDom()
    const chunks = Array.from({ length:30 }, (_, i) => ({ id:String(i), html:`<section id="turn-${i}"><h3>Turn ${i}</h3>${'<p>Response paragraph with <strong>formatted content</strong> and a link.</p>'.repeat(40)}${i === 5 ? '<input value="initial"><details><summary>Activity</summary>Output</details>' : ''}</section>` }))
    dom.render(container, 'codex:a', chunks)
    const input = container.querySelector('input')
    const details = container.querySelector('details')
    input.value = 'unsent approval answer'
    details.open = true
    input.focus()
    container.scrollTop = 8500
    const top = container.scrollTop
    const original = document.getElementById('turn-5')
    chunks[0] = { ...chunks[0], html:chunks[0].html.replace('Turn 0', 'Changed') }
    const result = dom.render(container, 'codex:a', chunks)
    const check = (condition, message) => { if (!condition) throw Error(message) }
    check(document.getElementById('turn-5') === original, 'unchanged Turn replaced')
    check(document.activeElement === input && input.value === 'unsent approval answer', 'focus/input lost')
    check(details.open, 'details collapsed')
    check(container.scrollTop === top, 'scroll jumped')
    check(result.reusedTurns === 29, 'unexpected reuse count')
    const measure = (fn) => {
      const times = []
      for (let i=0;i<50;i++) { const start=performance.now();fn(i);void container.offsetHeight;times.push(performance.now()-start) }
      times.sort((a,b)=>a-b)
      return {medianMs:times[25], p95Ms:times[47]}
    }
    const full = measure(i => { container.innerHTML = chunks.map(c => c.html).join('') + `<span>${i}</span>` })
    dom.render(container, 'codex:a', chunks)
    const incremental = measure(i => dom.render(container, 'codex:a', [...chunks, { id:'__footer', html:`<span>${i}</span>` }]))
    dom.render(container, 'ept-codex:a', chunks)
    check(document.getElementById('turn-5') !== original, 'cross-backend DOM retained')
    return { checks:'PASS: node identity, focus, unsent input, expanded details, scroll, backend isolation', syntheticBenchmark:{turns:30,paragraphsPerTurn:40,iterations:50,full,incremental} }
  }, `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)
  console.log(JSON.stringify(report, null, 2))
} finally { await browser.close() }
