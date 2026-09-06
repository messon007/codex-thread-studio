import { coordinateHistoryLoad } from '../ui-src/history-load-coordinator.mjs'
import type { HistoryFlight } from '../ui-src/history-load-coordinator.mjs'
import { createCodexHistoryLoader } from '../ui-src/codex-history-loader.mjs'
import type { CodexHistoryServices } from '../ui-src/codex-history-types.mjs'
const flights = new Map<string, HistoryFlight<string>>()
const result: string = await coordinateHistoryLoad(flights, 'codex:one', 'codex', null, async () => 'loaded')
void result
// @ts-expect-error Flight return types must agree with their registry.
coordinateHistoryLoad(flights, 'codex:one', 'codex', null, async () => 10)
// @ts-expect-error Connection epochs must be numeric.
coordinateHistoryLoad(flights, 'opencode:one', 'opencode', 'new', async () => 'loaded')
declare const services: CodexHistoryServices
const loader = createCodexHistoryLoader(services)
await loader.loadCodexHistoryForSelection('ept-codex', 'one', null, { environmentRoot: '/work' })
// @ts-expect-error Environment revision IDs are strings.
await loader.loadCodexHistoryForSelection('codex', 'one', null, { environmentRevision: 1 })
