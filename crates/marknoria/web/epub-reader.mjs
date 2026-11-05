// Adapted from Codex Thread Studio a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a (MIT).
// License: third-party/codex-thread-studio-MIT.txt. Local source only.
import { uiLabels } from './translation.mjs'
import { ePub, replaceBase } from './vendor/epub.mjs'

// Same public surface as the PDF component: no host-private DOM or credentials.
export function mountEpubReader(root, options) {
  const {
    documentId,
    revision,
    resourceId,
    api,
    onState = () => {},
    onSelection = () => {}
  } = options
  const lifetime = new AbortController(),
    started = performance.now()
  let instance,
    scope,
    source,
    reading = normalizeEpubReaderState(options.initialState),
    captured = null
  let disposed = false,
    readyState = false,
    busy = 0,
    lastError = null,
    firstContentMs = null,
    firstLocation
  const firstLocationReady = new Promise((resolve) => {
    firstLocation = resolve
  })
  const archiveHref = (href) => {
    if (!href || !source || !scope) return undefined
    const url = new URL(href, source)
    if (!url.href.startsWith(scope.href)) throw Error('EPUB reference escaped its granted scope')
    return decodeURIComponent(url.pathname.slice(scope.pathname.length)) + url.hash
  }
  const locationFor = () =>
    reading.cfi
      ? { kind: 'epub', cfi: checkedCfi(reading.cfi), href: archiveHref(reading.href) }
      : null
  const state = () => ({
    format: 'epub',
    ...reading,
    ready: readyState,
    busy: busy > 0 || !readyState || reading.progressIndex?.status === 'indexing',
    error: lastError,
    firstUsefulContentMs: firstContentMs,
    position: locationFor()
      ? { version: 1, resourceId, revision, view: 'preview', location: locationFor() }
      : null
  })
  const emit = () => {
    if (!disposed) onState(state())
  }
  const error = (value) => {
    if (!disposed) {
      lastError = String(value.message || value).slice(0, 800)
      emit()
    }
  }
  const currentSelection = (value) => {
    captured = value
      ? {
          documentId,
          revision,
          quote: value.quote,
          quoteTruncated: value.quoteTruncated,
          precision: 'exact',
          location: {
            kind: 'epub',
            cfi: checkedCfi(value.cfiRange),
            href: archiveHref(value.href)
          },
          viewportRect: {
            x: value.rect.left,
            y: value.rect.top,
            width: value.rect.width,
            height: value.rect.height
          }
        }
      : null
    if (!disposed) onSelection(captured)
  }
  const ready = (async () => {
    const metadata = await api('/api/epub', lifetime.signal)
    if (metadata.revision !== revision) throw Error('EPUB revision changed')
    lifetime.signal.throwIfAborted()
    scope = new URL(metadata.scope, import.meta.url)
    source = new URL(metadata.package.split('/').map(encodeURIComponent).join('/'), scope)
    instance = await createEpubReader({
      container: root,
      source: source.href,
      resourceRoot: scope.href,
      signal: lifetime.signal,
      initialState: options.initialState,
      typography: options.typography || options.initialState?.typography,
      translate: options.translate,
      onSelection: currentSelection,
      onRelocate: (value) => {
        reading = value
        if (value.cfi) firstLocation()
        emit()
      },
      onError: error,
      onExternalLink: (url) => options.onExternalLink?.(url)
    })
    reading = instance.state()
    if (!reading.cfi) {
      let timer, abort
      try {
        await Promise.race([
          firstLocationReady,
          new Promise((_, reject) => {
            abort = () => reject(new DOMException('EPUB disposed', 'AbortError'))
            lifetime.signal.addEventListener('abort', abort, { once: true })
            timer = setTimeout(() => reject(Error('EPUB did not publish an initial CFI')), 10000)
          })
        ])
      } finally {
        clearTimeout(timer)
        lifetime.signal.removeEventListener('abort', abort)
      }
    }
    lifetime.signal.throwIfAborted()
    readyState = true
    firstContentMs = performance.now() - started
    emit()
    return state()
  })().catch((value) => {
    error(value)
    instance?.destroy()
    throw value
  })
  void ready.catch(() => {})
  async function work(action) {
    await ready
    lifetime.signal.throwIfAborted()
    busy++
    emit()
    try {
      return await action()
    } finally {
      busy--
      emit()
    }
  }
  return {
    ready,
    state,
    selection: () => {
      if (!instance?.selection()) captured = null
      return captured
    },
    async navigate(location) {
      return work(async () => {
        captured = null
        if (location?.kind === 'epub') await instance.display(checkedCfi(location.cfi))
        else if (location?.kind === 'epubChapter' && typeof location.href === 'string') {
          const [path, ...fragment] = location.href.split('#'),
            target = new URL(path.split('/').map(encodeURIComponent).join('/'), scope)
          if (fragment.length) target.hash = fragment.join('#')
          await instance.display(target.href)
        } else throw Error('An EPUB CFI or original chapter is required')
        reading = instance.state()
      })
    },
    async settings(patch) {
      if (patch.flow != null && !FLOWS.has(patch.flow)) throw Error('Invalid EPUB flow')
      if (patch.theme != null && !THEMES.has(patch.theme)) throw Error('Invalid EPUB theme')
      if (
        patch.fontScale != null &&
        (!Number.isFinite(patch.fontScale) || patch.fontScale < 0.75 || patch.fontScale > 2)
      )
        throw Error('EPUB font scale must be 75–200%')
      return work(async () => {
        await instance.settings(patch)
        reading = instance.state()
      })
    },
    async outline({ offset = 0 } = {}) {
      await ready
      if (!Number.isSafeInteger(offset) || offset < 0) throw Error('Invalid EPUB outline offset')
      const items = instance.outline()
      return {
        revision,
        total: items.length,
        offset,
        items: items.slice(offset, offset + 256).map((item, i) => ({
          id: offset + i,
          level: item.depth + 1,
          label: item.label,
          location: { kind: 'epubChapter', href: archiveHref(item.target.href) }
        }))
      }
    },
    async search(query, { signal } = {}) {
      return work(async () => {
        const result = await instance.search(query, { signal })
        return {
          revision,
          ...result,
          matches: result.matches.map((match) => ({
            excerpt: match.excerpt,
            label: 'EPUB',
            location: { kind: 'epub', cfi: match.cfi, href: archiveHref(match.href) }
          }))
        }
      })
    },
    buildProgressIndex: ({ signal } = {}) =>
      work(async () => {
        await instance.buildProgressIndex({ signal })
        reading = instance.state()
        return state()
      }),
    cancelProgressIndex: () => instance?.cancelProgressIndex(),
    seekProgress: (percentage) =>
      work(async () => {
        await instance.seekProgress(percentage)
        reading = instance.state()
        return state()
      }),
    dispose() {
      if (!disposed) {
        disposed = true
        lifetime.abort()
        instance?.destroy()
        captured = null
        readyState = false
      }
      return ready.catch(() => {})
    }
  }
}

const THEMES = new Set(['light', 'sepia', 'dark'])
const FLOWS = new Set(['paginated', 'scrolled'])
const MAX_SELECTION = 16_000

export const EPUB_PROGRESS_PROFILE = Object.freeze({
  version: 1,
  chunkUtf16: 1600,
  maxChapters: 256,
  maxChapterUtf16: 256 * 1024,
  maxTotalUtf16: 16 * 1024 * 1024,
  maxLocations: 16384,
  maxCfiBytes: 2 * 1024 * 1024,
  maxChapterNodes: 32768,
  maxDepth: 64,
  maxIdentifierUtf16: 128,
  deadlineMs: 15000
})

export const DEFAULT_EPUB_READER_STATE = Object.freeze({
  cfi: '',
  chapterLabel: '',
  progress: 0,
  fontScale: 1,
  theme: 'light',
  flow: 'paginated',
  tocOpen: false
})

export function normalizeEpubReaderState(value = {}) {
  const progress = Number(value.progress)
  const fontScale = Number(value.fontScale)
  return {
    cfi: checkedCfi(value.cfi || ''),
    chapterLabel: bounded(value.chapterLabel, 1024),
    progress: Number.isFinite(progress) ? clamp(progress, 0, 1) : 0,
    fontScale: Number.isFinite(fontScale) ? clamp(fontScale, 0.75, 2) : 1,
    theme: THEMES.has(value.theme) ? value.theme : 'light',
    flow: FLOWS.has(value.flow) ? value.flow : 'paginated',
    tocOpen: Boolean(value.tocOpen)
  }
}

export function flattenEpubToc(items = [], depth = 0) {
  const result = [],
    pending = (Array.isArray(items) ? items : []).map((item) => ({ item, depth })).reverse()
  while (pending.length) {
    if (result.length >= 2000 || pending.length > 2000)
      throw Error('EPUB outline exceeds 2,000 entries')
    const { item, depth } = pending.pop()
    result.push({
      href: checkedText(item?.href || '', 8192, 'EPUB chapter'),
      label: bounded(item?.label, 1024) || 'Untitled',
      depth: clamp(depth, 0, 12)
    })
    for (const child of (Array.isArray(item?.subitems) ? item.subitems : []).toReversed())
      pending.push({ item: child, depth: depth + 1 })
  }
  return result
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

export function normalizeEpubTypography(value = {}) {
  const fontFamily = String(
    value.fontFamily || 'Georgia, "Noto Serif CJK SC", "Source Han Serif SC", serif'
  )
    .replace(/!important/giu, '')
    .replace(/[;{}]/gu, '')
    .trim()
    .slice(0, 512)
  const fontSize = Number(value.fontSize)
  const fontWeight = Number(value.fontWeight)
  return {
    fontFamily: fontFamily || 'Georgia, "Noto Serif CJK SC", "Source Han Serif SC", serif',
    fontSize: Number.isFinite(fontSize) ? clamp(fontSize, 11, 24) : 15,
    fontWeight: [400, 500, 600].includes(fontWeight) ? fontWeight : 400
  }
}

export async function createEpubReader({
  container,
  source,
  resourceRoot,
  signal,
  initialState,
  typography,
  translate = (value) => value,
  onSelection = () => {},
  onRelocate = () => {},
  onExternalLink = () => {},
  onError = () => {}
} = {}) {
  if (!(container instanceof HTMLElement)) throw new Error('EPUB reader requires a container')
  const sourceUrl = new URL(source)
  const scope = new URL(resourceRoot)
  if (
    !scope.href.endsWith('/') ||
    sourceUrl.origin !== location.origin ||
    !sourceUrl.href.startsWith(scope.href)
  )
    throw Error('EPUB source must use its granted resource scope')
  signal?.throwIfAborted()

  const readerState = normalizeEpubReaderState(initialState)
  const readingTypography = normalizeEpubTypography(typography)
  const shell = buildShell(container, translate)
  const requests = new AbortController()
  const request = async (input, type, overrideSignal) => {
    signal?.throwIfAborted()
    requests.signal.throwIfAborted()
    const url = new URL(input, sourceUrl)
    if (url.origin !== scope.origin || !url.href.startsWith(scope.href))
      throw Error('EPUB resource escaped its granted scope')
    let response
    for (let attempt = 0; attempt < 5; attempt++) {
      response = await fetch(url, {
        signal:
          overrideSignal instanceof AbortSignal
            ? AbortSignal.any([requests.signal, overrideSignal])
            : requests.signal
      })
      if (response.status !== 429) break
      await new Promise((resolve, reject) => {
        const operation =
          overrideSignal instanceof AbortSignal
            ? AbortSignal.any([requests.signal, overrideSignal])
            : requests.signal
        const done = () => {
          clearTimeout(timer)
          operation.removeEventListener('abort', cancel)
        }
        const cancel = () => {
          done()
          reject(new DOMException('EPUB resource cancelled', 'AbortError'))
        }
        const timer = setTimeout(
          () => {
            done()
            resolve()
          },
          50 * 2 ** attempt
        )
        operation.addEventListener('abort', cancel, { once: true })
        if (operation.aborted) cancel()
      })
    }
    if (!response.ok) throw Error(`EPUB resource failed (${response.status})`)
    if (type === 'blob') return response.blob()
    if (type === 'binary') return response.arrayBuffer()
    const text = await response.text()
    if (type === 'json') return JSON.parse(text)
    if (type === 'text' || /\.css$/i.test(url.pathname)) return text
    const navigationPath = book.packaging?.navPath
    const chapter =
      /\.(?:xhtml|html|htm)$/i.test(url.pathname) &&
      (!navigationPath || new URL(navigationPath, sourceUrl).pathname !== url.pathname)
    // CFIs are generated in the HTML tree rendered by the iframe. Parse chapters
    // with the same browser normalization (including implicit table sections).
    const document = new DOMParser().parseFromString(
      text,
      chapter ? 'text/html' : 'application/xml'
    )
    if (document.querySelector('parsererror')) throw Error('EPUB contains malformed XML')
    if (chapter) sanitizeDocument(document, url)
    return document
  }
  let book
  try {
    book = ePub(sourceUrl.href, { replacements: 'none', requestMethod: request })
  } catch (error) {
    requests.abort()
    shell.labels.dispose()
    shell.root.remove()
    throw error
  }
  // All member URLs are resolved under the read-only capability before render.
  // Keep the parent's base-uri boundary instead of injecting a new document base.
  book.spine.hooks.content.deregister(replaceBase)
  let rendition = null
  let viewLifetime = new AbortController()
  let destroyed = false,
    opened = false
  let currentToc = []
  let flatToc = []
  let currentHref = ''
  let locationGeneration = 0
  let progressController = null
  let progressState = {
    status: 'idle',
    chapters: 0,
    scanned: 0,
    locations: 0,
    textUtf16: 0,
    error: null
  }
  let lastSelection = null
  let selectionBoundaries = new WeakMap()
  const lifetime = new AbortController()
  const destroy = () => {
    if (destroyed) return
    destroyed = true
    locationGeneration++
    progressController?.abort()
    requests.abort()
    shell.labels.dispose()
    lifetime.abort()
    viewLifetime.abort()
    clearInterval(selectionPoll)
    signal?.removeEventListener('abort', destroy)
    rendition?.destroy()
    book.destroy()
    shell.root.remove()
  }
  signal?.addEventListener('abort', destroy, { once: true })
  function chapterTarget(value) {
    const target = new URL(value, sourceUrl)
    if (target.origin !== scope.origin || !target.href.startsWith(scope.href))
      throw Error('EPUB chapter escaped its granted scope')
    const section = book.spine.spineItems.find(
      (item) => new URL(item.url, sourceUrl).pathname === target.pathname
    )
    if (!section) throw Error('EPUB chapter is not in the spine')
    return section.href + target.hash
  }
  async function awaitBounded(promise, label, milliseconds = 20000) {
    lifetime.signal.throwIfAborted()
    let timer, cancelled
    try {
      return await Promise.race([
        promise,
        new Promise((_, reject) => {
          cancelled = () => reject(new DOMException('EPUB reader disposed', 'AbortError'))
          lifetime.signal.addEventListener('abort', cancelled, { once: true })
          timer = setTimeout(() => {
            reject(Error(`${label} exceeded ${milliseconds / 1000} seconds`))
            destroy()
          }, milliseconds)
        })
      ])
    } finally {
      clearTimeout(timer)
      lifetime.signal.removeEventListener('abort', cancelled)
    }
  }

  const notifyRelocate = () => {
    if (destroyed) return
    onRelocate({
      ...readerState,
      href: currentHref,
      typography: { ...readingTypography },
      progressPrecision: progressState.status === 'ready' ? 'indexed' : 'chapter',
      progressIndex: { ...progressState },
      progressProfile: EPUB_PROGRESS_PROFILE
    })
  }

  function updateProgressControls() {
    const indexing = progressState.status === 'indexing',
      indexed = progressState.status === 'ready'
    shell.labels.set(
      shell.progressBuild,
      indexing ? 'Cancel indexing' : indexed ? 'Progress indexed' : 'Build progress index'
    )
    shell.progressBuild.disabled = !opened || indexed
    shell.progressSeek.disabled = !indexed
    shell.progressSeek.value = Math.round(readerState.progress * 1000)
    shell.labels.set(
      shell.progressSeek,
      indexed ? 'Indexed reading progress' : 'Build a progress index in reading settings',
      {},
      'title'
    )
    if (indexing)
      shell.labels.set(shell.progressStatus, '{scanned} / {chapters}', {
        scanned: progressState.scanned,
        chapters: progressState.chapters
      })
    else {
      shell.labels.clear(shell.progressStatus)
      shell.progressStatus.textContent = progressState.error || ''
    }
  }

  async function buildProgressIndex({ signal: operationSignal } = {}) {
    signal?.throwIfAborted()
    lifetime.signal.throwIfAborted()
    operationSignal?.throwIfAborted()
    if (progressController)
      throw Object.assign(Error('EPUB progress indexing is already running'), { code: 'BUSY' })
    if (progressState.status === 'ready') return { ...progressState }
    const chapters = book.spine.spineItems.filter((section) => section.linear)
    const controller = new AbortController(),
      generation = ++locationGeneration
    progressController = controller
    const operation = AbortSignal.any([
      lifetime.signal,
      controller.signal,
      ...(operationSignal ? [operationSignal] : [])
    ])
    const started = performance.now(),
      positions = []
    let textUtf16 = 0,
      cfiBytes = 0,
      deadline = false
    const timer = setTimeout(() => {
      deadline = true
      controller.abort()
    }, EPUB_PROGRESS_PROFILE.deadlineMs)
    const checkpoint = () => {
      operation.throwIfAborted()
      if (performance.now() - started > EPUB_PROGRESS_PROFILE.deadlineMs) {
        deadline = true
        controller.abort()
        operation.throwIfAborted()
      }
    }
    progressState = {
      status: 'indexing',
      chapters: chapters.length,
      scanned: 0,
      locations: 0,
      textUtf16: 0,
      error: null
    }
    updateProgressControls()
    notifyRelocate()
    try {
      if (chapters.length > EPUB_PROGRESS_PROFILE.maxChapters)
        throw Object.assign(
          Error(
            'This book has too many chapters for a full progress index; use Contents or a saved CFI'
          ),
          { code: 'PROGRESS_BUDGET' }
        )
      for (const section of chapters) {
        checkpoint()
        const contents = await section.load((url, type) => request(url, type, operation))
        try {
          checkpoint()
          const body = section.document?.body
          if (!body) throw Error('EPUB progress chapter has no document body')
          for (let ancestor = body; ancestor; ancestor = ancestor.parentElement)
            if (ancestor.id?.length > EPUB_PROGRESS_PROFILE.maxIdentifierUtf16)
              throw Object.assign(
                Error('This chapter has identifiers too large for a bounded progress index'),
                { code: 'PROGRESS_BUDGET' }
              )
          const walker = section.document.createTreeWalker(
            body,
            NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT
          )
          let node,
            nodes = 0,
            units = 0
          while ((node = walker.nextNode())) {
            if (++nodes > EPUB_PROGRESS_PROFILE.maxChapterNodes)
              throw Object.assign(Error('This chapter is too complex for a full progress index'), {
                code: 'PROGRESS_BUDGET'
              })
            if (node.nodeType === Node.TEXT_NODE) units += node.length
            else {
              if (node.id?.length > EPUB_PROGRESS_PROFILE.maxIdentifierUtf16)
                throw Object.assign(
                  Error('This chapter has identifiers too large for a bounded progress index'),
                  { code: 'PROGRESS_BUDGET' }
                )
              let parent = node,
                depth = 0
              while ((parent = parent.parentElement))
                if (++depth > EPUB_PROGRESS_PROFILE.maxDepth)
                  throw Object.assign(
                    Error('This chapter is too deeply nested for a progress index'),
                    { code: 'PROGRESS_BUDGET' }
                  )
            }
            if (
              units > EPUB_PROGRESS_PROFILE.maxChapterUtf16 ||
              textUtf16 + units > EPUB_PROGRESS_PROFILE.maxTotalUtf16
            )
              throw Object.assign(
                Error(
                  'This book is too large for a full progress index; Contents and saved CFIs remain available'
                ),
                { code: 'PROGRESS_BUDGET' }
              )
          }
          checkpoint()
          // Reuse epub.js's CFI algorithm without its unbounded whole-spine queue.
          // Commit the complete candidate only after every chapter passes.
          const chapterPositions = book.locations.parse(
            contents,
            section.cfiBase,
            EPUB_PROGRESS_PROFILE.chunkUtf16
          )
          for (const value of chapterPositions) {
            const cfi = checkedCfi(value)
            cfiBytes += new TextEncoder().encode(cfi).length
            if (
              positions.length >= EPUB_PROGRESS_PROFILE.maxLocations ||
              cfiBytes > EPUB_PROGRESS_PROFILE.maxCfiBytes
            )
              throw Object.assign(Error('The book progress index exceeds its location budget'), {
                code: 'PROGRESS_BUDGET'
              })
            positions.push(cfi)
          }
          textUtf16 += units
          checkpoint()
          progressState = {
            ...progressState,
            scanned: progressState.scanned + 1,
            locations: positions.length,
            textUtf16
          }
          updateProgressControls()
          notifyRelocate()
        } finally {
          if (
            !(rendition?.getContents() || []).some(
              (contents) => contents.sectionIndex === section.index
            )
          )
            section.unload()
        }
        await new Promise((resolve) => setTimeout(resolve, 0))
      }
      checkpoint()
      if (!positions.length) throw Error('This book has no text locations to index')
      book.locations.load(positions)
      progressState = {
        ...progressState,
        status: 'ready',
        elapsedMs: performance.now() - started,
        cfiBytes
      }
      const percentage = book.locations.percentageFromCfi(readerState.cfi)
      if (Number.isFinite(percentage)) readerState.progress = clamp(percentage, 0, 1)
      renderPosition(shell, readerState, translate)
      return { ...progressState }
    } catch (error) {
      if (!destroyed && generation === locationGeneration)
        progressState = {
          ...progressState,
          status: operation.aborted && !deadline ? 'cancelled' : 'error',
          error: deadline
            ? 'Book progress indexing timed out; use Contents or a saved CFI'
            : operation.aborted
              ? null
              : String(error.message || error).slice(0, 800)
        }
      if (deadline) throw Object.assign(Error(progressState.error), { code: 'PROGRESS_TIMEOUT' })
      throw error
    } finally {
      clearTimeout(timer)
      if (progressController === controller) progressController = null
      if (!destroyed) {
        updateProgressControls()
        notifyRelocate()
      }
    }
  }

  async function seekProgress(percentage) {
    if (!Number.isFinite(percentage) || percentage < 0 || percentage > 1)
      throw Error('EPUB progress must be between 0 and 1')
    if (progressState.status !== 'ready')
      throw Object.assign(Error('Build the book progress index before seeking by percentage'), {
        code: 'PROGRESS_INDEX_REQUIRED'
      })
    await displayTarget(checkedCfi(book.locations.cfiFromPercentage(percentage)))
  }

  const applyTheme = () => {
    if (!rendition) return
    const palette = epubPaperTheme(readerState.theme)
    const rules = {
      'html, body': {
        background: `${palette.background} !important`,
        color: `${palette.foreground} !important`
      },
      body: {
        'font-family': `${readingTypography.fontFamily} !important`,
        'font-size': `${readingTypography.fontSize * readerState.fontScale}px !important`,
        'font-weight': `${readingTypography.fontWeight} !important`,
        'line-height': '1.72 !important',
        'padding-left': 'clamp(16px, 5vw, 54px) !important',
        'padding-right': 'clamp(16px, 5vw, 54px) !important'
      },
      a: { color: `${palette.link} !important` },
      'img, svg': { 'max-width': '100% !important', height: 'auto !important' },
      'pre, code': { 'white-space': 'pre-wrap !important', 'overflow-wrap': 'anywhere !important' },
      '::selection': { background: 'rgba(19, 124, 112, .28) !important' }
    }
    rendition.themes.register('studio-reader', rules)
    rendition.themes.select('studio-reader')
    shell.root.dataset.paper = readerState.theme
  }

  function sanitizeDocument(document, url) {
    if (!document) return
    document
      .querySelectorAll(
        'script, iframe, frame, object, embed, form, input, button, textarea, select, base, meta[http-equiv="refresh" i]'
      )
      .forEach((node) => node.remove())
    document.querySelectorAll('*').forEach((element) => {
      for (const attribute of [...element.attributes]) {
        if (
          /^on/iu.test(attribute.name) ||
          attribute.name === 'contenteditable' ||
          attribute.name === 'srcdoc' ||
          (/^(?:href|src|xlink:href|formaction)$/iu.test(attribute.name) &&
            /^\s*(?:javascript|file):/iu.test(attribute.value))
        ) {
          element.removeAttribute(attribute.name)
        }
      }
      for (const name of ['src', 'poster', 'href', 'xlink:href']) {
        const raw = element.getAttribute(name)
        if (!raw || raw.startsWith('#') || raw.startsWith('data:')) continue
        const target = new URL(raw, url)
        if (element.tagName.toLowerCase() === 'a' && /^https?:/i.test(raw)) continue
        if (target.origin !== scope.origin || !target.href.startsWith(scope.href))
          element.removeAttribute(name)
        else element.setAttribute(name, target.href)
      }
    })
    // Head-only policy insertion leaves the body CFI tree identical to Studio's
    // existing sanitizer. No table wrappers or generated body nodes are inserted.
    const policy = document.createElement('meta')
    policy.setAttribute('http-equiv', 'Content-Security-Policy')
    policy.setAttribute(
      'content',
      "default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data: blob:; connect-src 'none'; form-action 'none'; object-src 'none'"
    )
    document.querySelector('head')?.prepend(policy)
  }
  const sanitizeContents = (contents) => {
    const document = contents?.document
    if (!document) return
    const section = book.spine.get(contents.sectionIndex)
    const label =
      flatToc.find((item) => stripFragment(item.href) === stripFragment(section?.href))?.label ||
      `Chapter ${(contents.sectionIndex || 0) + 1}`
    if (contents.window?.frameElement) contents.window.frameElement.title = label
    document.querySelectorAll('a[href]').forEach((anchor) => {
      const href = anchor.getAttribute('href') || ''
      anchor.addEventListener(
        'click',
        (event) => {
          event.preventDefault()
          event.stopImmediatePropagation()
          const target = new URL(href, section?.url || sourceUrl)
          if (target.origin !== scope.origin || !target.href.startsWith(scope.href))
            onExternalLink(href)
          else displayTarget(chapterTarget(target.href)).catch(onError)
        },
        { signal: viewLifetime.signal, capture: true }
      )
    })
    contents.window?.addEventListener('keydown', handleKeydown, { signal: viewLifetime.signal })
    // Capture in the owning reader realm. WebKit can clear an iframe selection
    // before epub.js's delayed selected event reaches the parent rendition.
    contents.document.addEventListener('selectionchange', () => selected(null, contents), {
      signal: viewLifetime.signal
    })
    contents.document.addEventListener('pointerup', () => selected(null, contents), {
      signal: viewLifetime.signal
    })
  }

  const selected = (cfiRange, contents) => {
    try {
      const selection = contents?.window?.getSelection?.()
      const rawQuote = String(selection?.toString() || '')
      const end = Math.min(rawQuote.length, MAX_SELECTION)
      const quote = rawQuote.slice(
        0,
        end && /[\uD800-\uDBFF]/.test(rawQuote[end - 1]) ? end - 1 : end
      )
      if (!quote.trim() || !selection.rangeCount) return
      const rangeRect = selection.getRangeAt(0).getBoundingClientRect()
      const sourceRange = selection.getRangeAt(0)
      let cfiSelection = sourceRange
      // epub.js treats an element child offset as a character offset. Normalize
      // element-boundary selections (Ctrl+A/triple-click) before creating the CFI.
      if (
        sourceRange.startContainer.nodeType !== Node.TEXT_NODE ||
        sourceRange.endContainer.nodeType !== Node.TEXT_NODE
      ) {
        const walker = contents.document.createTreeWalker(
          contents.document.body,
          NodeFilter.SHOW_TEXT
        )
        let node,
          first,
          last,
          examined = 0
        while ((node = walker.nextNode())) {
          if (++examined > 10000)
            throw Error('EPUB selection exceeds the 10,000-node traversal budget')
          if (!node.length || !sourceRange.intersectsNode(node)) continue
          const from = sourceRange.startContainer === node ? sourceRange.startOffset : 0
          const to = sourceRange.endContainer === node ? sourceRange.endOffset : node.length
          if (to <= from) continue
          first ||= { node, from }
          last = { node, to }
        }
        if (!first || !last) return
        cfiSelection = contents.document.createRange()
        cfiSelection.setStart(first.node, first.from)
        cfiSelection.setEnd(last.node, last.to)
      }
      cfiRange = contents.cfiFromRange(cfiSelection)
      const frameRect = contents.window.frameElement?.getBoundingClientRect?.() || {
        left: 0,
        top: 0
      }
      const captured = {
        quote,
        quoteTruncated: quote.length < rawQuote.length,
        cfiRange: checkedCfi(cfiRange),
        href: currentHref,
        chapterLabel: readerState.chapterLabel,
        rect: {
          left: frameRect.left + rangeRect.left,
          top: frameRect.top + rangeRect.top,
          width: rangeRect.width,
          height: rangeRect.height
        }
      }
      if (
        lastSelection?.cfiRange !== captured.cfiRange ||
        lastSelection?.quote !== captured.quote
      ) {
        lastSelection = captured
        onSelection(captured)
      }
    } catch (error) {
      onError(error)
    }
  }

  const relocated = (location) => {
    if (!location?.start) return
    if (currentHref && currentHref !== location.start.href) {
      lastSelection = null
      onSelection(null)
    }
    readerState.cfi = checkedCfi(location.start.cfi)
    currentHref = checkedText(location.start.href || '', 8192, 'EPUB chapter')
    readerState.chapterLabel =
      flatToc.find((item) => stripFragment(item.href) === stripFragment(currentHref))?.label ||
      readerState.chapterLabel
    const generated = Number(book.locations?.total) > 0
    const percentage = generated
      ? book.locations.percentageFromCfi(readerState.cfi)
      : location.start.percentage
    if (Number.isFinite(percentage)) readerState.progress = clamp(percentage, 0, 1)
    else {
      const section = book.spine.get(readerState.cfi),
        displayed = location.start.displayed
      readerState.progress = clamp(
        ((section?.index || 0) +
          Math.max(0, (displayed?.page || 1) - 1) / Math.max(1, displayed?.total || 1)) /
          Math.max(1, book.spine.length),
        0,
        1
      )
    }
    renderPosition(shell, readerState, translate)
    updateProgressControls()
    notifyRelocate()
  }

  async function reportPosition() {
    const updated = new Promise((resolve) => rendition.once('relocated', resolve))
    await rendition.reportLocation()
    await awaitBounded(updated, 'EPUB position update')
  }
  async function displayTarget(target) {
    await awaitBounded(rendition.display(target), 'EPUB navigation')
    await reportPosition()
  }

  const mountRendition = async (target = '') => {
    lastSelection = null
    selectionBoundaries = new WeakMap()
    viewLifetime.abort()
    viewLifetime = new AbortController()
    rendition?.destroy()
    shell.viewer.replaceChildren()
    rendition = book.renderTo(shell.viewer, {
      width: '100%',
      height: '100%',
      manager: readerState.flow === 'scrolled' ? 'continuous' : 'default',
      flow: readerState.flow === 'scrolled' ? 'scrolled' : 'paginated',
      spread: 'none',
      allowScriptedContent: false,
      allowPopups: false
    })
    const mounted = rendition
    mounted.hooks.content.register(sanitizeContents)
    const locationChanged = (location) => {
      if (!destroyed && rendition === mounted) relocated(location)
    }
    mounted.on('relocated', locationChanged)
    viewLifetime.signal.addEventListener('abort', () => mounted.off('relocated', locationChanged), {
      once: true
    })
    applyTheme()
    const rendered = new Promise((resolve) => rendition.once('rendered', resolve))
    await awaitBounded(
      Promise.all([rendition.display(target || undefined), rendered]),
      'EPUB chapter display'
    )
    await reportPosition()
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
    if (
      event.key !== 'Escape' &&
      event.target?.closest?.('input,textarea,select,[contenteditable]')
    )
      return
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

  shell.root.addEventListener('keydown', handleKeydown, { signal: lifetime.signal })
  shell.previous.addEventListener('click', () => rendition?.prev(), { signal: lifetime.signal })
  shell.next.addEventListener('click', () => rendition?.next(), { signal: lifetime.signal })
  shell.progressBuild.addEventListener(
    'click',
    () => {
      if (progressController) progressController.abort()
      else
        void buildProgressIndex().catch((error) => {
          if (error.name !== 'AbortError') onError(error)
        })
    },
    { signal: lifetime.signal }
  )
  shell.progressSeek.addEventListener(
    'change',
    () => {
      void seekProgress(Number(shell.progressSeek.value) / 1000)
        .catch(onError)
        .finally(() => {
          if (!destroyed) updateProgressControls()
        })
    },
    { signal: lifetime.signal }
  )
  updateProgressControls()
  shell.settingsToggle.addEventListener('click', () => shell.settings.classList.toggle('hidden'))
  shell.fontDecrease.addEventListener('click', () => changeFont(-0.1))
  shell.fontIncrease.addEventListener('click', () => changeFont(0.1))
  shell.settings.addEventListener('click', (event) => {
    const theme = event.target.closest('[data-epub-theme]')?.dataset.epubTheme
    const flow = event.target.closest('[data-epub-flow]')?.dataset.epubFlow
    if (theme) changeTheme(theme)
    if (flow) changeFlow(flow).catch(() => {})
  })
  // Some WebKit iframe selections do not deliver selectionchange outside the
  // script-disabled book document. Poll only the focused visible reader, compare
  // boundary identities first, and never traverse unchanged text/CFIs.
  const selectionPoll = setInterval(() => {
    if (
      destroyed ||
      document.hidden ||
      !shell.root.isConnected ||
      document.activeElement?.tagName !== 'IFRAME'
    )
      return
    for (const contents of rendition?.getContents() || []) {
      const value = contents.window?.getSelection()
      if (!value?.rangeCount || value.isCollapsed) continue
      const previous = selectionBoundaries.get(contents)
      if (
        previous?.anchor === value.anchorNode &&
        previous?.focus === value.focusNode &&
        previous?.from === value.anchorOffset &&
        previous?.to === value.focusOffset
      )
        continue
      selectionBoundaries.set(contents, {
        anchor: value.anchorNode,
        focus: value.focusNode,
        from: value.anchorOffset,
        to: value.focusOffset
      })
      selected(null, contents)
    }
  }, 150)
  try {
    const [, navigation, metadata] = await awaitBounded(
      Promise.all([book.ready, book.loaded.navigation, book.loaded.metadata]),
      'EPUB opening',
      30000
    )
    if (destroyed) throw new Error('EPUB reader was closed while loading')
    currentToc = navigation?.toc || []
    flatToc = flattenEpubToc(currentToc).map((item) => ({
      ...item,
      href: chapterTarget(item.href)
    }))
    const bookTitle = displayMetadata(metadata?.title)
    if (bookTitle) shell.bookTitle.textContent = bookTitle
    else shell.labels.set(shell.bookTitle, 'E-book')
    shell.bookAuthor.textContent = displayMetadata(metadata?.creator)
    updateSettings(shell, readerState)
    await mountRendition(readerState.cfi)
    opened = true
    updateProgressControls()
  } catch (error) {
    destroy()
    throw error
  }

  return {
    title: shell.bookTitle.textContent,
    state: () => ({
      ...readerState,
      href: currentHref,
      typography: { ...readingTypography },
      progressPrecision: progressState.status === 'ready' ? 'indexed' : 'chapter',
      progressIndex: { ...progressState },
      progressProfile: EPUB_PROGRESS_PROFILE
    }),
    selection: () => {
      for (const contents of rendition?.getContents() || []) selected(null, contents)
      return lastSelection
    },
    outline: () =>
      flatToc.map((item, index) => ({
        id: `epub-${index + 1}`,
        label: item.label,
        depth: item.depth,
        target: { kind: 'epub', href: item.href }
      })),
    display: (target) => {
      signal?.throwIfAborted()
      return displayTarget(
        String(target).startsWith('epubcfi(') ? checkedCfi(target) : chapterTarget(target)
      )
    },
    quoteForCfi: async (cfi) => {
      checkedCfi(cfi)
      const range = await awaitBounded(book.getRange(cfi), 'EPUB CFI resolution')
      const quote = range.toString()
      if (quote.length > MAX_SELECTION) throw Error('EPUB CFI quote exceeds its text budget')
      return quote
    },
    async search(query, { signal: operationSignal } = {}) {
      if (typeof query !== 'string' || !query || new TextEncoder().encode(query).length > 256)
        throw Error('EPUB search requires 1–256 UTF-8 bytes')
      const matches = [],
        pattern = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gu')
      const started = performance.now()
      for (const section of book.spine.spineItems) {
        lifetime.signal.throwIfAborted()
        operationSignal?.throwIfAborted()
        if (performance.now() - started > 30000) throw Error('EPUB search exceeded 30 seconds')
        await awaitBounded(
          section.load((url, type) => request(url, type, operationSignal)),
          'EPUB search chapter'
        )
        try {
          const nodes = [],
            parts = [],
            walker = section.document.createTreeWalker(section.document.body, NodeFilter.SHOW_TEXT)
          let node,
            length = 0,
            previousBlock
          while ((node = walker.nextNode())) {
            if (node.parentElement?.closest('style,script,noscript') || !node.length) continue
            if (nodes.length >= 10000 || length + node.length > 2 * 1024 * 1024)
              throw Error('EPUB search chapter exceeds its node/text budget')
            const block = node.parentElement?.closest('p,div,li,h1,h2,h3,h4,h5,h6,td,th')
            if (nodes.length && block !== previousBlock) {
              parts.push('\n')
              length++
            }
            nodes.push({ node, start: length })
            parts.push(node.textContent)
            length += node.length
            previousBlock = block
          }
          const text = parts.join('')
          pattern.lastIndex = 0
          const boundary = (offset) => {
            let left = 0,
              right = nodes.length
            while (left + 1 < right) {
              const middle = (left + right) >> 1
              if (nodes[middle].start <= offset) left = middle
              else right = middle
            }
            const value = nodes[left]
            return {
              node: value.node,
              offset: Math.max(0, Math.min(value.node.length, offset - value.start))
            }
          }
          let match
          while (nodes.length && (match = pattern.exec(text))) {
            const from = boundary(match.index),
              to = boundary(match.index + match[0].length),
              range = section.document.createRange()
            range.setStart(from.node, from.offset)
            range.setEnd(to.node, to.offset)
            matches.push({
              cfi: checkedCfi(section.cfiFromRange(range)),
              href: section.href,
              excerpt: text.slice(Math.max(0, match.index - 80), match.index + match[0].length + 80)
            })
            if (matches.length >= 500) return { matches, complete: false, limit: 500 }
          }
        } finally {
          if (
            !(rendition?.getContents() || []).some(
              (contents) => contents.sectionIndex === section.index
            )
          )
            section.unload()
        }
        await new Promise((resolve) => setTimeout(resolve, 0))
      }
      return { matches, complete: true, limit: 500 }
    },
    settings: async (patch) => {
      if (patch.typography) {
        Object.assign(
          readingTypography,
          normalizeEpubTypography({ ...readingTypography, ...patch.typography })
        )
        applyTheme()
        notifyRelocate()
      }
      if (patch.flow) await changeFlow(patch.flow)
      if (patch.theme) changeTheme(patch.theme)
      if (patch.fontScale != null) {
        readerState.fontScale = clamp(Number(patch.fontScale), 0.75, 2)
        applyTheme()
        notifyRelocate()
      }
    },
    buildProgressIndex,
    cancelProgressIndex: () => progressController?.abort(),
    seekProgress,
    destroy
  }
}

function buildShell(container, translate) {
  const labels = uiLabels(translate)
  const wrapper = document.createElement('div')
  wrapper.innerHTML = `<section class="epub-reader" tabindex="0">
    <header class="epub-toolbar">
      <div class="epub-book-identity"><strong></strong><small></small></div>
      <div class="epub-page-actions">
        <button class="epub-tool-button epub-previous" type="button" title="${escapeHtml(translate('Previous'))}" aria-label="${escapeHtml(translate('Previous'))}">←</button>
        <button class="epub-tool-button epub-next" type="button" title="${escapeHtml(translate('Next'))}" aria-label="${escapeHtml(translate('Next'))}">→</button>
        <button class="epub-tool-button epub-settings-toggle" type="button" title="${escapeHtml(translate('Reading settings'))}" aria-label="${escapeHtml(translate('Reading settings'))}">Aa</button>
      </div>
      <div class="epub-settings-popover hidden">
        <div class="epub-setting-row"><span>${escapeHtml(translate('Type size'))}</span><div><button class="epub-font-decrease" type="button">A−</button><output class="epub-font-value">100%</output><button class="epub-font-increase" type="button">A+</button></div></div>
        <div class="epub-setting-row"><span>${escapeHtml(translate('Paper'))}</span><div class="epub-setting-options"><button data-epub-theme="light" type="button">${escapeHtml(translate('Bright'))}</button><button data-epub-theme="sepia" type="button">${escapeHtml(translate('Sepia'))}</button><button data-epub-theme="dark" type="button">${escapeHtml(translate('Dark'))}</button></div></div>
        <div class="epub-setting-row"><span>${escapeHtml(translate('Reading flow'))}</span><div class="epub-setting-options"><button data-epub-flow="paginated" type="button">${escapeHtml(translate('Pages'))}</button><button data-epub-flow="scrolled" type="button">${escapeHtml(translate('Scroll'))}</button></div></div>
        <div class="epub-setting-row epub-progress-settings"><span>${escapeHtml(translate('Book progress'))}</span><button class="epub-progress-build" type="button">${escapeHtml(translate('Build progress index'))}</button><small class="epub-progress-status" role="status"></small></div>
      </div>
    </header>
    <div class="epub-reader-body">
      <div class="epub-viewer"></div>
    </div>
    <footer class="epub-reader-footer"><span class="epub-chapter"></span><input class="epub-progress-seek" type="range" min="0" max="1000" step="1" value="0" disabled aria-label="${escapeHtml(translate('Reading progress'))}"><span class="epub-progress">0%</span></footer>
  </section>`
  const root = wrapper.firstElementChild
  const style = document.createElement('link')
  style.rel = 'stylesheet'
  style.href = new URL('./epub-reader.css', import.meta.url)
  root.prepend(style)
  for (const [selector, key] of [
    ['.epub-previous', 'Previous'],
    ['.epub-next', 'Next'],
    ['.epub-settings-toggle', 'Reading settings'],
    ['.epub-font-decrease', 'Decrease type size'],
    ['.epub-font-increase', 'Increase type size']
  ]) {
    const button = root.querySelector(selector)
    labels.set(button, key, {}, 'title')
    labels.set(button, key, {}, 'aria-label')
  }
  const settingLabels = ['Type size', 'Paper', 'Reading flow', 'Book progress']
  root
    .querySelectorAll('.epub-setting-row > span')
    .forEach((node, index) => labels.set(node, settingLabels[index]))
  for (const [selector, key] of [
    ['[data-epub-theme="light"]', 'Bright'],
    ['[data-epub-theme="sepia"]', 'Sepia'],
    ['[data-epub-theme="dark"]', 'Dark'],
    ['[data-epub-flow="paginated"]', 'Pages'],
    ['[data-epub-flow="scrolled"]', 'Scroll'],
    ['.epub-progress-build', 'Build progress index']
  ])
    labels.set(root.querySelector(selector), key)
  labels.set(root.querySelector('.epub-progress-seek'), 'Reading progress', {}, 'aria-label')
  container.append(root)
  return {
    labels,
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
    progressBuild: root.querySelector('.epub-progress-build'),
    progressStatus: root.querySelector('.epub-progress-status'),
    progressSeek: root.querySelector('.epub-progress-seek'),
    bookTitle: root.querySelector('.epub-book-identity strong'),
    bookAuthor: root.querySelector('.epub-book-identity small')
  }
}

function renderPosition(shell, state, translate) {
  if (state.chapterLabel) {
    shell.labels.clear(shell.chapter)
    shell.chapter.textContent = state.chapterLabel
  } else shell.labels.set(shell.chapter, 'Reading')
  shell.labels.set(shell.progress, '{percent}%', {
    percent: Math.round(clamp(state.progress, 0, 1) * 100)
  })
}

function updateSettings(shell, state) {
  shell.labels.set(shell.fontValue, '{percent}%', { percent: Math.round(state.fontScale * 100) })
  shell.settings
    .querySelectorAll('[data-epub-theme]')
    .forEach((button) =>
      button.classList.toggle('active', button.dataset.epubTheme === state.theme)
    )
  shell.settings
    .querySelectorAll('[data-epub-flow]')
    .forEach((button) => button.classList.toggle('active', button.dataset.epubFlow === state.flow))
}

function displayMetadata(value) {
  if (Array.isArray(value)) return value.map(displayMetadata).filter(Boolean).join(', ')
  if (value && typeof value === 'object')
    return displayMetadata(value.name || Object.values(value)[0])
  return bounded(value, 1024)
}

function stripFragment(value) {
  return String(value || '')
    .split('#')[0]
    .replace(/^\.\//u, '')
}

function bounded(value, limit) {
  return String(value || '')
    .trim()
    .slice(0, limit)
}

function checkedText(value, limit, label) {
  if (
    typeof value !== 'string' ||
    value.includes('\0') ||
    new TextEncoder().encode(value).length > limit
  )
    throw Error(`${label} exceeds its UTF-8 byte budget`)
  return value
}
function checkedCfi(value) {
  checkedText(value, 8192, 'EPUB CFI')
  if (value && (!value.startsWith('epubcfi(') || !value.endsWith(')') || /[\r\n]/.test(value)))
    throw Error('Invalid EPUB CFI; retain the original position')
  return value
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value))
}

function escapeHtml(value) {
  return String(value || '').replace(
    /[&<>"']/gu,
    (character) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]
  )
}
