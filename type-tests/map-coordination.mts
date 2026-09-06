import { SessionMapCoordinator } from '../ui-src/session-map-coordination.mjs'
const flow = new SessionMapCoordinator({ sessionMaps: new Map(), sessionMapLoads: new Map(), sessionMapSync: new Map() }, () => {})
// @ts-expect-error Published maps must already have passed normalization.
flow.publish('codex:a', { id: 'raw' })
flow.synchronize('codex:a', async (map, isCurrent) => isCurrent() ? map : null, { syncing: 'working', synced: 'ready' })
// @ts-expect-error Sync outcomes are checked models, unlike untrusted HTTP payloads.
flow.synchronize('codex:a', async () => ({ id: 'raw' }), { syncing: 'working', synced: 'ready' })
