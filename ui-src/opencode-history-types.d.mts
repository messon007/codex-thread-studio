import type { OpenCodeInfo, OpenCodePart, OpenCodeEvent, OpenCodeTurn, OpenCodeEventModel } from './opencode-event-types.mjs'
export interface OpenCodeSession {
  id: string
  title?: string
  directory?: string
  parentID?: string
  modelID?: string
  model?: { modelID?: string; providerID?: string; id?: string }
  time?: { updated?: number; created?: number }
}
export interface OpenCodeMessage { info?: OpenCodeInfo; parts?: OpenCodePart[] }
export type MessagePage = OpenCodeMessage[] | { messages?: OpenCodeMessage[]; cursor?: string }
export type FetchMessagePage = (params: { limit: number; before?: string }) => Promise<MessagePage | null>
export type FetchRootPage = (params: { limit: number; archived: false; roots: true; cursor?: number }) => Promise<OpenCodeSession[] | null>
export interface OpenCodeHistoryThread extends Partial<Pick<OpenCodeEventModel, 'messageTurns' | 'messageRoles' | 'messageItems' | 'messageErrors'>> {
  id?: string | undefined
  status?: string
  turns: OpenCodeTurn[]
}
export interface BufferedOpenCodeEvent { event: OpenCodeEvent; sequence: number }
export interface HistoryReplayOptions {
  messageSnapshots?: Record<string, { afterSequence: number; ambiguousThroughSequence: number }>
  statusAfterSequence?: number
  authoritativeStatus?: string | null
}
export interface OpenCodeProviderResult {
  providers?: OpenCodeProvider[]
  all?: OpenCodeProvider[]
  connected?: string[]
  default?: Record<string, string>
}
export interface OpenCodeProvider { id: string; name?: string; models?: Record<string, { name?: string }> }
export interface LoopGuardState {
  parents: Map<string, { ids: Set<string>; lastSignaledSize: number }>
  messages: Map<string, { hasContent: boolean; hasTool: boolean }>
}
