export async function imageViewContract(moduleUrl) {
  const { mountImageView, readerImageProvider } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const owner = { documentId: 'first', revision: 'a'.repeat(64) }
  const literal = 'Fit <img onerror="forbidden()">'
  const metadata = {
    id: 0,
    revision: owner.revision,
    alt: literal,
    width: 2,
    height: 1,
    frames: [{ frame: 0, delayMs: 20 }],
    plays: 1,
    dimensionsExact: true,
    pdfPage: null
  }
  const png = new Blob(
    [
      Uint8Array.from(
        atob(
          'iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII='
        ),
        (c) => c.charCodeAt(0)
      )
    ],
    { type: 'image/png' }
  )
  const views = [],
    urls = new Set(),
    viewport = document.createElement('div'),
    root = document.createElement('div'),
    neighbor = document.createElement('span')
  viewport.style.cssText = 'height:300px;width:380px;overflow:auto'
  neighbor.textContent = 'Retained host content'
  root.append(neighbor)
  viewport.append(root)
  document.body.append(viewport)
  const create = URL.createObjectURL,
    revoke = URL.revokeObjectURL
  URL.createObjectURL = (blob) => {
    const url = create.call(URL, blob)
    urls.add(url)
    return url
  }
  URL.revokeObjectURL = (url) => {
    urls.delete(url)
    revoke.call(URL, url)
  }
  let assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const wait = async (predicate) => {
    const deadline = performance.now() + 5000
    while (!predicate()) {
      if (performance.now() > deadline) throw Error('Image contract timed out')
      await new Promise((resolve) => setTimeout(resolve, 20))
    }
  }
  const make = (options) => {
    const view = mountImageView(root, {
      owner,
      metadata,
      frame: async () => png,
      viewport,
      ...options
    })
    views.push(view)
    return view
  }
  try {
    let requests = [],
      changes = 0
    const translate = createTranslator()
    const first = make({
      translate,
      onChange() {
        changes++
      },
      frame: async (request) => {
        requests.push(request)
        return png
      }
    })
    await first.ready
    check(first.root.querySelector('img').naturalWidth === 2, 'PNG frame did not render')
    check(
      requests[0].documentId === owner.documentId &&
        requests[0].revision === owner.revision &&
        requests[0].id === 0,
      'Frame provider lost owning scope'
    )
    check(
      first.root.querySelector('figcaption').textContent === literal &&
        !first.root.querySelector('figcaption img'),
      'Image alt text became markup'
    )
    translate.update({ locale: 'fr', messages: { Fit: 'Adapter' } })
    check(
      first.root.querySelector('button').textContent === 'Adapter' &&
        first.root.querySelector('figcaption').textContent === literal,
      'Translation changed document data'
    )
    const second = make({ owner: { ...owner, documentId: 'second' } })
    await second.ready
    check(
      root.querySelectorAll('.asset-controls').length === 2 &&
        first.root.querySelectorAll('.asset-controls').length === 1,
      "Image view mounted another owner's controls"
    )
    const plus = first.root.querySelectorAll('button')[2]
    plus.click()
    check(
      first.state().zoom === 1.25 && second.state().zoom === null,
      'Zoom leaked into the other image'
    )
    const before = changes
    first.dispose()
    plus.click()
    check(
      changes === before && !first.root.querySelector('img'),
      'Disposed image retained callbacks or bitmaps'
    )
    second.dispose()
    check(
      root.firstChild === neighbor && root.children.length === 1 && urls.size === 0,
      'Dispose removed host content or retained object URLs'
    )
    const beforeInvalid = root.children.length
    let invalid = false
    try {
      make({ metadata: { ...metadata, revision: 'b'.repeat(64) } })
    } catch {
      invalid = true
    }
    check(invalid && root.children.length === beforeInvalid, 'Stale metadata mounted DOM')
    invalid = false
    try {
      make({ metadata: { ...metadata, width: 32768, height: 32768 } })
    } catch {
      invalid = true
    }
    check(invalid, 'Unbounded pixel metadata was accepted')
    const malformed = make({ frame: async () => new Blob(['not a PNG'], { type: 'image/png' }) })
    check(
      await malformed.ready.then(
        () => false,
        () => true
      ),
      'Malformed PNG did not reject readiness'
    )
    malformed.dispose()
    check(urls.size === 0, 'Failed decoding retained a candidate object URL')
    let signal,
      finish,
      lateChanges = 0
    const deferred = make({
      onChange() {
        lateChanges++
      },
      frame: (request) => {
        signal = request.signal
        return new Promise((resolve) => {
          finish = resolve
        })
      }
    })
    await wait(() => Boolean(finish))
    deferred.dispose()
    check(
      signal.aborted &&
        (await deferred.ready.then(
          () => false,
          (error) => error.name === 'AbortError'
        )),
      'Closing did not cancel the owning frame request'
    )
    finish(png)
    await new Promise((resolve) => setTimeout(resolve, 50))
    check(
      lateChanges === 0 && !deferred.root.isConnected && urls.size === 0,
      'Late frame completion reopened a closed image'
    )
    const frames = []
    const animated = make({
      metadata: {
        ...metadata,
        alt: 'Animation',
        frames: [
          { frame: 0, delayMs: 200 },
          { frame: 1, delayMs: 20 }
        ]
      },
      frame: async (request) => {
        frames.push(request.frame)
        return png
      }
    })
    await animated.ready
    const pause = animated.root.querySelectorAll('button')[4]
    pause.click()
    await new Promise((resolve) => setTimeout(resolve, 250))
    check(animated.state().paused && frames.length === 1, 'Paused image kept requesting frames')
    pause.click()
    await wait(() => animated.state().frame === 1)
    check(
      !animated.state().paused && frames.includes(1),
      'Animation did not resume through the same controller'
    )
    animated.dispose()
    const compact = make({ compact: true })
    await compact.ready
    compact.root.style.width = '2px'
    compact.root.focus()
    const compactControls = compact.root.querySelector('.asset-controls')
    const compactBounds = compactControls.getBoundingClientRect(), viewportBounds = viewport.getBoundingClientRect()
    check(compact.root.tabIndex === 0 && compact.root.getAttribute('aria-label') === literal,
      'Compact image lost its keyboard focus or accessible name')
    check(getComputedStyle(compactControls).visibility === 'visible' &&
      compactBounds.left >= viewportBounds.left && compactBounds.right <= viewportBounds.right + 1 &&
      compactBounds.top >= viewportBounds.top && compactBounds.bottom <= viewportBounds.bottom + 1,
      'Compact controls were clipped outside the reading viewport')
    check(compact.root.querySelector('.document-image').getBoundingClientRect().height < 10,
      'Compact image retained the standalone minimum height')
    compact.dispose()
    let wrong = false
    try {
      await readerImageProvider({ request: async () => ({ ...metadata, id: 1 }) }, owner, 0)
    } catch {
      wrong = true
    }
    check(wrong, 'Provider accepted metadata from another asset')
    const provider = await readerImageProvider(
      {
        request: async (method) =>
          method === 'assetInfo'
            ? metadata
            : { revision: 'b'.repeat(64), id: 0, frame: 0, blob: png }
      },
      owner,
      0
    )
    check(
      await provider.frame({ frame: 0 }).then(
        () => false,
        () => true
      ),
      'Provider accepted a frame from another revision'
    )
    check(urls.size === 0, 'Final image URLs were not released')
    return { assertions, sharedController: true, ownerIsolation: true, cancellation: true }
  } finally {
    for (const view of views) view.dispose()
    URL.createObjectURL = create
    URL.revokeObjectURL = revoke
    viewport.remove()
  }
}
