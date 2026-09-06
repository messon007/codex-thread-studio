// Generated from ui-src; run npm run build:ui. Do not edit.
import { finalAgentText, parseRouterDecision } from './thread-router.mjs';
export class RouterTurnCoordinator {
    state;
    starts = new Set();
    constructor(state) {
        this.state = state;
    }
    async start(sessionKey, execute, busyMessage) {
        if (this.starts.has(sessionKey))
            throw new Error(busyMessage);
        this.starts.add(sessionKey);
        try {
            return await execute();
        }
        finally {
            this.starts.delete(sessionKey);
        }
    }
    finishTarget(key, turn) {
        const routed = this.state.targetTurns.get(key);
        if (!routed)
            return false;
        this.state.dispatches.set(routed.routerTurnId, {
            ...this.state.dispatches.get(routed.routerTurnId),
            status: turn?.status === 'failed' ? 'failed' : 'completed',
            error: turn?.error?.message || '',
        });
        this.state.targetTurns.delete(key);
        return true;
    }
    async complete(key, turn, effects) {
        const pending = this.state.pending.get(key);
        if (!pending)
            return false;
        // Claim synchronously before awaiting: notifications and polling can race.
        this.state.pending.delete(key);
        let decisionParsed = false;
        try {
            const decision = parseRouterDecision(finalAgentText(turn), pending.candidateKeys);
            decisionParsed = true;
            if (decision.action === 'clarify') {
                this.state.dispatches.set(key, { status: 'clarify', decision });
                effects.changed();
                return true;
            }
            this.state.dispatches.set(key, { status: 'dispatching', decision });
            effects.changed();
            const result = await effects.dispatch(decision, pending);
            this.state.dispatches.set(key, { status: 'running', decision, targetTurnId: result.targetTurnId });
            this.state.targetTurns.set(result.targetTurnKey, { routerTurnId: key, targetSessionKey: result.targetSessionKey });
            effects.started(result);
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            this.state.dispatches.set(key, { status: 'failed', error: message, decisionInvalid: !decisionParsed });
            effects.changed();
            effects.failed(message);
        }
        return true;
    }
}
