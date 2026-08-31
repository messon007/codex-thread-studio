export class SessionDispatchRegistry {
  constructor() {
    this.adapters = new Map()
    this.preparedSessions = new Set()
    this.preparationLoads = new Map()
    this.preparationGenerations = new Map()
    this.sessionPreparationGenerations = new Map()
  }

  register(backend, adapter) {
    const name = normalizeBackend(backend)
    if (!name) throw new Error('Invalid session backend')
    if (!adapter || typeof adapter.read !== 'function' || typeof adapter.startTurn !== 'function') {
      throw new Error(`Session backend ${name} must implement read and startTurn`)
    }
    this.adapters.set(name, Object.freeze({ ...adapter }))
    return this
  }

  supports(backend) {
    return this.adapters.has(normalizeBackend(backend))
  }

  backends() {
    return [...this.adapters.keys()]
  }

  read(ref, options = {}) {
    const session = normalizeSessionRef(ref)
    return this.adapterFor(session).read(session, options)
  }

  prepareTurn(ref, options = {}) {
    const session = normalizeSessionRef(ref)
    const adapter = this.adapterFor(session)
    if (typeof adapter.prepareTurn !== 'function') return Promise.resolve(null)
    if (options.alreadyActive || this.preparedSessions.has(session.key)) {
      this.preparedSessions.add(session.key)
      return Promise.resolve(null)
    }
    if (this.preparationLoads.has(session.key)) return this.preparationLoads.get(session.key)
    const backendGeneration = this.preparationGenerations.get(session.backend) || 0
    const sessionGeneration = this.sessionPreparationGenerations.get(session.key) || 0
    const preparation = Promise.resolve(adapter.prepareTurn(session, options))
      .then((result) => {
        if ((this.preparationGenerations.get(session.backend) || 0) === backendGeneration
          && (this.sessionPreparationGenerations.get(session.key) || 0) === sessionGeneration) {
          this.preparedSessions.add(session.key)
        }
        return result
      })
      .finally(() => {
        if (this.preparationLoads.get(session.key) === preparation) this.preparationLoads.delete(session.key)
      })
    this.preparationLoads.set(session.key, preparation)
    return preparation
  }

  markPrepared(ref) {
    const session = normalizeSessionRef(ref)
    if (typeof this.adapterFor(session).prepareTurn === 'function') this.preparedSessions.add(session.key)
  }

  clearPreparedSession(ref) {
    const session = normalizeSessionRef(ref)
    this.sessionPreparationGenerations.set(
      session.key,
      (this.sessionPreparationGenerations.get(session.key) || 0) + 1,
    )
    this.preparedSessions.delete(session.key)
    this.preparationLoads.delete(session.key)
  }

  clearPrepared(backend = null) {
    const normalized = backend == null ? '' : normalizeBackend(backend)
    const prefix = normalized ? `${normalized}:` : ''
    if (backend == null) {
      this.preparedSessions.clear()
      this.preparationLoads.clear()
      this.sessionPreparationGenerations.clear()
      for (const name of this.adapters.keys()) {
        this.preparationGenerations.set(name, (this.preparationGenerations.get(name) || 0) + 1)
      }
      return
    }
    if (!normalized) throw new Error('Invalid session backend')
    this.preparationGenerations.set(normalized, (this.preparationGenerations.get(normalized) || 0) + 1)
    for (const key of this.preparedSessions) if (key.startsWith(prefix)) this.preparedSessions.delete(key)
    for (const key of this.preparationLoads.keys()) if (key.startsWith(prefix)) this.preparationLoads.delete(key)
  }

  startTurn(ref, input, options = {}) {
    const session = normalizeSessionRef(ref)
    return this.adapterFor(session).startTurn(session, input, options)
  }

  adapterFor(ref) {
    const session = normalizeSessionRef(ref)
    const adapter = this.adapters.get(session.backend)
    if (!adapter) throw new Error(`Unsupported session backend: ${session.backend}`)
    return adapter
  }
}

export function isThreadNotFoundError(error) {
  const message = String(error?.message || error || '')
  return /(?:\bthread\b.{0,320}\b(?:not found|does not exist|unknown)\b|\b(?:not found|unknown)\s+thread\b)/iu.test(message)
}

export async function startTurnWithPreparation({
  registry,
  ref,
  prepare,
  start,
  recoverThreadNotFound = false,
}) {
  await prepare()
  try {
    return await start()
  } catch (error) {
    if (!recoverThreadNotFound || !isThreadNotFoundError(error)) throw error
    registry.clearPreparedSession(ref)
    await prepare()
    return start()
  }
}

export function normalizeSessionRef(value) {
  const backend = normalizeBackend(value?.backend)
  const id = typeof value?.id === 'string' ? value.id.trim().slice(0, 256) : ''
  if (!backend || !id) throw new Error('Invalid session reference')
  return Object.freeze({ backend, id, key: `${backend}:${id}` })
}

function normalizeBackend(value) {
  const backend = typeof value === 'string' ? value.trim().slice(0, 64) : ''
  return /^[a-z][a-z0-9_-]*$/u.test(backend) ? backend : ''
}
