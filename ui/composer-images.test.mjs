import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  MAX_COMPOSER_IMAGE_TOTAL_BYTES,
  composerImageInputs,
  detectComposerImageMime,
  formatImageSize,
  openCodeImagePart,
  prepareComposerImage,
  safeImageUrl,
  userImagesFromContent,
} from './composer-images.mjs'

const PNG = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0])
const appSource = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const htmlSource = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styleSource = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('detects supported image formats by signature instead of file extension', () => {
  assert.equal(MAX_COMPOSER_IMAGE_TOTAL_BYTES, 20 * 1024 * 1024)
  assert.equal(detectComposerImageMime(PNG), 'image/png')
  assert.equal(detectComposerImageMime(Uint8Array.from([0xff, 0xd8, 0xff, 0xe0])), 'image/jpeg')
  assert.equal(detectComposerImageMime(new TextEncoder().encode('GIF89a')), 'image/gif')
  assert.equal(detectComposerImageMime(new TextEncoder().encode('not an image')), '')
})

test('prepares browser files as structured image input', async () => {
  const image = await prepareComposerImage({ name: 'screen.fake', size: PNG.length, async arrayBuffer() { return PNG.buffer } }, 'image-1')
  assert.equal(image.id, 'image-1')
  assert.equal(image.name, 'screen.fake')
  assert.equal(image.mime, 'image/png')
  assert.match(image.url, /^data:image\/png;base64,/u)
  assert.deepEqual(composerImageInputs([image]), [{ type: 'image', url: image.url }])
})

test('rejects unsupported and oversized attachments', async () => {
  await assert.rejects(() => prepareComposerImage({ name: 'note.svg', size: 4, async arrayBuffer() { return new TextEncoder().encode('<svg').buffer } }), /Only PNG/u)
  await assert.rejects(() => prepareComposerImage({ name: 'large.png', size: 10 * 1024 * 1024 + 1, async arrayBuffer() { return PNG.buffer } }), /10 MiB/u)
})

test('maps image data to OpenCode file parts and historical previews', () => {
  const url = 'data:image/png;base64,iVBORw0KGgo='
  assert.deepEqual(openCodeImagePart({ type: 'image', url }, 1), {
    type: 'file', mime: 'image/png', filename: 'image-2.png', url,
  })
  assert.deepEqual(userImagesFromContent([{ type: 'text', text: 'look' }, { type: 'image', url }, { type: 'localImage', path: '/tmp/example.jpg' }]), [
    { kind: 'url', source: url, label: 'Attached image' },
    { kind: 'local', source: '/tmp/example.jpg', label: 'example.jpg' },
  ])
  assert.equal(safeImageUrl('javascript:alert(1)'), false)
  assert.equal(formatImageSize(1536), '2 KiB')
})

test('composer exposes one consistent image attachment surface', () => {
  assert.match(htmlSource, /id="composer-add-image"[\s\S]*id="composer-model"/u)
  assert.match(htmlSource, /id="composer-image-input"[^>]+multiple/u)
  assert.match(appSource, /addEventListener\('paste', handleComposerImagePaste\)/u)
  assert.match(appSource, /addEventListener\('drop', handleComposerImageDrop\)/u)
  assert.match(appSource, /threadRouter\.startTurn\(text, imageInputs\)/u)
  assert.match(styleSource, /\.composer-icon-button \{ width: 27px; height: 27px;/u)
})
