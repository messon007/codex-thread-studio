import { finalAgentText, parseRouterDecision } from './thread-router.mjs'
import type { RouterDecision, RouterTurn } from './router-types.mjs'
import type { SessionTurnInput } from './backend-types.mjs'

export interface PendingRouterTurn { candidateKeys: string[]; requestedAt: number; attachments: SessionTurnInput[] }
export interface RouterDispatchState {
  status: 'routing' | 'dispatching' | 'running' | 'completed' | 'failed' | 'clarify'
  decision?: RouterDecision
  targetTurnId?: string
  error?: string
  decisionInvalid?: boolean
}
export interface RoutedTarget { routerTurnId: string; targetSessionKey: string }
export interface RouterCoordinationState {
  pending: Map<string, PendingRouterTurn>
  dispatches: Map<string, RouterDispatchState>
  targetTurns: Map<string, RoutedTarget>
}
export interface TargetDispatchResult { targetTurnKey: string; targetTurnId: string; targetSessionKey: string }

export class RouterTurnCoordinator {
  private starts = new Set<string>()
  constructor(readonly state: RouterCoordinationState) {}

  async start<T>(sessionKey: string, execute: () => Promise<T>, busyMessage: string): Promise<T> {
    if (this.starts.has(sessionKey)) throw new Error(busyMessage)
    this.starts.add(sessionKey)
    try { return await execute() }
    finally { this.starts.delete(sessionKey) }
  }

  finishTarget(key: string, turn: { status?: string; error?: { message?: string } } | undefined): boolean {
    const routed = this.state.targetTurns.get(key)
    if (!routed) return false
    this.state.dispatches.set(routed.routerTurnId, {
      ...this.state.dispatches.get(routed.routerTurnId),
      status: turn?.status === 'failed' ? 'failed' : 'completed',
      error: turn?.error?.message || '',
    })
    this.state.targetTurns.delete(key)
    return true
  }

  async complete<T extends TargetDispatchResult>(key: string, turn: RouterTurn | undefined, effects: {
    changed: () => void
    dispatch: (decision: RouterDecision, pending: PendingRouterTurn) => Promise<T>
    started: (result: T) => void
    failed: (message: string) => void
  }): Promise<boolean> {
    const pending = this.state.pending.get(key)
    if (!pending) return false
    // Claim synchronously before awaiting: notifications and polling can race.
    this.state.pending.delete(key)
    let decisionParsed = false
    try {
      const decision = parseRouterDecision(finalAgentText(turn), pending.candidateKeys)
      decisionParsed = true
      if (decision.action === 'clarify') {
        this.state.dispatches.set(key, { status: 'clarify', decision })
        effects.changed()
        return true
      }
      this.state.dispatches.set(key, { status: 'dispatching', decision })
      effects.changed()
      const result = await effects.dispatch(decision, pending)
      this.state.dispatches.set(key, { status: 'running', decision, targetTurnId: result.targetTurnId })
      this.state.targetTurns.set(result.targetTurnKey, { routerTurnId: key, targetSessionKey: result.targetSessionKey })
      effects.started(result)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      this.state.dispatches.set(key, { status: 'failed', error: message, decisionInvalid: !decisionParsed })
      effects.changed()
      effects.failed(message)
    }
    return true
  }
}
