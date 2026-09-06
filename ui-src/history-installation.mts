import type { OpenCodeEventModel } from './opencode-event-types.mjs'
import type { OpenCodeHistoryThread } from './opencode-history-types.mjs'

export function hydrateOpenCodeHistoryMetadata(
  thread: OpenCodeHistoryThread | null | undefined,
  model: OpenCodeEventModel & { historyComplete?: boolean },
  historyComplete = true,
): void {
  model.messageTurns = thread?.messageTurns || {}
  model.messageRoles = thread?.messageRoles || {}
  model.messageItems = thread?.messageItems || {}
  model.messageErrors = thread?.messageErrors || {}
  model.historyComplete = historyComplete !== false
  model.status = thread?.status || model.status
  model.activeTurnId = model.status === 'running' ? model.turns.at(-1)?.id || null : null
}

/** Synchronous installation after the caller's selection/connection checks. */
export function installSelectedHistory<M extends { historyComplete?: boolean }, T extends { status?: unknown }, E, S>(options: {
  opencode: boolean
  model: M
  thread: T
  historyAnchorTurnId?: string | undefined
  historyComplete?: boolean | undefined
  historyEvents: E[] | null
  messageSnapshots?: S | undefined
  statusSequence?: number | undefined
  mergeTail: (model: M, thread: T, anchor: string) => boolean
  hydrate: (model: M, thread: T) => void
  hydrateMetadata: (thread: T, model: M, complete?: boolean) => void
  replay: (model: M, events: E[], options: {
    messageSnapshots: S | undefined
    statusAfterSequence: number | undefined
    authoritativeStatus: unknown
  }) => void
  ready: (options: { complete: boolean }) => void
  mergeMetadata: (thread: T) => void
  cache: (model: M) => void
}): void {
  const { model, thread } = options
  const installedTail = options.opencode && options.historyAnchorTurnId
    ? options.mergeTail(model, thread, options.historyAnchorTurnId)
    : false
  if (!installedTail) options.hydrate(model, thread)
  if (options.opencode && !installedTail) options.hydrateMetadata(thread, model, options.historyComplete)
  if (options.historyEvents?.length) options.replay(model, options.historyEvents, {
    messageSnapshots: options.messageSnapshots,
    statusAfterSequence: options.statusSequence,
    authoritativeStatus: options.statusSequence !== undefined && options.statusSequence >= 0 ? thread?.status : null,
  })
  options.ready({ complete: installedTail ? model.historyComplete !== false : options.historyComplete !== false })
  options.mergeMetadata(thread)
  options.cache(model)
}
