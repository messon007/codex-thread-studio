import { hydrateOpenCodeHistoryMetadata } from '../ui-src/history-installation.mjs'
import type { OpenCodeEventModel } from '../ui-src/opencode-event-types.mjs'

function check(model: OpenCodeEventModel) {
  // @ts-expect-error Completeness is a boolean, not a backend status label.
  hydrateOpenCodeHistoryMetadata(null, model, 'complete')
  // @ts-expect-error Message-to-turn metadata maps IDs to strings.
  hydrateOpenCodeHistoryMetadata({ messageTurns: { message: 12 } }, model)
}
void check
