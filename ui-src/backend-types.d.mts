/** Backend IDs remain open-ended: configured Codex-compatible instances are supported. */
export type BackendKind = 'codex' | 'opencode'
export interface BackendDescriptor {
  readonly id: string
  readonly name: string
  readonly tag: string
  readonly nativeLabel: string
  readonly binary: string
  readonly infoPath: string
  readonly socketPath: string
  readonly protocol: string
  readonly transport: string
  readonly kind: BackendKind
  readonly adapter: string
}

export interface SessionRef { readonly backend: string; readonly id: string }
export interface NormalizedSessionRef extends SessionRef { readonly key: string }
export interface PreparationOptions { alreadyActive?: boolean }
export interface SessionReadOptions { [key: string]: unknown }
export type SessionTurnInput = string | readonly Record<string, unknown>[]
export interface SessionStartOptions {
  clientUserMessageId?: string
  additionalContext?: string
  developerInstructions?: string
  outputSchema?: Record<string, unknown>
  turnOptions?: Record<string, unknown>
  timeoutMs?: number
}
/** Results stay opaque until an adapter-specific decoder validates the response. */
export interface SessionAdapter<ReadResult = unknown, StartResult = unknown> {
  read: (ref: NormalizedSessionRef, options: SessionReadOptions) => ReadResult
  prepareTurn?: (ref: NormalizedSessionRef, options: PreparationOptions) => unknown
  startTurn: (ref: NormalizedSessionRef, input: SessionTurnInput, options: SessionStartOptions) => StartResult
}
export interface PreparedTurnRequest<T> {
  registry: { clearPreparedSession(ref: SessionRef): void }
  ref: SessionRef
  prepare: () => unknown
  start: () => T | PromiseLike<T>
  recoverThreadNotFound?: boolean
}

/** Catalog metadata may include adapter-specific fields; they remain unknown. */
export interface SessionCatalogEntry {
  id?: string
  cwd?: string
  turns?: unknown
  [field: string]: unknown
}
