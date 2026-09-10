import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

export async function checkRouterAttention(page) {
  await page.bringToFront()
  if (process.env.STUDIO_SCREENSHOT_DIR) {
    await mkdir(process.env.STUDIO_SCREENSHOT_DIR, { recursive: true })
    await page.exposeFunction('captureRouterAttention', async () => {
      await page.locator('#router-attention-acceptance').screenshot({ path: join(process.env.STUDIO_SCREENSHOT_DIR, 'router-attention.png') })
    })
  }
  const result = await page.evaluate(async () => {
    const { routerAttentionEntries, responseIsVisible } = await import('/router-attention.mjs')
    const source = await (await fetch('/thread-router-controller.mjs')).text()
    const start = source.indexOf('  const returnPositions =')
    const end = source.indexOf('  function bind()', start)
    const bindStart = source.indexOf("    element('transcript')?.addEventListener('scroll'", end)
    const bindEnd = source.indexOf("    element('router-settings-action')", bindStart)
    const host = document.createElement('section')
    host.id = 'router-attention-acceptance'
    host.style.cssText = 'position:fixed;inset:30px;z-index:10000;background:var(--panel);padding:20px'
    host.innerHTML = '<div id="transcript" style="height:240px;overflow:auto;position:relative"><div style="height:600px">Current reading position</div><section class="router-turn" data-turn-id="r1"><div class="router-target-response"><div class="message agent"><div class="markdown-body"><p style="height:80px">Worker one completed response</p></div></div></div></section><div style="height:300px"></div></div><div id="router-attention" class="router-attention"></div>'
    document.body.append(host)
    const element = id => host.querySelector(`#${id}`)
    const transcript = element('transcript')
    const runtime = { controllers: new Map([['codex:r1', 'codex:router'], ['codex:r2', 'codex:router']]), dispatches: new Map([
      ['codex:r1', { status: 'completed', unread: true, requestedAt: 1, targetTurnId: 't1', decision: { targetSessionKey: 'codex:worker', forwardedPrompt: 'Check latest progress' } }],
      ['codex:r2', { status: 'failed', unread: true, requestedAt: 2, targetTurnId: 't2', decision: { targetSessionKey: 'codex:second', forwardedPrompt: 'Run the tests' } }],
    ]) }
    const state = { backend: 'codex', selectedId: 'router', model: { turns: [{ id: 'r1' }, { id: 'r2' }] }, threadsByBackend: { codex: [{ id: 'worker', name: 'Worker one' }, { id: 'second', name: 'Worker two' }] } }
    let saves = 0, returns = 0
    const view = {
      captureReadingPosition: () => ({ top: transcript.scrollTop }),
      restoreReadingPosition: position => { returns++; transcript.scrollTop = position.top },
      showRouterResponse: () => { transcript.scrollTop = transcript.querySelector('.router-turn').offsetTop },
    }
    const api = new Function('state', 'runtime', 'element', 'isThread', 'sessionRefKey', 'routerRuntimeKey', 'routerAttentionEntries', 'responseIsVisible', 'view', 'saveDispatch', 'notify', 'parseSessionRefKey', 'threadTitle', 't', 'escapeHtml', `${source.slice(start, end)};${source.slice(bindStart, bindEnd)};return {renderAttention, unreadTurnIds, dispose(){if(attentionFrame!=null)cancelAnimationFrame(attentionFrame);document.removeEventListener('visibilitychange',scheduleAttentionRead);window.removeEventListener('focus',scheduleAttentionRead)}}`)(
      state, runtime, element, () => true, (backend, id) => `${backend}:${id}`, (backend, id) => `${backend}:${id}`, routerAttentionEntries, responseIsVisible, view,
      async () => { saves++ }, message => { throw Error(message) }, key => { const [backend, id] = key.split(':'); return { backend, id } }, thread => thread.name,
      (text, values) => text.replace('{count}', values?.count ?? ''), text => String(text).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;'),
    )
    const check = (ok, message) => { if (!ok) throw Error(message) }
    try {
      api.renderAttention()
      check(!element('router-attention').querySelector('[data-router-unread="codex:r1"]'), 'Older Router turn should not remain eligible')
      check(element('router-attention').querySelector('[data-router-unread="codex:r2"]'), 'Latest Router turn reminder missing')
      check(api.unreadTurnIds().size === 1 && api.unreadTurnIds().has('r2'), 'Only the latest Router turn should have a navigation marker')
      check(transcript.scrollTop === 0, 'Reminder moved reading position')
      element('router-attention').querySelector('[data-router-unread="codex:r2"]').click()
      await new Promise(resolve => setTimeout(resolve, 20))
      check(runtime.dispatches.get('codex:r2').unread === false && saves === 1, 'Clicked response was not marked read exactly once')
      element('router-attention').querySelector('[data-router-return]').click()
      check(returns === 1 && transcript.scrollTop === 0, 'Return to previous reading position failed')
      state.model.turns = [{ id: 'r2' }, { id: 'r1' }]
      runtime.dispatches.get('codex:r1').unread = true
      transcript.scrollTop = transcript.querySelector('.router-turn').offsetTop
      api.renderAttention()
      check(!element('router-attention').querySelector('[data-router-unread="codex:r1"]'), 'Visible completion should not show a reminder')
      await new Promise(resolve => setTimeout(resolve, 20))
      check(runtime.dispatches.get('codex:r1').unread === false && saves === 2, 'Visible completion was not acknowledged automatically')
      transcript.scrollTop = 0
      transcript.dispatchEvent(new Event('scroll'))
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      check(!element('router-attention').querySelector('[data-router-unread="codex:r1"]'), 'Acknowledged visible completion became unread after scrolling away')
      if (window.captureRouterAttention) await window.captureRouterAttention()
      return 'PASS: latest-turn-only reminder, navigation marker, no automatic jump, visible-result acknowledgement and return position'
    } finally { api.dispose(); host.remove() }
  })
  return result
}
