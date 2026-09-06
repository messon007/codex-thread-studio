import { normalizeStoredMessageQueues } from '../ui-src/message-queue.mjs'
import { resolveModelDisplay } from '../ui-src/model-display.mjs'
import { collectCodexTurnTail } from '../ui-src/thread-history-tail.mjs'

const queues = normalizeStoredMessageQueues({})
const text: string | undefined = queues['codex:example']?.[0]?.text
void text
// @ts-expect-error Queue message text cannot be treated as a number.
const invalidText: number = queues['codex:example']?.[0]?.text
void invalidText
// @ts-expect-error Model IDs must be strings.
resolveModelDisplay({ overrideModel: { id: 123 } })
const result = await collectCodexTurnTail({
  cachedTurns: [{ id: 'one', items: ['hello'] }],
  initialPage: { data: [{ id: 'one', items: ['updated'] }] },
})
const item: string | undefined = result.turns[0]?.items[0]
void item
// @ts-expect-error Generic history merging must preserve the item type.
const invalidItem: number | undefined = result.turns[0]?.items[0]
void invalidItem
