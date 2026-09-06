import { cachedSession, storeCachedSession, routeCodexNotification } from '../ui-src/session-model-cache.mjs'
import type { SessionModelCache } from '../ui-src/session-model-cache.mjs'
interface Model { threadId: string; activeTurnId: string | null; turns: { id: string }[]; status: 'idle' | 'running' }
const cache: SessionModelCache<Model> = new Map()
const model: Model = { threadId: 'one', activeTurnId: null, turns: [], status: 'idle' }
storeCachedSession(cache, 'codex', 'one', model, null, 10)
const status: 'idle' | 'running' | undefined = cachedSession(cache, 'codex', 'one')?.model.status
void status
// @ts-expect-error Cache timestamps cannot be strings.
storeCachedSession(cache, 'codex', 'one', model, null, 'now')
const routed = routeCodexNotification({}, {
  cache, backend: 'codex', selectedBackend: 'codex', selectedId: 'one', selectedModel: model,
  hiddenThreads: new Set(), hiddenTurns: new Set(), sessionKey: () => '', turnKey: () => '',
})
// @ts-expect-error Routing preserves the concrete model status type.
const invalid: number | undefined = routed?.status
void invalid
