import { ePub } from './vendor/epub.mjs'

const THEMES = new Set(['light', 'sepia', 'dark'])
const FLOWS = new Set(['paginated', 'scrolled'])
const MAX_SELECTION = 16_000

export const DEFAULT_EPUB_READER_STATE = Object.freeze({
  cfi: '',
  chapterLabel: '',
  progress: 0,
  fontScale: 1,
  theme: 'light',
  flow: 'paginated',
  tocOpen: false,
})

export function normalizeEpubReaderState(value = {}) {
  const progress = Number(value.progress)
  const fontScale = Number(value.fontScale)
  return {
    cfi: bounded(value.cfi, 8192),
    chapterLabel: bounded(value.chapterLabel, 1024),
    progress: Number.isFinite(progress) ? clamp(progress, 0, 1) : 0,
    fontScale: Number.isFinite(fontScale) ? clamp(fontScale, 0.75, 2) : 1,
    theme: THEMES.has(value.theme) ? value.theme : 'light',
    flow: FLOWS.has(value.flow) ? value.flow : 'paginated',
    tocOpen: Boolean(value.tocOpen),
  }
}

export function flattenEpubToc(items = [], depth = 0) {
  return (Array.isArray(items) ? items : []).flatMap((item) => {
    const current = [{
      href: bounded(item?.href, 8192),
      label: bounded(item?.label, 1024) || 'Untitled',
      depth: clamp(Number(depth) || 0, 0, 12),
    }]
    return current.concat(flattenEpubToc(item?.subitems, depth + 1))
  })
}

export function chapterLabelForHref(toc, href) {
  const target = stripFragment(href)
  const exact = flattenEpubToc(toc).find((item) => stripFragment(item.href) === target)
  return exact?.label || ''
}

export function epubPaperTheme(name) {
  const theme = THEMES.has(name) ? name : 'light'
  if (theme === 'dark') {
    return { background: '#171b1f', foreground: '#e8ecec', muted: '#aab4b5', link: '#74cdbf' }
  }
  if (theme === 'sepia') {
    return { background: '#f4ecd8', foreground: '#453b2d', muted: '#776b5a', link: '#28766c' }
  }
  return { background: '#fffdfa', foreground: '#263234', muted: '#687476', link: '#137c70' }
}

export async function createEpubReader({
  container,
  bytes,
  initialState,
  translate = (value) => value,
  onSelection = () => {},
  onRelocate = () => {},
  onExternalLink = () => {},
} = {}) {
  if (!(container instanceof HTMLElement)) throw new Error('EPUB reader requires a container')
  if (!(bytes instanceof ArrayBuffer)) throw new Error('EPUB reader requires an ArrayBuffer')

  const readerState = normalizeEpubReaderState(initialState)
  const shell = buildShell(container, translate)
  const book = ePub(bytes, { replacements: 'blobUrl' })
  let rendition = null
  let destroyed = false
  let currentToc = []
  let currentHref = ''
  let locationGeneration = 0

  const notifyRelocate = () => {
    if (destroyed) return
    onRelocate({ ...readerState, href: currentHref })
  }

  const applyTheme = () => {
    if (!rendition) return
    const palette = epubPaperTheme(readerState.theme)
    const rules = {
      'html, body': {
        'background': `${palette.background} !important`,
        'color': `${palette.foreground} !important`,
      },
      'body': {
        'font-family': 'Georgia, "Noto Serif CJK SC", "Source Han Serif SC", serif !important',
        'line-height': '1.72 !important',
        'padding-left': 'clamp(16px, 5vw, 54px) !important',
        'padding-right': 'clamp(16px, 5vw, 54px) !important',
      },
      'a': { 'color': `${palette.link} !important` },
      'img, svg': { 'max-width': '100% !important', 'height': 'auto !important' },
      'pre, code': { 'white-space': 'pre-wrap !important', 'overflow-wrap': 'anywhere !important' },
      '::selection': { 'background': 'rgba(19, 124, 112, .28) !important' },
    }
    rendition.themes.register('studio-reader', rules)
    rendition.themes.select('studio-reader')
    rendition.themes.fontSize(`${Math.round(readerState.fontScale * 100)}%`)
    shell.root.dataset.paper = readerState.theme
  }

  const sanitizeContents = (contents) => {
    const document = contents?.document
    if (!document) return
    document.querySelectorAll('script, iframe, frame, object, embed, form, input, button, textarea, select, meta[http-equiv="refresh" i]').forEach((node) => node.remove())
    document.querySelectorAll('*').forEach((element) => {
      for (const attribute of [...element.attributes]) {
        if (/^on/iu.test(attribute.name) || attribute.name === 'srcdoc' || (/^(?:href|src|xlink:href|formaction)$/iu.test(attribute.name) && /^\s*(?:javascript|file):/iu.test(attribute.value))) {
          element.removeAttribute(attribute.name)
        }
      }
    })
    document.querySelectorAll('a[href]').forEach((anchor) => {
      const href = anchor.getAttribute('href') || ''
      if (!/^(https?:)?\/\//iu.test(href)) return
      anchor.addEventListener('click', (event) => {
        event.preventDefault()
        event.stopPropagation()
        onExternalLink(href)
      })
    })
    contents.window?.addEventListener('keydown', handleKeydown)
  }

  const selected = (cfiRange, contents) => {
    const selection = contents?.window?.getSelection?.()
    const quote = String(selection?.toString() || '').trim().slice(0, MAX_SELECTION)
    if (!quote || !selection.rangeCount) return
    const rangeRect = selection.getRangeAt(0).getBoundingClientRect()
    const frameRect = contents.window.frameElement?.getBoundingClientRect?.() || { left: 0, top: 0 }
    onSelection({
      quote,
      cfiRange: bounded(cfiRange, 8192),
      href: currentHref,
      chapterLabel: readerState.chapterLabel,
      rect: {
        left: frameRect.left + rangeRect.left,
        top: frameRect.top + rangeRect.top,
        width: rangeRect.width,
        height: rangeRect.height,
      },
    })
  }

  const relocated = (location) => {
    if (!location?.start) return
    readerState.cfi = bounded(location.start.cfi, 8192)
    currentHref = bounded(location.start.href, 8192)
    readerState.chapterLabel = chapterLabelForHref(currentToc, currentHref) || readerState.chapterLabel
    const generated = Number(book.locations?.total) > 0
    const percentage = generated ? book.locations.percentageFromCfi(readerState.cfi) : location.start.percentage
    if (Number.isFinite(percentage)) readerState.progress = clamp(percentage, 0, 1)
    renderPosition(shell, readerState, translate)
    notifyRelocate()
  }

  const mountRendition = async (target = '') => {
    rendition?.destroy()
    shell.viewer.replaceChildren()
    rendition = book.renderTo(shell.viewer, {
      width: '100%',
      height: '100%',
      manager: readerState.flow === 'scrolled' ? 'continuous' : 'default',
      flow: readerState.flow === 'scrolled' ? 'scrolled' : 'paginated',
      spread: 'none',
      allowScriptedContent: false,
    })
    rendition.hooks.content.register(sanitizeContents)
    rendition.on('selected', selected)
    rendition.on('relocated', relocated)
    applyTheme()
    await rendition.display(target || undefined)
  }

  const changeFlow = async (flow) => {
    if (!FLOWS.has(flow) || readerState.flow === flow) return
    const target = readerState.cfi
    readerState.flow = flow
    updateSettings(shell, readerState)
    await mountRendition(target)
    notifyRelocate()
  }

  const changeTheme = (theme) => {
    if (!THEMES.has(theme)) return
    readerState.theme = theme
    updateSettings(shell, readerState)
    applyTheme()
    notifyRelocate()
  }

  const changeFont = (step) => {
    readerState.fontScale = clamp(Math.round((readerState.fontScale + step) * 20) / 20, 0.75, 2)
    updateSettings(shell, readerState)
    applyTheme()
    notifyRelocate()
  }

  function handleKeydown(event) {
    if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return
    if (event.key === 'ArrowLeft' || event.key === 'PageUp') {
      event.preventDefault()
      rendition?.prev()
    } else if (event.key === 'ArrowRight' || event.key === 'PageDown') {
      event.preventDefault()
      rendition?.next()
    } else if (event.key === 'Escape') {
      shell.settings.classList.add('hidden')
    }
  }

  shell.root.addEventListener('keydown', handleKeydown)
  shell.previous.addEventListener('click', () => rendition?.prev())
  shell.next.addEventListener('click', () => rendition?.next())
  shell.settingsToggle.addEventListener('click', () => shell.settings.classList.toggle('hidden'))
  shell.fontDecrease.addEventListener('click', () => changeFont(-0.1))
  shell.fontIncrease.addEventListener('click', () => changeFont(0.1))
  shell.settings.addEventListener('click', (event) => {
    const theme = event.target.closest('[data-epub-theme]')?.dataset.epubTheme
    const flow = event.target.closest('[data-epub-flow]')?.dataset.epubFlow
    if (theme) changeTheme(theme)
    if (flow) changeFlow(flow).catch(() => {})
  })
  try {
    const [, navigation, metadata] = await Promise.all([
      book.ready,
      book.loaded.navigation,
      book.loaded.metadata,
    ])
    if (destroyed) throw new Error('EPUB reader was closed while loading')
    currentToc = navigation?.toc || []
    shell.bookTitle.textContent = displayMetadata(metadata?.title) || translate('电子书')
    shell.bookAuthor.textContent = displayMetadata(metadata?.creator)
    updateSettings(shell, readerState)
    await mountRendition(readerState.cfi)
    const generation = ++locationGeneration
    book.locations.generate(1600).then(() => {
      if (destroyed || generation !== locationGeneration || !readerState.cfi) return
      const percentage = book.locations.percentageFromCfi(readerState.cfi)
      if (Number.isFinite(percentage)) {
        readerState.progress = clamp(percentage, 0, 1)
        renderPosition(shell, readerState, translate)
        notifyRelocate()
      }
    }).catch(() => {})
  } catch (error) {
    rendition?.destroy()
    book.destroy()
    container.replaceChildren()
    throw error
  }

  return {
    title: shell.bookTitle.textContent,
    state: () => ({ ...readerState, href: currentHref }),
    outline: () => flattenEpubToc(currentToc).map((item, index) => ({
      id: `epub-${index + 1}`,
      label: item.label,
      depth: item.depth,
      target: { kind: 'epub', href: item.href },
    })),
    display: (target) => rendition?.display(target),
    destroy() {
      destroyed = true
      locationGeneration += 1
      rendition?.destroy()
      book.destroy()
      container.replaceChildren()
    },
  }
}

function buildShell(container, translate) {
  container.innerHTML = `<section class="epub-reader" tabindex="0">
    <header class="epub-toolbar">
      <div class="epub-book-identity"><strong></strong><small></small></div>
      <div class="epub-page-actions">
        <button class="epub-tool-button epub-previous" type="button" title="${escapeHtml(translate('上一页'))}" aria-label="${escapeHtml(translate('上一页'))}">←</button>
        <button class="epub-tool-button epub-next" type="button" title="${escapeHtml(translate('下一页'))}" aria-label="${escapeHtml(translate('下一页'))}">→</button>
        <button class="epub-tool-button epub-settings-toggle" type="button" title="${escapeHtml(translate('阅读设置'))}" aria-label="${escapeHtml(translate('阅读设置'))}">Aa</button>
      </div>
      <div class="epub-settings-popover hidden">
        <div class="epub-setting-row"><span>${escapeHtml(translate('字号'))}</span><div><button class="epub-font-decrease" type="button">A−</button><output class="epub-font-value">100%</output><button class="epub-font-increase" type="button">A+</button></div></div>
        <div class="epub-setting-row"><span>${escapeHtml(translate('纸张'))}</span><div class="epub-setting-options"><button data-epub-theme="light" type="button">${escapeHtml(translate('明亮'))}</button><button data-epub-theme="sepia" type="button">${escapeHtml(translate('柔和'))}</button><button data-epub-theme="dark" type="button">${escapeHtml(translate('深色'))}</button></div></div>
        <div class="epub-setting-row"><span>${escapeHtml(translate('翻阅'))}</span><div class="epub-setting-options"><button data-epub-flow="paginated" type="button">${escapeHtml(translate('分页'))}</button><button data-epub-flow="scrolled" type="button">${escapeHtml(translate('连续'))}</button></div></div>
      </div>
    </header>
    <div class="epub-reader-body">
      <div class="epub-viewer"></div>
    </div>
    <footer class="epub-reader-footer"><span class="epub-chapter"></span><span class="epub-progress">0%</span></footer>
  </section>`
  const root = container.firstElementChild
  return {
    root,
    viewer: root.querySelector('.epub-viewer'),
    previous: root.querySelector('.epub-previous'),
    next: root.querySelector('.epub-next'),
    settingsToggle: root.querySelector('.epub-settings-toggle'),
    settings: root.querySelector('.epub-settings-popover'),
    fontDecrease: root.querySelector('.epub-font-decrease'),
    fontIncrease: root.querySelector('.epub-font-increase'),
    fontValue: root.querySelector('.epub-font-value'),
    chapter: root.querySelector('.epub-chapter'),
    progress: root.querySelector('.epub-progress'),
    bookTitle: root.querySelector('.epub-book-identity strong'),
    bookAuthor: root.querySelector('.epub-book-identity small'),
  }
}

function renderPosition(shell, state, translate) {
  shell.chapter.textContent = state.chapterLabel || translate('正在阅读')
  shell.progress.textContent = `${Math.round(clamp(state.progress, 0, 1) * 100)}%`
}

function updateSettings(shell, state) {
  shell.fontValue.textContent = `${Math.round(state.fontScale * 100)}%`
  shell.settings.querySelectorAll('[data-epub-theme]').forEach((button) => button.classList.toggle('active', button.dataset.epubTheme === state.theme))
  shell.settings.querySelectorAll('[data-epub-flow]').forEach((button) => button.classList.toggle('active', button.dataset.epubFlow === state.flow))
}

function displayMetadata(value) {
  if (Array.isArray(value)) return value.map(displayMetadata).filter(Boolean).join('、')
  if (value && typeof value === 'object') return displayMetadata(value.name || Object.values(value)[0])
  return bounded(value, 1024)
}

function stripFragment(value) {
  return String(value || '').split('#')[0].replace(/^\.\//u, '')
}

function bounded(value, limit) {
  return String(value || '').trim().slice(0, limit)
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value))
}

function escapeHtml(value) {
  return String(value || '').replace(/[&<>"']/gu, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character])
}
