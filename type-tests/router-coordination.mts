import { RouterTurnCoordinator } from '../ui-src/router-coordination.mjs'
const coordinator = new RouterTurnCoordinator({ pending: new Map(), dispatches: new Map(), targetTurns: new Map() })
// @ts-expect-error Router lifecycle statuses are closed.
coordinator.state.dispatches.set('x', { status: 'anything' })
coordinator.complete('x', undefined, {
  changed() {}, started() {}, failed() {},
  // @ts-expect-error The target turn must be identified for completion tracking.
  dispatch: async () => ({ targetSessionKey: 'codex:x' }),
})
