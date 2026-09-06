import type { CodexTurn } from './codex-types.mjs'
import type { TurnPage } from './session-types.mjs'
export interface HistoryThread {
  id?: string
  turns?: CodexTurn[]
  [field: string]: unknown
}
export interface HistoryResponse {
  thread?: HistoryThread
  initialTurnsPage?: TurnPage<CodexTurn>
  [field: string]: unknown
}
export interface CachedHistory {
  model: { threadId: string | null; turns: CodexTurn[] }
}
export interface ResumeOptions {
  environmentRoot?: string
  environmentRevision?: string
  incremental?: boolean
}
export interface CodexHistoryServices {
  dispatchBackendRpc(backend: string, method: 'thread/read', params: { threadId: string; includeTurns: true }): Promise<HistoryResponse>
  dispatchBackendRpc(backend: string, method: 'thread/turns/list', params: { threadId: string; limit: number; sortDirection: 'desc'; itemsView: 'full'; cursor?: string }): Promise<TurnPage<CodexTurn>>
  requestCodexResume(backend: string, id: string, options: ResumeOptions): Promise<HistoryResponse>
  historyTailCapability(backend: string, id: string): boolean | undefined
  rememberHistoryTailCapability(backend: string, id: string, supported: boolean): void
  threadForRef(ref: { backend: string; id: string }): HistoryThread | null
}
