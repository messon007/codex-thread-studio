export class SessionDispatchRegistry {
  constructor() {
    this.adapters = new Map()
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
    return typeof adapter.prepareTurn === 'function'
      ? adapter.prepareTurn(session, options)
      : Promise.resolve(null)
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
