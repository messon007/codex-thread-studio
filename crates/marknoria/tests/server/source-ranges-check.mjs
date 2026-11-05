import assert from 'node:assert/strict'
import { sourceRangeOffsets } from '../../web/source.mjs'

const text = '\ufeff# 中文😀\r\nnext\r\n'
const bytes = Buffer.from(text)
const window = { start: 0, end: bytes.length, text }
const emoji = bytes.indexOf(Buffer.from('😀'))
const from = text.indexOf('😀')
assert.deepEqual(sourceRangeOffsets(window, emoji, emoji + 4), { from, to: from + 2 })
assert.deepEqual(sourceRangeOffsets(window, 0, 3), { from: 0, to: 1 })
const next = bytes.indexOf(Buffer.from('next'))
assert.deepEqual(sourceRangeOffsets(window, next, next + 4), { from: text.replaceAll('\r\n', '\n').indexOf('next'), to: text.replaceAll('\r\n', '\n').indexOf('next') + 4 })
const tail = { start: emoji, end: bytes.length, text: text.slice(from) }
assert.deepEqual(sourceRangeOffsets(tail, emoji, emoji + 4), { from: 0, to: 2 })
for (const [start,end] of [[1,2],[emoji+1,emoji+4],[emoji,emoji+1]]) assert.throws(() => sourceRangeOffsets(window,start,end))
const cr = bytes.indexOf(13)
assert.throws(() => sourceRangeOffsets(window,cr+1,cr+2),/CRLF/)
assert.throws(() => sourceRangeOffsets(window,0,bytes.length+1),/bounded window/)
assert.throws(() => sourceRangeOffsets({...window,end:window.end-1},0,3),/byte length/)
assert.throws(() => sourceRangeOffsets({start:0,end:128*1024+1,text:''},0,1),/bounded window/)
console.log(JSON.stringify({ok:true, assertions:11, originalBytes:true, normalizedDomUnits:true}))
