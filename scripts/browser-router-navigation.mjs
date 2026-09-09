import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

// Real workspace CSS and navigation implementation, with no backend messages.
export async function checkRouterNavigation(page) {
  if (process.env.STUDIO_SCREENSHOT_DIR) {
    await mkdir(process.env.STUDIO_SCREENSHOT_DIR, { recursive: true })
    await page.exposeFunction('captureRouterNavigation', () => page.screenshot({ path: join(process.env.STUDIO_SCREENSHOT_DIR, 'router-navigation.png') }))
  }
  return page.evaluate(async () => {
    const style = document.createElement('style')
    style.textContent = await (await fetch('/styles.css')).text()
    document.head.append(style)
    const host = document.createElement('div')
    host.className = 'app-shell'
    host.style.cssText = 'position:fixed;inset:0;z-index:10000;height:100%;background:white'
    host.innerHTML = '<main class="workspace"><header class="thread-toolbar">Router navigation test</header><section class="native-workspace"><div id="navigation-transcript" class="transcript"></div><form class="composer"><textarea placeholder="Message"></textarea><button type="button">Send</button></form><div style="position:absolute;top:calc(100% + 600px);height:20px">Overflow stress fixture</div></section></main>'
    document.body.append(host)
    const transcript = host.querySelector('.transcript'), workspace = host.querySelector('.native-workspace'), composer = host.querySelector('.composer')
    const source = await (await fetch('/app.js')).text()
    const start = source.indexOf('function navigateTranscriptTurn('), end = source.indexOf('\nfunction ', start + 1)
    let captures = 0, shows = 0
    const render = () => { transcript.innerHTML = '<div style="height:1800px">History</div><section class="turn" data-turn-id="old-turn">Target response</section>' }
    const navigate = new Function('$', 'presentationThreadKey', 'transcriptPresentationCache', 'transcriptScrollFollower', 'state', 'renderTranscript', 'followTranscriptOutput', 'setActiveTurnNavigator', 'transcriptCaptureSuppressedKeys', 'captureTranscriptViewState', `let pendingTranscriptViewRestore={key:'codex:rico'};${source.slice(start, end)};return navigateTranscriptTurn`)(
      () => transcript, () => 'codex:rico', { setScrollState() {}, showTurn() { shows++ } }, { pause() {}, reset() {} }, { model: {} }, render, () => {}, () => {}, new Set(), () => { captures++ },
    )
    const check = (ok, message) => { if (!ok) throw Error(message) }
    try {
      const bottom = composer.getBoundingClientRect().bottom
      navigate('old-turn')
      check(shows === 1 && captures === 1 && transcript.scrollTop > 0, 'Unrendered target Turn was not revealed and saved')
      check(workspace.scrollTop === 0 && composer.getBoundingClientRect().bottom === bottom, 'Open response moved the composer')
      // A second session reuses the containers; it must retain the same geometry.
      transcript.innerHTML = '<section class="turn">Router response</section>'
      check(workspace.scrollTop === 0 && composer.getBoundingClientRect().bottom === bottom, 'Returning to Router changed layout')
      // clip must also prevent unrelated focus/scrollIntoView from moving this ancestor.
      workspace.scrollTop = 500
      check(workspace.scrollTop === 0, 'Outer workspace remains programmatically scrollable')
      if (window.captureRouterNavigation) await window.captureRouterNavigation()
      return 'Router navigation: virtual Turn reveal, scoped scroll, return layout and outer overflow passed'
    } finally { host.remove(); style.remove() }
  })
}
