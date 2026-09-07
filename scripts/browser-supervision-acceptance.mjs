import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

export async function checkSupervisionAcceptance(page) {
  const html = await (await page.request.get(new URL('/', page.url()).href)).text()
  await page.evaluate(async html => {
    const check = (value, message) => { if (!value) throw Error(message) }
    const { createTurnSupervision } = await import('/turn-supervision.mjs')
    const app = await (await fetch('/app.js')).text()
    const parsed = new DOMParser().parseFromString(html, 'text/html')
    check(!parsed.querySelector('#composer-attachments'), 'Attachment submenu returned')
    check(parsed.querySelector('#composer-model').nextElementSibling.id === 'router-composer-target', 'Routing must follow model')
    const host = document.createElement('section'); host.id = 'tools-acceptance'
    host.style.cssText = 'width:900px;min-height:360px;padding:28px;background:var(--panel);color:var(--text)'
    host.innerHTML = '<h2>Turn supervision</h2><div id="wand-fixtures"></div><div style="height:90px"></div>'
    host.append(parsed.querySelector('#composer-input'), parsed.querySelector('.composer-footer'))
    document.body.append(host)
    const css = document.createElement('link'); css.rel = 'stylesheet'; css.href = '/styles.css'
    await new Promise(resolve => { css.onload = resolve; css.onerror = resolve; document.head.append(css) })
    const $ = selector => host.querySelector(selector)
    const snapshot = { activeTurnId:'root', blocked:false, turns:[{ id:'root', status:'inProgress', items:[{ type:'userMessage', content:[{text:'Finish A and B'}] }] }] }
    const state = { backend:'codex', selectedId:'ordinary', model:snapshot }
    const controller = createTurnSupervision({ snapshot:()=>snapshot, evaluate:async()=>{throw Error('No live evaluator')}, prepare:async()=>{}, send:async()=>{throw Error('No live send')}, changed:()=>{} })
    const deps = {state,$,isCodexBackend:b=>b!=='opencode',isArchivedPreview:()=>false,turnSupervision:controller,messageQueueModel:()=>snapshot,
      escapeHtml:text=>String(text).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;'),t:text=>text,isRouterThread:()=>state.selectedId==='router'}
    const start=app.indexOf('const supervisionWand ='),end=app.indexOf('\nconst composerActions',start)
    const api=new Function(...Object.keys(deps), app.slice(start,end)+';return {renderSupervisionMenu,renderComposerTools}')(...Object.values(deps))
    const ref={backend:'codex',id:'ordinary'}
    check(api.renderSupervisionMenu('root').includes('Supervise this turn'), 'Running task has no wand')
    check(!api.renderSupervisionMenu('old'), 'Old task exposes wand')
    snapshot.turns[0].status='completed'
    check(!api.renderSupervisionMenu('root'), 'Completed task offers new supervision')
    snapshot.turns[0].status='inProgress'
    controller.start(ref,'root')
    check(api.renderSupervisionMenu('root').includes('data-supervision-action="stop"'), 'Active supervision cannot stop')
    controller.start({backend:'codex',id:'second'},'root')
    controller.stop(ref)
    check(controller.activeFor({backend:'codex',id:'second'}), 'Stopping one task stopped another')
    api.renderComposerTools()
    check($('#composer-tools-content').querySelectorAll('button').length===1, 'Plus menu must expose one action')
    check(!$('#composer-tools-content').querySelector('input'), 'Pre-send checkbox returned')
    const bindStart=app.indexOf("  $('#composer-add-image').addEventListener")
    const bindEnd=app.indexOf("  $('#composer-image-input').addEventListener('change'",bindStart)
    new Function('$','renderComposerTools','handleTranscriptClick',app.slice(bindStart,bindEnd))($,api.renderComposerTools,()=>{})
    $('#wand-fixtures').innerHTML='<div class="message-actions">Research '+api.renderSupervisionMenu('root',ref)+'</div><div class="message-actions">Implementation '+api.renderSupervisionMenu('root',{backend:'codex',id:'second'})+'</div>'
    const glyph=$('#wand-fixtures svg'), style=getComputedStyle(glyph)
    check(style.width==='18px' && style.height==='18px' && style.strokeWidth==='1.4px', 'Wand does not match native icon dimensions/stroke')
    const router = await (await fetch('/thread-router-controller.mjs')).text()
    const actionStart = router.indexOf('  function renderSourceActions('), actionEnd = router.indexOf('  async function openTarget(', actionStart)
    const renderSourceActions = new Function('escapeHtml','t',router.slice(actionStart,actionEnd)+';return renderSourceActions')(deps.escapeHtml,deps.t)
    const itemStart = app.indexOf('function renderItem('), itemEnd = app.indexOf('function renderUserMessageImages(',itemStart)
    const itemDeps = {...deps,reviewNotes:{favoriteForSource:()=>null},conversationTrackIcon:()=>'',renderMarkdown:text=>text,sessionMapVisibleText:text=>text,threadRouter:{renderSourceActions}}
    const renderItem = new Function(...Object.keys(itemDeps),app.slice(itemStart,itemEnd)+';return renderItem')(...Object.values(itemDeps))
    const reply = document.createElement('div')
    reply.innerHTML = renderItem({type:'agentMessage',id:'reply',text:'Completed response'},'root',{forkable:true,sourceRef:{backend:'codex',id:'second',key:'codex:second'}})
    host.insertBefore(reply,host.children[2])
    const nativeLast = reply.querySelector('[data-fork-turn]')
    check(nativeLast.nextElementSibling?.hasAttribute('data-router-reply'), 'Extensions did not follow the final native action (Fork after Favorite)')
    check(reply.querySelectorAll('.message-actions').length===1 && reply.querySelectorAll('.router-source-action').length===4,'Extensions duplicated or split the native toolbar')
    $('#router-composer-target').classList.remove('hidden')
    check(!$('#router-clear-target') && !$('#composer-supervision-status'), 'Removed status/close controls returned')
    $('#router-target-status').classList.add('hidden')
    $('#composer-tools').open=true
    check(Boolean(navigator.locks),'No safe supervision ownership support')
  }, html)
  const chooser = page.waitForEvent('filechooser')
  await page.locator('#composer-add-image').click()
  await chooser
  if(process.env.STUDIO_SCREENSHOT_DIR) {
    await mkdir(process.env.STUDIO_SCREENSHOT_DIR,{recursive:true})
    await page.locator('#tools-acceptance').screenshot({path:join(process.env.STUDIO_SCREENSHOT_DIR,'supervision-wand.png')})
  }
  await page.locator('#tools-acceptance').evaluate(node=>node.remove())
  return 'PASS: running-only wand, one Plus entry, no pre-send intent, independent tasks, 18px/1.4px SVG, direct image chooser, Router-after-model layout'
}
