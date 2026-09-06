export interface RouterConfiguration {
  controllerBackend: string
  controllers: Record<string, string>
  fallbacks: { sessionKey: string; condition: string }[]
}
export interface RouterThread {
  id?: string; cwd?: string; name?: string; title?: string
  archived?: boolean; ephemeral?: boolean
}
export interface RouterOpening { responsibility?: string; text?: string }
export interface RouterCandidate {
  key: string; backend: string; id: string; title: string; cwd: string
  responsibility: string; fallback: string; fallbackCondition: string; openingMessage: string
}
export interface RouterDecision {
  action: 'dispatch' | 'clarify'
  targetSessionKey: string; forwardedPrompt: string; reason: string; message: string
}
export interface RouterTurn { items?: readonly { type: string; text?: unknown }[] }
