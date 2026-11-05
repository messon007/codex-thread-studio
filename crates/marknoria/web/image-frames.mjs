// Transport only. Rust decodes/sanitizes images and produces bounded PNG frames.
export const IMAGE_FRAME_BYTES = 20 * 1024 * 1024

export async function imageFrameResponse(response, { revision, signal } = {}) {
  const reject = async (message, code) => {
    await response.body?.cancel().catch(() => {})
    throw Object.assign(Error(message), code ? { code } : {})
  }
  if (typeof revision !== 'string' || !/^[a-f0-9]{64}$/i.test(revision))
    return reject('An image revision is required')
  if (!response.ok) return reject(`Image request failed (${response.status})`)
  if (response.headers.get('x-reader-revision') !== revision)
    return reject('Image revision changed', 'REVISION')
  if (response.headers.get('content-type')?.split(';', 1)[0].trim().toLowerCase() !== 'image/png')
    return reject('Expected a projected PNG frame')
  const length = Number(response.headers.get('content-length'))
  if (length > IMAGE_FRAME_BYTES) return reject('Image exceeds its byte budget')
  if (!response.body) throw Error('Image frame is empty')
  const reader = response.body.getReader(),
    chunks = []
  let total = 0
  const abort = () => {
    void reader.cancel(signal.reason).catch(() => {})
  }
  signal?.addEventListener('abort', abort, { once: true })
  try {
    signal?.throwIfAborted()
    while (true) {
      const { done, value } = await reader.read()
      signal?.throwIfAborted()
      if (done) break
      total += value.byteLength
      if (total > IMAGE_FRAME_BYTES) throw Error('Image exceeds its byte budget')
      chunks.push(value)
    }
    if (!total) throw Error('Image frame is empty')
    return new Blob(chunks, { type: 'image/png' })
  } finally {
    signal?.removeEventListener('abort', abort)
    await reader.cancel().catch(() => {})
    reader.releaseLock()
  }
}
