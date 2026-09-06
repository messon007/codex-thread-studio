export interface CodexItem {
  id: string
  type?: string
  clientId?: string
  studioOptimistic?: boolean
  content?: unknown[]
  summary?: string[]
  text?: string
  aggregatedOutput?: string
  [field: string]: unknown
}
export interface CodexTurn {
  id: string
  status: string
  items: CodexItem[]
  error?: { message?: string } | null
  studioOptimistic?: boolean
  [field: string]: unknown
}
export type IncomingCodexTurn = Partial<CodexTurn>
export type IncomingCodexItem = Partial<CodexItem>
export interface CodexParams {
  threadId?: string
  turnId?: string
  itemId?: string
  turn?: IncomingCodexTurn
  item?: IncomingCodexItem
  delta?: string
  summaryIndex?: number
  contentIndex?: number
  explanation?: string
  plan?: unknown[]
  diff?: string
  tokenUsage?: unknown
  usage?: unknown
  status?: { type?: string } | null
  error?: { message?: string } | null
  message?: string
  requestId?: string | number
  [field: string]: unknown
}
export interface CodexNotification {
  method?: string
  id?: string | number | null
  params?: CodexParams
}
export interface CodexRequest {
  id: string | number
  method: string
  params: CodexParams
}
export interface CodexViewModel {
  threadId: string | null
  turns: CodexTurn[]
  activeTurnId: string | null
  status: string
  error: string | null
  diff: string
  usage: unknown
  approvals: CodexRequest[]
  interactions: CodexRequest[]
}
