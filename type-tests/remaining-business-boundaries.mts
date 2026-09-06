import type { OpenCodeRpcParams, OpenCodeProtocolServices } from '../ui-src/opencode-protocol.mjs'
import type { ActiveMessage, ActiveConnectionState } from '../ui-src/active-codex-connection.mjs'
import type { SettingsServices } from '../ui-src/settings-application.mjs'
import type { ComposerActionsState } from '../ui-src/composer-actions.mjs'

function check(params: OpenCodeRpcParams, protocol: OpenCodeProtocolServices, message: ActiveMessage, active: ActiveConnectionState, settings: SettingsServices, composer: ComposerActionsState) {
  // @ts-expect-error History anchor IDs are strings, not numeric array indices.
  params.historyAnchorTurnIds = [1]
  // @ts-expect-error Event sequence numbers cannot be string timestamps.
  protocol.openCodeHistoryEventSequences.set('a', '3')
  // @ts-expect-error The status payload must retain structured capabilities.
  message.params = { clientCapabilities: 'all' }
  // @ts-expect-error Connection generation guards use a number.
  active.socketGeneration = 'new'
  // @ts-expect-error Settings persistence failures must be awaitable.
  settings.persistPreferences = () => undefined
  // @ts-expect-error A queue item requires a stable ID and captured input.
  composer.messageQueues.a = [{ text: 'next' }]
}
void check
