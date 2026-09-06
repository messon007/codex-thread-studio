import { createSerializedStateWriter } from '../ui-src/serialized-state-writer.mjs'
const writer = createSerializedStateWriter(async (_path, request) => {
  const body: string = request.body
  void body
  return { ok: true, status: 200 }
}, (_error: unknown) => {})
await writer.write('/state', { sessionKey: 'codex:one' }, 'PUT')
// @ts-expect-error State persistence uses only the existing PUT/DELETE methods.
await writer.write('/state', {}, 'POST')
// @ts-expect-error The writer snapshots objects rather than accepting already-serialized strings.
await writer.write('/state', '{}')
