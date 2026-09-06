import type { OpenCodeEventModel, OpenCodeEvent, OpenCodeReducerHelpers } from '../ui-src/opencode-event-types.mjs'
import { reduceOpenCodeEvent } from '../ui-src/opencode-event-reducer.mjs'
declare const model: OpenCodeEventModel
declare const helpers: OpenCodeReducerHelpers
const event: OpenCodeEvent = { type: 'message.part.delta', properties: { sessionID: 'one', messageID: 'message', partID: 'part', delta: 'hello' } }
const result = reduceOpenCodeEvent(model, event, 'one', helpers)
const handled: boolean = result.handled
void handled
// @ts-expect-error OpenCode message IDs are strings.
reduceOpenCodeEvent(model, { properties: { messageID: 10 } }, 'one', helpers)
// @ts-expect-error Stream deltas cannot be arrays.
reduceOpenCodeEvent(model, { properties: { delta: ['hello'] } }, 'one', helpers)
// @ts-expect-error Model message-to-turn mappings are string IDs.
model.messageTurns = { message: 10 }
