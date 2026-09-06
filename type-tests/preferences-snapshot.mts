import type { PreferencesState } from '../ui-src/preferences-snapshot.mjs'
import { environmentSavePayload } from '../ui-src/environment-profile.mjs'

function check(state: PreferencesState) {
  // @ts-expect-error Queue depth must be numeric, even before serialization.
  state.queueDepth = '3'
  // @ts-expect-error Explicit settings snapshots have no session draft field.
  state.annotationDrafts = {}
  // @ts-expect-error Secret removal entries are names, not name/value objects.
  const removals: Parameters<typeof environmentSavePayload>[0]['removeSecrets'] = [{ TOKEN: 'value' }]
  void removals
}
void check
