export interface ScrollMetrics { scrollHeight?: number; scrollTop?: number; clientHeight?: number }
export interface ScrollState { anchorTurnId?: string; scrollTop?: number; [field: string]: unknown }
export type RestorePlan = { type: 'none' | 'defer' | 'stale' | 'unavailable' } | { type: 'anchor'; anchorTurnId: string } | { type: 'scrollTop'; scrollTop: number }
export interface PresentationItem {
  id?: string
  type?: string
  status?: string
  phase?: string
  changes?: { kind?: string; path?: string; diff?: unknown }[]
  [field: string]: unknown
}
export interface PresentationTurn { id?: string; status?: string; items?: PresentationItem[]; error?: { message?: string } | null }
export interface PresentationModel { turns?: PresentationTurn[] }
export type CommandKind = 'reads' | 'searches' | 'lists' | 'commands'
export type ActivityEntry = { itemId?: string | undefined; item: PresentationItem; status?: string } & (
  | { kind: 'command'; commandKind: CommandKind }
  | { kind: 'reasoning' | 'change' | 'plan' | 'search' | 'tool' | 'system' | 'unknown' | 'progress' }
)
export type ActivitySummary = Record<CommandKind | 'tools' | 'webSearches' | 'changedFiles' | 'plans' | 'reasoning' | 'failures', number>
export interface ActivityBlock {
  type: 'activity'
  id: string
  active: boolean
  entries: ActivityEntry[]
  displayEntries: ActivityEntry[]
  summary: ActivitySummary
  latestStage: string
  sourceItemIds: (string | undefined)[]
}
export type MessageBlock = { type: 'user' | 'assistant' | 'command'; itemId?: string | undefined; item: PresentationItem; variant?: 'plan' | 'message' }
export type PresentationBlock = MessageBlock | ActivityBlock | { type: 'error'; message: string }
export interface TurnPresentation { id: string; status: string; blocks: PresentationBlock[]; source: PresentationTurn }
export interface PresentationCacheEntry {
  turns: Map<string, { signature: string; presentation: TurnPresentation }>
  sourceModel: PresentationModel | null
  sourceRevision: number | null
  sourceTurns: PresentationTurn[]
  orderedIds: string[]
  historyWindow: number
  visibleStart: number
  visibleEnd: number
  windowMode: 'latest' | 'fixed'
  scrollTop: number | null
  scrollState: ScrollState | null
}
