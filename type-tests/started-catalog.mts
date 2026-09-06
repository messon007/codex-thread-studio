import type { createStartedSessionCatalog } from '../ui-src/started-session-catalog.mjs'

function check(catalog: ReturnType<typeof createStartedSessionCatalog>) {
  // @ts-expect-error Session IDs cannot be numeric.
  catalog.remember('codex', { id: 1 }, 'new')
  // @ts-expect-error Confirmation delay must be numeric milliseconds.
  catalog.schedule('codex', 'a', '800')
  // @ts-expect-error A catalog response must be a list, not an envelope.
  catalog.reconcile('codex', { data: [] })
}
void check
