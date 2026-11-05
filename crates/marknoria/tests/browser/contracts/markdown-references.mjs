// The same public client/component contract runs in Chromium and Wayland WebKit.
export async function markdownReferencesContract(urls) {
  const { createReaderClient } = await import('/host-client.mjs')
  const { mountReferences, readerReferenceProvider } = await import('/references.mjs')
  const { projectionHtml } = await import('/projection-html.mjs')
  const clients = [], frames = [], views = [], containers = []
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const wait = async predicate => {
    const deadline = performance.now() + 30000
    while (!predicate()) {
      if (performance.now() > deadline) throw Error('Markdown resource contract timed out')
      await new Promise(resolve => setTimeout(resolve, 25))
    }
  }
  const readyImage = async (client, id) => {
    const deadline = performance.now() + 30000
    while (performance.now() < deadline) {
      const result = await client.request('referenceImageInfo', { id })
      if (result.state === 'ready') return result
      if (result.state !== 'indexing') throw Error('Unknown image state')
      await new Promise(resolve => setTimeout(resolve, 30))
    }
    throw Error('Referenced image was not prepared')
  }
  try {
    for (const url of urls) {
      const frame = document.createElement('iframe')
      frame.title = 'Markdown resource reader'; frame.src = url
      frame.style.cssText = 'width:600px;height:240px;border:0;display:block'
      document.body.append(frame); frames.push(frame)
      clients.push(createReaderClient(frame))
    }
    const owners = await Promise.all(clients.map(client => client.ready()))
    check(owners[0].document.references === 13 && owners[1].document.references === 13, 'Reference count changed')
    check(owners[0].capabilities.referenceImages && owners[0].capabilities.referenceProfile.implicitFolderGrant === false, 'Reference authority profile missing')
    const refs = await Promise.all(Array.from({ length: 8 }, (_, id) => clients[1].request('referenceInfo', { id })))
    check(refs[0].reference.target === '中文%20pic.png' && refs[0].reference.startByte >= 3, 'Local image source range or URL changed')
    check(refs[1].reference.inline && refs[1].reference.target === null, 'Inline URI leaked into metadata')
    check(refs[2].reference.kind === 'link' && refs[2].localPath?.endsWith('sibling.md'), 'Explicitly granted local link was not resolved')
    const deniedLink = await clients[0].request('referenceInfo', { id: 2 })
    check(deniedLink.localPath === null, 'Single-file grant implicitly authorized siblings')
    check(await clients[0].request('referenceImageInfo', { id: 0 }).then(() => false, () => true), 'Ungranted image was read')
    const inline = await readyImage(clients[0], 1), local = await readyImage(clients[1], 0)
    check(inline.asset.width === 2 && local.asset.width === 2, 'Shared image decoder geometry differs')
    const blob = await clients[1].request('referenceImageFrame', { id: 0, imageRevision: local.imageRevision, frame: 0 })
    check(blob.blob instanceof Blob && blob.blob.type === 'image/png' && blob.revision === owners[1].revision, 'PNG transport lost its type/identity')
    check(await clients[1].request('referenceImageFrame', { id: 0, imageRevision: '0'.repeat(64), frame: 0 }).then(() => false, () => true), 'Stale image revision was accepted')
    check(await clients[1].request('referenceInfo', { id: 0, revision: '0'.repeat(64) }).then(() => false, () => true), 'Stale document revision was accepted')
    check(await clients[1].request('referenceInfo', { id: 13 }).then(() => false, () => true), 'Unknown reference was accepted')
    for (const id of [5, 6]) check(await clients[1].request('referenceImageInfo', { id }).then(() => false, () => true), 'An outside/remote image was fetched')

    // Ordinary host DOM uses exactly the Rust projection, sanitizer and image view.
    const viewport = document.createElement('div'), root = document.createElement('div')
    viewport.style.cssText = 'height:300px;width:600px;overflow:auto;border:1px solid gray'
    viewport.append(root); document.body.prepend(viewport); containers.push(viewport)
    const projection = await clients[1].request('projectionWindow', { start: 0, count: 5 })
    for (const page of projection.pages) root.append(projectionHtml(page.html))
    let link, busy = 0
    const provider = readerReferenceProvider(clients[1], owners[1])
    const view = mountReferences(root, { owner: owners[1], viewport,
      provider: {...provider,imageInfo: request => {
        if (!busy++) return Promise.reject(Object.assign(Error('Owned worker is busy'),{code:'BUSY'}))
        return provider.imageInfo(request)
      }}, onLink: value => { link = value } })
    views.push(view)
    await wait(() => root.querySelector('[data-image-reference="0"] [data-asset-state="ready"] img') && root.querySelector('[data-image-reference="1"] [data-asset-state="ready"] img'))
    check(root.querySelector('[data-image-reference="0"] img').naturalWidth === 2, 'Ordinary host image did not render')
    check(busy > 1, 'Busy public transport did not retry image preparation')
    const figure = root.querySelector('[data-image-reference="0"] figure')
    figure.focus()
    const controls = figure.querySelector('.asset-controls'), box = controls.getBoundingClientRect(), area = viewport.getBoundingClientRect()
    check(getComputedStyle(controls).visibility === 'visible' && box.left >= area.left && box.right <= area.right + 1 && box.top >= area.top && box.bottom <= area.bottom + 1, 'Compact image controls are clipped or inaccessible')
    root.querySelector('[data-link-reference="2"]').click()
    await wait(() => link)
    check(link.documentId === owners[1].documentId && link.precision === 'exact' && link.reference.startByte === refs[2].reference.startByte, 'Host link callback lost source identity')
    const old = root.querySelector('[data-image-reference="0"]')
    old.remove()
    await wait(() => !old.querySelector('img'))
    check(!old.querySelector('img'), 'Unmount retained a bitmap')
    const replacement = document.createElement('span'); replacement.dataset.imageReference = '0'; replacement.textContent = '[image: replacement]'
    root.prepend(replacement)
    await wait(() => replacement.querySelector('img'))
    check(replacement.querySelector('img').alt === 'replacement', 'Remount lost image alt or ownership')
    view.dispose()
    check(view.state().mounted === 0 && view.state().pixels === 0 && !root.querySelector('img'), 'Disposal retained resource mounts')

    // Real image mounts with a local provider exercise the DOM budget without
    // allocating oversized test bitmaps or invoking another document decoder.
    root.replaceChildren(); viewport.scrollTop = 0
    for (let id = 0; id < 33; id++) {
      const node = document.createElement('span')
      node.dataset.imageReference = String(id); node.textContent = '[image: bounded]'
      node.style.cssText = 'display:inline-block;width:2px;height:1px'
      root.append(node)
    }
    const budgetView = mountReferences(root, { owner: owners[1], viewport,
      provider: {info:provider.info,imageInfo:async()=>local,imageFrame:async()=>blob.blob} })
    views.push(budgetView)
    await wait(() => budgetView.state().mounted === 32 && budgetView.state().waiting === 1)
    check(root.querySelectorAll('img').length === 32, 'Image render budget was exceeded')
    const deferred = root.querySelector('[data-reference-state="waiting"]')
    deferred.style.display = 'none'
    await wait(() => budgetView.state().waiting === 0)
    root.querySelector('[data-reference-state="ready"]').style.display = 'none'
    await wait(() => budgetView.state().mounted === 31)
    deferred.style.display = 'inline-block'
    await wait(() => deferred.querySelector('img'))
    check(budgetView.state().mounted === 32, 'An offscreen deferred image never retried its budget')
    budgetView.dispose()
    check(!root.querySelector('img') && budgetView.state().pixels === 0, 'Budgeted images survived disposal')
    for (const client of clients) await client.dispose()
    check(clients.every(client => client.active === 0), 'Closing retained protocol requests')
    return { assertions, explicitAuthority: true, inlineImages: true, ordinaryImages: true, linkIdentity: true, remountCleanup: true }
  } finally {
    for (const view of views) view.dispose()
    for (const client of clients) await client.dispose({ force: true }).catch(() => {})
    for (const node of [...frames, ...containers]) node.remove()
  }
}
