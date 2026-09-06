import type { LifecycleConnectionEffects, EventStreamState } from '../ui-src/lifecycle-connection.mjs'
import type { createSessionStatePersistence } from '../ui-src/session-state-persistence.mjs'
import { executeComposerSend } from '../ui-src/composer-send.mjs'

function checkPersistence(store: ReturnType<typeof createSessionStatePersistence>) {
  // @ts-expect-error A pin change requires an explicit boolean.
  store.pin('codex:a', 'false')
  // @ts-expect-error Session keys are strings, not backend objects.
  store.remove({ backend: 'codex', id: 'a' })
}
void checkPersistence

function checkLifecycle(effects: LifecycleConnectionEffects, state: EventStreamState) {
  // @ts-expect-error Undecoded protocol values cannot become backend names.
  effects.notification({}, {})
  // @ts-expect-error Readiness waiters must consume booleans.
  state.waiters.add((value: string) => { void value })
}
void checkLifecycle

void executeComposerSend({
  send: async () => ({ id: 'turn' }),
  acknowledged: result => {
    // @ts-expect-error The acceptance payload is inferred from the transport.
    const text: string = result
    void text
  },
  failed: (error, accepted) => {
    // @ts-expect-error Failure reasons remain unknown until narrowed.
    const message: string = error.message
    void message; void accepted
  },
  finished() {},
})
