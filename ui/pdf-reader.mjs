import * as pdfjsLib from './vendor/pdf.min.mjs'
import { extractPdfOutline } from './document-outline.mjs'

pdfjsLib.GlobalWorkerOptions.workerSrc = new URL('./vendor/pdf.worker.min.mjs', import.meta.url).href

export async function createPdfReader({ container, bytes, initialPage = 1, search = '', onSelection, onPageChange, translate = (value) => value }) {
  if (!container) throw new Error('PDF reader requires a container')
  const task = pdfjsLib.getDocument({ data: bytes.slice(0), isEvalSupported: false })
  const document = await task.promise
  const outline = await extractPdfOutline(document)
  let pageNumber = Math.min(document.numPages, Math.max(1, Number(initialPage) || 1))
  let query = String(search || '').trim()
  let searchGeneration = 0
  let destroyed = false

  const shell = documentNode('div', 'pdf-reader')
  shell.innerHTML = `<div class="pdf-toolbar"><button data-pdf-action="previous" type="button">←</button><span data-pdf-page></span><button data-pdf-action="next" type="button">→</button><input data-pdf-search type="search" placeholder="${escapeHtml(translate('Search PDF…'))}" value="${escapeHtml(query)}"/><span data-pdf-results></span></div><div class="pdf-page-stage"><div class="pdf-page"><canvas></canvas><div class="pdf-text-layer"></div></div></div>`
  container.replaceChildren(shell)
  const canvas = shell.querySelector('canvas')
  const textLayer = shell.querySelector('.pdf-text-layer')
  const stage = shell.querySelector('.pdf-page-stage')

  async function render() {
    const page = await document.getPage(pageNumber)
    const unscaled = page.getViewport({ scale: 1 })
    const maxWidth = Math.max(320, stage.clientWidth - 32)
    const scale = Math.min(2, Math.max(.75, maxWidth / unscaled.width))
    const viewport = page.getViewport({ scale })
    const ratio = window.devicePixelRatio || 1
    canvas.width = Math.floor(viewport.width * ratio)
    canvas.height = Math.floor(viewport.height * ratio)
    canvas.style.width = `${viewport.width}px`
    canvas.style.height = `${viewport.height}px`
    textLayer.style.width = `${viewport.width}px`
    textLayer.style.height = `${viewport.height}px`
    await page.render({ canvasContext: canvas.getContext('2d'), viewport, transform: ratio === 1 ? null : [ratio, 0, 0, ratio, 0, 0] }).promise
    const content = await page.getTextContent()
    textLayer.replaceChildren()
    for (const item of content.items) {
      if (!item.str) continue
      const transform = pdfjsLib.Util.transform(viewport.transform, item.transform)
      const span = documentNode('span', 'pdf-text-item')
      span.textContent = item.str
      span.style.left = `${transform[4]}px`
      span.style.top = `${transform[5] - Math.abs(transform[3])}px`
      span.style.fontSize = `${Math.max(1, Math.abs(transform[3]))}px`
      span.style.transform = `scaleX(${item.width ? (item.width * scale) / Math.max(1, item.str.length * Math.abs(transform[3]) * .5) : 1})`
      if (query && item.str.toLowerCase().includes(query.toLowerCase())) span.classList.add('search-match')
      textLayer.append(span)
    }
    shell.querySelector('[data-pdf-page]').textContent = `${pageNumber} / ${document.numPages}`
    const count = query ? content.items.filter((item) => String(item.str || '').toLowerCase().includes(query.toLowerCase())).length : 0
    shell.querySelector('[data-pdf-results]').textContent = query ? translate('{count} matches', { count }) : ''
    onPageChange?.(pageNumber)
  }

  shell.addEventListener('click', (event) => {
    const action = event.target.closest('[data-pdf-action]')?.dataset.pdfAction
    if (!action) return
    pageNumber = Math.min(document.numPages, Math.max(1, pageNumber + (action === 'next' ? 1 : -1)))
    render().catch(console.error)
  })
  shell.querySelector('[data-pdf-search]').addEventListener('input', async (event) => {
    query = event.target.value.trim()
    const generation = ++searchGeneration
    if (!query) { await render(); return }
    const lowered = query.toLowerCase()
    let count = 0; let firstPage = null
    for (let pageIndex = 1; pageIndex <= document.numPages; pageIndex += 1) {
      const page = await document.getPage(pageIndex)
      const content = await page.getTextContent()
      const matches = content.items.filter((item) => String(item.str || '').toLowerCase().includes(lowered)).length
      if (matches && firstPage == null) firstPage = pageIndex
      count += matches
      if (generation !== searchGeneration) return
    }
    if (firstPage != null) pageNumber = firstPage
    await render()
    if (generation === searchGeneration) shell.querySelector('[data-pdf-results]').textContent = translate('{count} matches', { count })
  })
  textLayer.addEventListener('mouseup', () => {
    const selection = window.getSelection()
    const quote = selection?.toString().trim()
    if (!quote || !selection.rangeCount || !textLayer.contains(selection.anchorNode)) return
    const base = textLayer.getBoundingClientRect()
    const rects = [...selection.getRangeAt(0).getClientRects()].map((rect) => ({
      x: (rect.left - base.left) / base.width, y: (rect.top - base.top) / base.height,
      width: rect.width / base.width, height: rect.height / base.height,
    }))
    const finalRect = selection.getRangeAt(0).getBoundingClientRect()
    onSelection?.({ quote, page: pageNumber, rects, rect: finalRect })
  })
  let region = null
  textLayer.addEventListener('pointerdown', (event) => {
    if (!event.shiftKey || event.button !== 0) return
    event.preventDefault()
    const bounds = textLayer.getBoundingClientRect()
    const marker = documentNode('div', 'pdf-region-selection')
    textLayer.append(marker)
    region = { pointerId: event.pointerId, bounds, marker, startX: event.clientX, startY: event.clientY }
    textLayer.setPointerCapture(event.pointerId)
  })
  textLayer.addEventListener('pointermove', (event) => {
    if (!region || region.pointerId !== event.pointerId) return
    const left = Math.min(region.startX, event.clientX) - region.bounds.left
    const top = Math.min(region.startY, event.clientY) - region.bounds.top
    const width = Math.abs(event.clientX - region.startX)
    const height = Math.abs(event.clientY - region.startY)
    Object.assign(region.marker.style, { left: `${left}px`, top: `${top}px`, width: `${width}px`, height: `${height}px` })
  })
  textLayer.addEventListener('pointerup', (event) => {
    if (!region || region.pointerId !== event.pointerId) return
    const current = region; region = null
    const rect = current.marker.getBoundingClientRect()
    if (rect.width < 8 || rect.height < 8) { current.marker.remove(); return }
    onSelection?.({ quote: `[PDF region on page ${pageNumber}]`, page: pageNumber, rects: [{ x: (rect.left - current.bounds.left) / current.bounds.width, y: (rect.top - current.bounds.top) / current.bounds.height, width: rect.width / current.bounds.width, height: rect.height / current.bounds.height }], rect })
  })
  await render()
  return {
    pageCount: document.numPages,
    outline: () => outline,
    goToPage: async (page) => { pageNumber = Math.min(document.numPages, Math.max(1, Number(page) || 1)); await render() },
    destroy: () => { if (!destroyed) { destroyed = true; task.destroy(); shell.remove() } },
  }
}

function documentNode(tag, className) { const node = document.createElement(tag); node.className = className; return node }
function escapeHtml(value) { return String(value).replace(/[&<>"']/gu, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char])) }
