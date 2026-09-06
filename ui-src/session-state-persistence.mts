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
