import { SessionDispatchRegistry, normalizeSessionRef, startTurnWithPreparation } from '../ui-src/session-dispatch.mjs'
import { backendDescriptor, installBackendRegistry } from '../ui-src/backends.mjs'
import { mergeCatalogMetadata } from '../ui-src/session-catalog.mjs'

const ref = normalizeSessionRef({ backend: 'custom-codex', id: 'one' })
// @ts-expect-error Normalized references are immutable.
ref.key = 'opencode:one'
const registry = new SessionDispatchRegistry<{ title: string }, Promise<{ turnId: string }>>()
registry.register('custom-codex', {
  read: (session) => ({ title: session.id }),
  startTurn: async (session, input, options) => {
    const timeout: number | undefined = options.timeoutMs
    void input; void timeout
    return { turnId: session.key }
  },
})
const title: string = registry.read(ref).title
void title
// @ts-expect-error Adapter-specific read results retain their types.
const invalidTitle: number = registry.read(ref).title
void invalidTitle
// @ts-expect-error Required startTurn contract cannot be omitted.
registry.register('incomplete', { read: () => ({ title: '' }) })
// @ts-expect-error Session ID must not be a number.
registry.read({ backend: 'codex', id: 42 })
// @ts-expect-error Preparation's active flag must be boolean.
registry.prepareTurn(ref, { alreadyActive: 'true' })
// @ts-expect-error Dispatch timeout must be numeric.
registry.startTurn(ref, 'hello', { timeoutMs: '1000' })
const result = await startTurnWithPreparation({
  registry, ref, prepare: () => registry.prepareTurn(ref),
  start: () => registry.startTurn(ref, [{ type: 'text', text: 'hello' }]),
})
const turnId: string = result.turnId
void turnId
// @ts-expect-error The recovery wrapper must preserve the start result type.
const invalidTurn: number = result.turnId
void invalidTurn
installBackendRegistry(JSON.parse('[]')) // External configuration is deliberately unknown.
const descriptor = backendDescriptor('custom-codex')
// @ts-expect-error Descriptors are frozen at runtime and readonly in TypeScript.
descriptor.kind = 'opencode'
// @ts-expect-error Adapter kinds are distinct from arbitrary backend IDs.
mergeCatalogMetadata('custom-codex', null, null)
