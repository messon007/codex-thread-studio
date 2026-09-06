/** Non-text attachments remain opaque until the backend validates their schema. */
export interface QueuedMessage {
  id: string
  text: string
  input: Record<string, unknown>[]
  createdAt: number
}

export interface CatalogModel {
  model?: string
  id?: string
  modelID?: string
  isDefault?: boolean
  defaultReasoningEffort?: string
}
export type ModelReference = string | CatalogModel | null
export interface ModelDisplayOptions {
  overrideModel?: ModelReference
  overrideEffort?: string
  sessionModel?: ModelReference
  models?: CatalogModel[]
  fallback?: string
}

export interface HistoryTurn { id?: string }
export interface TurnPage<T extends HistoryTurn> {
  data: T[]
  nextCursor?: string | null
  backwardsCursor?: string | null
}
export interface TurnTailOptions<T extends HistoryTurn> {
  cachedTurns: T[]
  initialPage: TurnPage<T> | null
  fetchPage?: (cursor: string) => Promise<TurnPage<T> | null>
  maxPages?: number
}
