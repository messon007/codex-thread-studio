// Optional browser regression smoke test for vendored dependency updates.
// Requires an installed Playwright package, not included in production assets.
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { resolve, extname } from 'node:path'
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const browser = await chromium.launch({ headless:true, executablePath:process.env.STUDIO_CHROMIUM_PATH })

function pdfFixture() {
  const stream = text => `BT /F1 20 Tf 30 150 Td (${text}) Tj ET`
  const first = stream('Remote Studio first page')
  const second = stream('Remote Studio second page')
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R >>',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    `<< /Length ${first.length} >>\nstream\n${first}\nendstream`,
    `<< /Length ${second.length} >>\nstream\n${second}\nendstream`,
  ]
  let pdf = '%PDF-1.4\n'
  const offsets = []
  objects.forEach((object,i) => { offsets.push(pdf.length);pdf += `${i+1} 0 obj\n${object}\nendobj\n` })
  const xref = pdf.length
  pdf += `xref\n0 ${objects.length+1}\n0000000000 65535 f \n${offsets.map(offset=>`${String(offset).padStart(10,'0')} 00000 n \n`).join('')}trailer\n<< /Size ${objects.length+1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`
  return [...Buffer.from(pdf)]
}

try {
  const context = await browser.newContext({ viewport:{width:1400,height:1000} })
  await context.route('**/*', async route => {
    const url = new URL(route.request().url())
    if (url.hostname !== 'studio.test') return route.abort()
    if (url.pathname === '/') return route.fulfill({contentType:'text/html',body:'<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="/ui/styles.css"><div id="diagrams"></div><div id="pdf"></div>'})
    const path = resolve(root, `.${url.pathname}`)
    if (!path.startsWith(root)) return route.abort()
    try {
      const body = await readFile(path)
      return route.fulfill({body,contentType:extname(path)==='.css'?'text/css':'text/javascript'})
    } catch { return route.fulfill({status:404,body:'not found'}) }
  })
  const page = await context.newPage()
  await page.goto('http://studio.test/')
  await page.addScriptTag({url:'/ui/vendor/mermaid.min.js'})
  const diagrams = await page.evaluate(async () => {
    const {marked} = await import('/ui/vendor/marked.esm.js')
    const {default:DOMPurify} = await import('/ui/vendor/purify.es.mjs')
    const {mermaidInitializeConfig} = await import('/ui/mermaid-config.mjs')
    const check = (value,message) => {if (!value) throw Error(message)}
    const markdown = '# 中文标题\n\n| A | B |\n|---|---|\n| one | two |\n\n- [x] done\n\n```js\nconst x = 1\n```\n\n[link](https://example.com)\n\n<img src=x onerror="window.injected=true"><script>window.injected=true</script>'
    const clean = DOMPurify.sanitize(marked.parse(markdown), {USE_PROFILES:{html:true},FORBID_TAGS:['button','form','iframe','object','embed','script','style'],FORBID_ATTR:['style']})
    const box = document.createElement('div');box.innerHTML=clean
    check(box.querySelector('table') && box.querySelector('code.language-js') && box.querySelector('h1').textContent==='中文标题','Markdown structure changed')
    check(!box.querySelector('script,[onerror]'),'unsafe Markdown HTML survived sanitization')
    const samples = [
      ['flowchart','flowchart LR\n A[中文输入] --> B{检查}\n B --> C[完成]'],
      ['class','classDiagram\n Animal <|-- Duck\n Animal : +name\n Duck : +swim()'],
      ['er','erDiagram\n USER ||--o{ SESSION : owns\n SESSION { string id }'],
      ['c4','C4Context\n Person(user, "用户", "使用 Studio")\n System(studio, "Studio", "管理会话")\n Rel(user, studio, "使用")'],
    ]
    const results=[]
    for (const dark of [false,true]) {
      mermaid.initialize(mermaidInitializeConfig({}, {dark,fontFamily:'sans-serif'}))
      for (const [name,text] of samples) {
        const {svg} = await mermaid.render(`diagram-${name}-${dark}`,text)
        const card=document.createElement('section');card.style.cssText=`padding:20px;background:${dark?'#171b24':'#fff'};color:${dark?'white':'black'}`
        card.innerHTML=DOMPurify.sanitize(svg,{USE_PROFILES:{svg:true,svgFilters:true},ADD_TAGS:['style'],FORBID_TAGS:['foreignObject','script']})
        document.getElementById('diagrams').append(card)
        check(card.querySelector('svg') && !card.querySelector('script,foreignObject'),'diagram missing or unsafe')
        results.push(`${name}:${dark?'dark':'light'}`)
      }
    }
    return results
  })
  await page.screenshot({path:process.env.STUDIO_UI_SCREENSHOT || '/tmp/studio-ui-dependencies.png',fullPage:true})
  const pdf = await page.evaluate(async bytes => {
    const {createPdfReader} = await import('/ui/pdf-reader.mjs')
    const reader = await createPdfReader({container:document.getElementById('pdf'),bytes:Uint8Array.from(bytes)})
    if (reader.pageCount!==2) throw Error('PDF page count changed')
    if (!document.querySelector('.pdf-text-layer').textContent.includes('first page')) throw Error('first PDF page text missing')
    await reader.goToPage(2)
    if (!document.querySelector('.pdf-text-layer').textContent.includes('second page')) throw Error('PDF navigation failed')
    const canvas=document.querySelector('#pdf canvas')
    const data=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data
    const hasInk=data.some((value,index)=>index%4!==3 && value<100)
    if (!hasInk) throw Error('PDF canvas has no rendered text')
    reader.destroy()
    return 'PASS: render, text layer, navigation, destroy'
  },pdfFixture())
  console.log(JSON.stringify({markdown:'PASS: structure and sanitization',diagrams,pdf},null,2))
} finally { await browser.close() }
