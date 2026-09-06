export interface OpenCodeItem {
  id?: string | undefined
  type?: string
  content?: unknown[]
  studioUserParts?: Record<string, unknown[]>
  [field: string]: unknown
}
export interface OpenCodeTurn {
  id: string
  status: string
  items: OpenCodeItem[]
  error?: { message?: string } | null
}
export interface OpenCodeEventModel {
  threadId?: string | null | undefined
  turns: OpenCodeTurn[]
  activeTurnId: string | null
  status: string
  error: string | null
  diff: string
  approvals: { id: string | number; method: string; params: OpenCodeProperties }[]
  messageTurns?: Record<string, string>
  messageRoles?: Record<string, string | undefined>
  messageItems?: Record<string, string[]>
  messageErrors?: Record<string, { turnId: string; message: string }>
}
export interface OpenCodeInfo {
  id?: string
  sessionID?: string
  parentID?: string
  role?: string
  structured?: unknown
  error?: OpenCodeError | null
  finish?: string
  tokens?: { output?: number }
  [field: string]: unknown
}
export interface OpenCodePart {
  id?: string
  sessionID?: string
  messageID?: string
  type?: string
  text?: string
  url?: string
  path?: string
  filename?: string
  mime?: string
  tool?: string
  files?: string[]
  hash?: string
  state?: { input?: { command?: string; [key: string]: unknown }; title?: string; output?: string; error?: string; status?: string }
  [field: string]: unknown
}
export interface OpenCodeProperties {
  sessionID?: string
  sessionId?: string
  messageID?: string
  partID?: string
  field?: string
  delta?: string
  info?: OpenCodeInfo
  part?: OpenCodePart
  status?: unknown
  error?: OpenCodeError | null
  data?: { message?: string }
  message?: string
  name?: string
  type?: string
  text?: string
  diff?: unknown
  id?: string | number
  requestID?: string | number
  permissionID?: string | number
  [field: string]: unknown
}
export interface OpenCodeEvent {
  type?: string
  properties?: OpenCodeProperties
  payload?: OpenCodeEvent
}
export interface OpenCodeReduction {
  handled: boolean
  kind?: 'metadata' | 'full' | 'stream'
  sessionId: string | undefined
  type: string | undefined
  turnId?: string | null
  itemId?: string | undefined
}
/** Shared history conversion helpers remain JS; the live state reducer is typed. */
export interface OpenCodeReducerHelpers {
  normalizeOpenCodeStatus(status: unknown): string
  errorText(error: OpenCodeError | OpenCodeProperties | null | undefined): string
  ensureTurn(model: OpenCodeEventModel, id: string | null | undefined): OpenCodeTurn
  structuredOutputItem(info: OpenCodeInfo): OpenCodeItem
  upsertItem(turn: OpenCodeTurn, item: OpenCodeItem | null): void
  upsertOpenCodeUserPart(turn: OpenCodeTurn, part: OpenCodePart): void
  openCodePartToItem(part: OpenCodePart, role: string): OpenCodeItem | null
  rememberMessageItem(items: Record<string, string[]>, messageId: string | undefined, itemId: string | undefined): void
  forgetMessageItem(items: Record<string, string[]> | undefined, messageId: string, itemId: string): void
  refreshTurnMessageError(model: OpenCodeEventModel, turn: OpenCodeTurn): void
}

export type OpenCodeError = string | { data?: { message?: string }; message?: string; name?: string }
