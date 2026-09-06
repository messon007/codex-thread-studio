// Optional real-browser regression: STUDIO_PLAYWRIGHT_MODULE / STUDIO_CHROMIUM_PATH.
// Uses the real HTML/CSS and toolbar, without starting backends or changing user data.
import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {fileURLToPath} from 'node:url'
import {join} from 'node:path'
const {chromium} = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../ui/', import.meta.url))
const html = (await readFile(join(root,'index.html'),'utf8')).replace(/<script\b[^>]*>[\s\S]*?<\/script>/giu,'').replace(/<link\b[^>]*>/giu,'')
const css = await readFile(join(root,'styles.css'),'utf8')
const browser = await chromium.launch({headless:true,executablePath:process.env.STUDIO_CHROMIUM_PATH})
try {
  const page = await browser.newPage({viewport:{width:1400,height:1000}})
  await page.route('http://typography.test/**', async route => {
    const path = new URL(route.request().url()).pathname
    if (path==='/') return route.fulfill({contentType:'text/html',body:html})
    if (path==='/i18n.mjs') return route.fulfill({contentType:'text/javascript',body:await readFile(join(root,'i18n.mjs'),'utf8')})
    return route.abort()
  })
  await page.goto('http://typography.test/')
  await page.addStyleTag({content:css})
  const count = await page.evaluate(async () => {
    const {setLanguage,translateDocument}=await import('/i18n.mjs')
    const icons='.icon-button, .new-thread-button > span, .sidebar-divider button span, .thread-content-search-close, .empty-mark, .review-context-icon, .favorites-empty-mark, .settings-navigation-icon.typography, .typography-profile-icon.typography'
    const nodes=[...document.querySelectorAll('body *')].filter(e=>!['SCRIPT','STYLE','OPTION'].includes(e.tagName)&&!e.matches(icons)&&[...e.childNodes].some(n=>n.nodeType===3&&n.textContent.trim()))
    const snapshot=()=>nodes.map(e=>{const s=getComputedStyle(e);return {size:s.fontSize,weight:s.fontWeight}})
    for(const language of ['en-US','zh-CN']) {
      setLanguage(language);translateDocument()
      const snapshots=[]
      for(const [size,weight] of [[14,400],[20,600]]) {
        for(const profile of ['ui','content','code']) {
          document.documentElement.style.setProperty(`--${profile}-font-size`,`${size}px`)
          document.documentElement.style.setProperty(`--${profile}-font-weight`,weight)
          document.documentElement.style.setProperty(`--${profile}-font-emphasis`,weight+100)
        }
        snapshots.push(snapshot())
      }
      nodes.forEach((e,i)=>{
        for(const property of ['size','weight']) if(snapshots[0][i][property]===snapshots[1][i][property]) throw Error(`${language}: fixed ${property}: ${e.id||e.className||e.tagName} ${e.textContent.trim().slice(0,40)}`)
      })
    }
    return nodes.length
  })
  await page.evaluate(() => {
    const fixture=document.createElement('div')
    fixture.innerHTML='<div class="markdown-body"><p id="reading-sample">Reading <strong>bold</strong> <code id="inline-code-sample">x</code></p><div class="markdown-code-block"><pre><code id="block-code-sample">x</code></pre></div></div><table class="table-grid"><tr><td id="cell-sample">Text</td></tr></table><pre class="artifact-editor-gutter">1</pre><textarea class="artifact-editor">x</textarea>'
    document.body.append(fixture)
    const root=document.documentElement.style
    root.setProperty('--ui-font-size','20px');root.setProperty('--content-font-size','17px');root.setProperty('--code-font-size','14px')
    root.setProperty('--content-font-weight','400');root.setProperty('--code-font-weight','500')
    root.setProperty('--content-font-emphasis','500')
    const sizes={'#reading-sample':17,'#inline-code-sample':12,'#block-code-sample':14,'#cell-sample':15}
    for(const [selector,size]of Object.entries(sizes))if(parseFloat(getComputedStyle(document.querySelector(selector)).fontSize)!==size)throw Error(`Profile crossover: ${selector}`)
    const gutter=getComputedStyle(document.querySelector('.artifact-editor-gutter')),editor=getComputedStyle(document.querySelector('.artifact-editor'))
    if(gutter.lineHeight!==editor.lineHeight||gutter.paddingTop!==editor.paddingTop)throw Error('Code gutter alignment mismatch')
    fixture.remove()
  })
  // Exercise actual dialogs at the allowed size extremes, including hidden panes.
  for(const size of [11,14,20]) {
    await page.evaluate(size=>{
      document.documentElement.style.setProperty('--ui-font-size',`${size}px`)
      for(const dialog of document.querySelectorAll('dialog[open]')) dialog.close()
      document.querySelector('#settings-dialog').showModal()
    },size)
    const clipped = await page.evaluate(() => {
      const failures=[]
      for(const pane of document.querySelectorAll('[data-settings-pane-content]')) {
        for(const sibling of document.querySelectorAll('[data-settings-pane-content]')) sibling.classList.toggle('hidden', sibling!==pane)
        for(const button of document.querySelectorAll('#settings-dialog button')) {
          if(button.getClientRects().length && button.scrollHeight>button.clientHeight+1) failures.push(button.textContent.trim())
        }
      }
      for(const pane of document.querySelectorAll('[data-settings-pane-content]')) pane.classList.toggle('hidden',pane.id!=='settings-pane-general')
      return failures
    })
    assert.deepEqual(clipped,[],`Settings buttons clip at ${size}px`)
    for(const theme of ['light','dark']) {
      await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme)
      await page.screenshot({path:`/tmp/studio-typography-${size}-${theme}.png`})
    }
  }
  const toolbar=await browser.newPage({viewport:{width:900,height:160}})
  await toolbar.setContent(await readFile(join(root,'embedded-browser.html'),'utf8'))
  const metrics=await toolbar.evaluate(()=>{
    window.__embeddedBrowserToolbar.setState({typography:{fontFamily:'sans-serif',fontSize:20,fontWeight:400},tabs:[{id:1,title:'Example',active:true}]})
    const s=getComputedStyle(document.querySelector('.tab-title'))
    return {size:s.fontSize,weight:s.fontWeight,family:s.fontFamily}
  })
  assert.equal(metrics.size,'18px');assert.equal(metrics.weight,'400');assert.equal(metrics.family,'sans-serif')
  console.log(`PASS: ${count} static text elements scale size/weight in both languages; settings screenshots at 11/14/20 in both themes; native toolbar profile applied`)
} finally {await browser.close()}
