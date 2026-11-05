// Actual cross-origin public protocol and Blob transport in native WebKitGTK.
export async function embeddedImageContract(moduleUrl, sourceUrl) {
  const { mountImageView, readerImageProvider } = await import(moduleUrl)
  const { createReaderClient } = await import(
    new URL('./host-client.mjs', new URL(moduleUrl, location.href))
  )
  const frame = document.createElement('iframe'),
    viewport = document.createElement('div'),
    root = document.createElement('div')
  frame.src = sourceUrl
  frame.title = 'Granted image reader'
  frame.style.cssText = 'width:640px;height:300px'
  viewport.style.cssText = 'width:600px;height:220px;overflow:auto'
  viewport.append(root)
  document.body.append(frame, viewport)
  const client = createReaderClient(frame)
  let view,
    assertions = 0
  const check = (condition, message) => {
    if (!condition) throw Error(message)
    assertions++
  }
  try {
    const owner = await client.ready()
    check(owner.document.assets === 1 && owner.capabilities.assetFrame, 'Image capability missing')
    const result = await client.request('assetFrame', { id: 0, frame: 0 })
    check(
      result.blob instanceof Blob && result.blob.type === 'image/png',
      'Binary frame was not cloned as a PNG Blob'
    )
    check(
      result.revision === owner.revision && result.id === 0 && result.frame === 0,
      'Binary frame lost its identity'
    )
    const signature = new Uint8Array(await result.blob.slice(0, 8).arrayBuffer())
    check([...signature].join(',') === '137,80,78,71,13,10,26,10', 'PNG bytes changed in transport')
    view = mountImageView(root, { ...(await readerImageProvider(client, owner, 0)), viewport })
    await view.ready
    check(
      view.root.querySelector('img').naturalWidth === 2,
      'Ordinary host did not render the projected frame'
    )
    view.root.querySelectorAll('button')[2].click()
    check(view.state().zoom === 1.25, 'Shared zoom control did not work')
    view.dispose()
    check(
      !view.root.isConnected && !view.root.querySelector('img'),
      'Closing retained the image or bitmap'
    )
    await client.dispose()
    check(client.active === 0, 'Public client retained pending image requests')
    return { assertions, publicImageProtocol: true, blobTransport: true, ordinaryImage: true }
  } finally {
    view?.dispose()
    await client.dispose({ force: true }).catch(() => {})
    frame.remove()
    viewport.remove()
  }
}
