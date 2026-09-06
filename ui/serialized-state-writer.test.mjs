import test from 'node:test'
import assert from 'node:assert/strict'
import { createSerializedStateWriter } from './serialized-state-writer.mjs'

test('writes are ordered and preserve the data captured when enqueued', async () => {
  const requests = []
  let release
  const writer = createSerializedStateWriter(async (path, request) => {
    requests.push({ path, ...request })
    if (requests.length === 1) await new Promise(resolve => { release = resolve })
    return { ok: true, status: 200 }
  }, () => {})
  const body = { sessionKey: 'codex:one', nested: { model: 'first' } }
  const first = writer.write('/state', body)
  body.nested.model = 'second'
  const second = writer.write('/state', body, 'DELETE')
  body.sessionKey = 'ept-codex:other'
  await Promise.resolve(); await Promise.resolve()
  assert.equal(requests.length, 1)
  release()
  await Promise.all([first, second])
  assert.deepEqual(requests.map(entry => JSON.parse(entry.body)), [
    { sessionKey: 'codex:one', nested: { model: 'first' } },
    { sessionKey: 'codex:one', nested: { model: 'second' } },
  ])
  assert.equal(requests[1].method, 'DELETE')
})

test('one failed write is reported but does not prevent later persistence', async () => {
  const errors = []
  let calls = 0
  const writer = createSerializedStateWriter(async () => ({ ok: ++calls > 1, status: calls === 1 ? 503 : 200 }), error => errors.push(error))
  const first = writer.write('/state', {})
  const next = writer.write('/state', {})
  await assert.rejects(first, /HTTP 503/)
  await next
  assert.equal(errors.length, 1)
  assert.equal(calls, 2)
})

test('preference and session writers do not serialize unrelated domains together', async () => {
  let release
  const first = createSerializedStateWriter(() => new Promise(resolve => { release = resolve }), () => {})
  const second = createSerializedStateWriter(async () => ({ ok: true, status: 200 }), () => {})
  const pending = first.write('/preferences', {})
  await second.write('/session', {})
  release({ ok: true, status: 200 })
  await pending
})
