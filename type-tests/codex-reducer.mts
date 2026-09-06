import { createCodexViewModel, applyCodexNotification, beginOptimisticCodexTurn, hydrateCodexThread } from '../ui-src/codex-native.mjs'
const model = createCodexViewModel()
applyCodexNotification(model, { method: 'turn/started', params: { turn: { id: 'one', status: 'inProgress', items: [] } } })
// @ts-expect-error Turn IDs are strings.
applyCodexNotification(model, { method: 'turn/started', params: { turn: { id: 1 } } })
// @ts-expect-error Stream delta payloads are strings.
applyCodexNotification(model, { method: 'item/agentMessage/delta', params: { delta: 12 } })
// @ts-expect-error History turns must have item arrays.
hydrateCodexThread(model, { turns: [{ id: 'one', status: 'completed', items: 'text' }] })
const id: string = beginOptimisticCodexTurn(model, { clientUserMessageId: 'user', input: [{ type: 'text', text: 'hello' }] })
void id
// @ts-expect-error A view model has a nullable string active turn, not a numeric ID.
model.activeTurnId = 10
