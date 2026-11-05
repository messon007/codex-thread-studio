import { assetController } from './assets.mjs'
import { componentStyles } from './component-styles.mjs'
import { IMAGE_FRAME_BYTES } from './image-frames.mjs'

export function mountImageView(
  root,
  { owner, metadata, frame, viewport = root, translate, onChange = () => {} }
) {
  if (
    root?.nodeType !== 1 ||
    viewport?.nodeType !== 1 ||
    typeof frame !== 'function' ||
    typeof onChange !== 'function' ||
    typeof owner?.documentId !== 'string' ||
    !owner.documentId ||
    owner.documentId.length > 256 ||
    typeof owner.revision !== 'string' ||
    !/^[a-f0-9]{64}$/i.test(owner.revision)
  )
    throw TypeError('Images require an owning document, viewport and frame provider')
  if (
    !metadata ||
    metadata.revision !== owner.revision ||
    !Number.isSafeInteger(metadata.id) ||
    metadata.id < 0 ||
    metadata.id >= 2048 ||
    !Number.isInteger(metadata.width) ||
    !Number.isInteger(metadata.height) ||
    metadata.width < 1 ||
    metadata.height < 1 ||
    metadata.width > 32768 ||
    metadata.height > 32768 ||
    metadata.width * metadata.height > 16 * 1024 * 1024 ||
    typeof metadata.alt !== 'string' ||
    new TextEncoder().encode(metadata.alt).length > 1024 ||
    !Array.isArray(metadata.frames) ||
    !metadata.frames.length ||
    metadata.frames.length > 128 ||
    metadata.frames.some(
      (item, index) =>
        item.frame !== index ||
        !Number.isInteger(item.delayMs) ||
        item.delayMs < 0 ||
        item.delayMs > 0xffffffff
    ) ||
    !Number.isInteger(metadata.plays) ||
    metadata.plays < 0 ||
    metadata.plays > 0xffffffff
  )
    throw TypeError('Invalid bounded image metadata')
  const identity = Object.freeze({ documentId: owner.documentId, revision: owner.revision })
  metadata = Object.freeze({
    ...metadata,
    frames: Object.freeze(metadata.frames.map((item) => Object.freeze({ ...item })))
  })
  const document = root.ownerDocument,
    figure = document.createElement('figure'),
    image = document.createElement('div'),
    caption = document.createElement('figcaption')
  figure.dataset.assetId = metadata.id
  figure.setAttribute('data-marknoria-image', '')
  image.className = 'document-image'
  caption.textContent = metadata.alt
  figure.append(image, caption)
  root.append(figure)
  const releaseStyle = componentStyles(document, new URL('./image-view.css', import.meta.url))
  let controller,
    disposed = false,
    resolveReady,
    rejectReady
  const ready = new Promise((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })
  void ready.catch(() => {})
  try {
    controller = assetController(
      viewport,
      { revision: identity.revision, format: 'image', assets: { [metadata.id]: metadata } },
      identity.documentId,
      () => onChange(),
      {
        translate,
        frame: async (request) => {
          for (let attempt = 0; ; attempt++) {
            request.signal.throwIfAborted()
            try {
              return await frame({ ...identity, ...request })
            } catch (error) {
              if (error.code !== 'BUSY' || attempt >= 7) throw error
              await new Promise((resolve, reject) => {
                const timer = setTimeout(done, 50 * (attempt + 1))
                function done() {
                  request.signal.removeEventListener('abort', abort)
                  resolve()
                }
                function abort() {
                  clearTimeout(timer)
                  request.signal.removeEventListener('abort', abort)
                  reject(request.signal.reason)
                }
                request.signal.addEventListener('abort', abort, { once: true })
                if (request.signal.aborted) abort()
              })
            }
          }
        },
        onState(node) {
          if (disposed) return
          if (node.dataset.assetState === 'ready') resolveReady(controller.state(figure))
          else if (node.dataset.assetState === 'error') rejectReady(Error(image.textContent))
        }
      }
    )
    controller.mount(figure)
  } catch (error) {
    figure.remove()
    releaseStyle()
    throw error
  }
  return {
    root: figure,
    ready,
    state: () => ({ ...identity, disposed, ...controller.state(figure) }),
    dispose() {
      if (disposed) return
      disposed = true
      controller.dispose()
      rejectReady(new DOMException('Image view disposed', 'AbortError'))
      figure.remove()
      releaseStyle()
    }
  }
}

export async function readerImageProvider(client, owner, id, { signal } = {}) {
  const identity = Object.freeze({ documentId: owner.documentId, revision: owner.revision, id })
  const metadata = await client.request('assetInfo', identity, { signal })
  if (metadata.id !== id || metadata.revision !== identity.revision)
    throw Error('Image metadata identity changed')
  return {
    owner: identity,
    metadata,
    frame: async ({ frame, signal }) => {
      const result = await client.request('assetFrame', { ...identity, frame }, { signal })
      if (
        result.revision !== identity.revision ||
        result.id !== id ||
        result.frame !== frame ||
        !(result.blob instanceof Blob) ||
        result.blob.type !== 'image/png' ||
        result.blob.size > IMAGE_FRAME_BYTES
      )
        throw Error('Image frame identity or byte profile changed')
      return result.blob
    }
  }
}
