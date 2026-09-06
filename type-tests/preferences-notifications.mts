import { normalizeStoredTurnOptions, sessionModelPreferencePayload } from '../ui-src/session-model-preferences.mjs'
import { codexLifecycleStreamMessage, claimLifecycleNotification } from '../ui-src/codex-lifecycle-diagnostics.mjs'
const options = normalizeStoredTurnOptions(null)
const model: string | undefined = options['codex:one']?.model
void model
// @ts-expect-error Model IDs must remain strings.
sessionModelPreferencePayload('codex:one', { model: 10 })
const message = codexLifecycleStreamMessage({})
if (message?.type === 'ready') {
  const backends: string[] = message.backends
  void backends
  // @ts-expect-error Ready frames have no event message.
  message.message
}
if (message?.type === 'event') {
  // @ts-expect-error Backend payload fields are unknown until narrowed.
  const text: string = message.message.text
  void text
}
// @ts-expect-error Status deduplication timestamps must be numbers.
claimLifecycleNotification('codex', {}, new Map<string, boolean>(), new Map<string, string>(), () => 0)
