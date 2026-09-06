import { collectOpenCodeMessageHistory, openCodeThreadFromHistory, replayOpenCodeEventsAfterHistory, openCodeModelList } from '../ui-src/opencode-native.mjs'
import type { OpenCodeEventModel } from '../ui-src/opencode-event-types.mjs'
const history = await collectOpenCodeMessageHistory(async () => ({ messages: [{ info: { id: 'one', role: 'user' }, parts: [] }] }))
const id: string | undefined = history.messages[0]?.info?.id
void id
// @ts-expect-error Pagination callbacks must return supported message pages.
await collectOpenCodeMessageHistory(async () => 'invalid')
const thread = openCodeThreadFromHistory(null, history.messages, { type: 'idle' })
// @ts-expect-error Turn IDs are strings, not numbers.
const invalid: number | undefined = thread.turns[0]?.id
void invalid
declare const model: OpenCodeEventModel
// @ts-expect-error Replay ordering markers are numeric.
replayOpenCodeEventsAfterHistory(model, [], 'one', { statusAfterSequence: '10' })
// @ts-expect-error Provider catalog model names are strings.
openCodeModelList({ providers: [{ id: 'provider', models: { model: { name: 10 } } }] })
