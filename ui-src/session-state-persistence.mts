import type { CommentDraft } from './comment-types.mjs'
import type { QueuedMessage } from './session-types.mjs'
import { sessionModelPreferencePayload, type SessionModelOptions } from './session-model-preferences.mjs'
import type { createSerializedStateWriter } from './serialized-state-writer.mjs'

export interface OpeningMessage {
  text?: string
  responsibility?: string
  source?: string
  capturedAt?: string
  truncated?: boolean
}
export interface PersistedSessionState {
  annotationDrafts: Record<string, CommentDraft[]>
  annotationAdditional: Record<string, string>
  openingMessages: Record<string, OpeningMessage>
  turnOptions: Record<string, SessionModelOptions>
  messageQueues: Record<string, QueuedMessage[]>
}

/** The writer serializes synchronously; later state changes cannot alter a queued write. */
export function createSessionStatePersistence(
  state: PersistedSessionState,
  writer: ReturnType<typeof createSerializedStateWriter>,
  ready: () => boolean,
) {
  function write(key: string, path: string, body: Record<string, unknown>, method: 'PUT' | 'DELETE' = 'PUT'): Promise<void> {
    if (!key || !ready()) return Promise.resolve()
    return writer.write(path, body, method)
  }
  return {
    annotations(key: string) {
      return write(key, '/studio/session-state/annotations', {
        sessionKey: key, drafts: state.annotationDrafts[key] || [], additional: state.annotationAdditional[key] || '',
      })
    },
    openingMessage(key: string) {
      return write(key, '/studio/session-state/opening-message', { sessionKey: key, message: state.openingMessages[key] || null })
    },
    turnOptions(key: string) {
      return write(key, '/studio/session-state/turn-options', sessionModelPreferencePayload(key, state.turnOptions[key] || {}))
    },
    messageQueue(key: string) {
      return write(key, '/studio/session-state/message-queue', { sessionKey: key, messages: state.messageQueues[key] || [] })
    },
    remove(key: string) {
      return write(key, '/studio/session-state/session', { sessionKey: key }, 'DELETE')
    },
    pin(key: string, pinned: boolean) {
      return write(key, '/studio/session-state/pin', { sessionKey: key, pinned })
    },
  }
}

/** Keep queue edits atomic across tabs. A stale writer stops rather than overwriting another page. */
export function createVersionedMessageQueueWriter(
  state: { messageQueues: Record<string, QueuedMessage[]>; messageQueueRevisions?: Record<string, number> },
  transport: typeof fetch, ready: () => boolean,
  onConflict: (key: string, error: Error) => void,
) {
  const chains = new Map<string, Promise<void>>()
  function write(key: string): Promise<void> {
    if (!key || !ready()) return Promise.resolve()
    const messages = JSON.parse(JSON.stringify(state.messageQueues[key] || [])) as QueuedMessage[]
    const pending = (chains.get(key) || Promise.resolve()).catch(() => {}).then(async () => {
      const revision = state.messageQueueRevisions?.[key] || 0
      const response = await transport('/studio/session-state/message-queue', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ sessionKey: key, messages, expectedRevision: revision }) })
      if (!response.ok) {
        const error = new Error(response.status === 409 ? 'Another window changed this queue. Reload before editing or sending.' : `Queue save failed (HTTP ${response.status}); reload before retrying.`)
        throw error
      }
      const next = Number(response.headers.get('x-studio-queue-revision'))
      if (!Number.isSafeInteger(next) || next <= revision) throw new Error('Missing queue revision receipt; reload before retrying.')
      state.messageQueueRevisions ||= {}; state.messageQueueRevisions[key] = next
    })
    const guarded = pending.catch(error => { onConflict(key, error instanceof Error ? error : new Error(String(error))); throw error })
    chains.set(key, guarded)
    return guarded
  }
  return { write }
}
