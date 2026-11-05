import assert from 'node:assert/strict'
import { imageFrameResponse, IMAGE_FRAME_BYTES } from '../../web/image-frames.mjs'
const revision = 'a'.repeat(64)
const headers = { 'content-type': 'image/png', 'x-reader-revision': revision }
const response = (body) => new Response(body, { headers })
const bytes = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10])
const blob = await imageFrameResponse(response(bytes), { revision })
assert.equal(blob.type, 'image/png')
assert.deepEqual(new Uint8Array(await blob.arrayBuffer()), bytes)
await assert.rejects(
  imageFrameResponse(response(bytes), { revision: 'b'.repeat(64) }),
  /revision changed/
)
await assert.rejects(imageFrameResponse(response(bytes)), /revision is required/)
await assert.rejects(
  imageFrameResponse(
    new Response(bytes, { headers: { ...headers, 'content-type': 'image/svg+xml' } }),
    { revision }
  ),
  /PNG/
)
await assert.rejects(imageFrameResponse(new Response(null, { headers }), { revision }), /empty/)
let headerCancelled = false
const tooLarge = new Response(
  new ReadableStream({
    cancel() {
      headerCancelled = true
    }
  }),
  { headers: { ...headers, 'content-length': String(IMAGE_FRAME_BYTES + 1) } }
)
await assert.rejects(imageFrameResponse(tooLarge, { revision }), /byte budget/)
assert.equal(headerCancelled, true)
let cancelled = false
let produced = 0
const oversized = new Response(
  new ReadableStream({
    pull(controller) {
      produced++
      controller.enqueue(new Uint8Array(1024 * 1024))
    },
    cancel() {
      cancelled = true
    }
  }),
  { headers }
)
await assert.rejects(imageFrameResponse(oversized, { revision }), /byte budget/)
assert.equal(cancelled, true)
assert.ok(produced <= 22)
const lifetime = new AbortController()
let readCancelled = false
const blocked = new Response(
  new ReadableStream({
    cancel() {
      readCancelled = true
    }
  }),
  { headers }
)
const pending = imageFrameResponse(blocked, { revision, signal: lifetime.signal })
lifetime.abort(new DOMException('Synthetic close', 'AbortError'))
await assert.rejects(pending, { name: 'AbortError' })
assert.equal(readCancelled, true)
console.log(JSON.stringify({ ok: true, assertions: 12, boundedStream: true, cancellation: true }))
