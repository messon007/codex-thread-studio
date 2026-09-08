export interface SelectionEnvironment {
  configured?: boolean
  root?: string
  revision?: string
}

/** Lightweight per-session UI descriptors; never retains document bytes or DOM. */
export class SessionWorkspaceMemory<T> {
  private entries = new Map<string, T>()
  private generation = 0
  remember(key: string, value: T) { if (key) this.entries.set(key, value) }
  get(key: string) { return this.entries.get(key) }
  invalidate() { this.generation++ }
  begin() { const generation = ++this.generation; return () => generation === this.generation }
}

/** Initial cross-backend selection belongs to the ready handler, not a second resume. */
export async function awaitBackendSelection(effects: {
  switchBackend: () => Promise<unknown>
  waitFor: (predicate: () => boolean, timeoutMs: number) => Promise<unknown>
  ready: () => boolean
  initialLoad: () => Promise<unknown> | undefined
  catalogContainsSession: () => boolean
  freshSelection: () => boolean
}): Promise<void> {
  if (await effects.switchBackend() === false) return
  await effects.waitFor(effects.ready, 15_000)
  const load = effects.initialLoad()
  if (load) await load
  await effects.waitFor(effects.catalogContainsSession, 15_000)
  await effects.waitFor(effects.freshSelection, 30_000)
}

/** UI has already installed/rendered its cache; coordinate only the remaining work. */
export async function completeSessionSelection(options: {
  fresh: boolean
  cached: boolean
  codex: boolean
  mapLoad: Promise<unknown>
  environmentLoad: Promise<SelectionEnvironment | null>
  isCurrent: () => boolean
  status: (value: 'cached' | 'checking') => void
  resume: (options: { environmentRoot: string; environmentRevision: string }) => Promise<unknown>
  bootstrap: () => void
}): Promise<void> {
  if (options.fresh) {
    options.status('cached')
    await options.mapLoad
    await options.environmentLoad
    if (!options.isCurrent()) return
    options.bootstrap()
    return
  }
  if (options.cached) options.status('checking')
  const profile = options.codex ? await options.environmentLoad : null
  if (!options.isCurrent()) return
  await options.resume({
    environmentRoot: options.codex && profile?.configured ? profile.root || '' : '',
    environmentRevision: profile?.revision || '',
  })
  await Promise.all([options.mapLoad, options.codex ? null : options.environmentLoad])
  if (!options.isCurrent()) return
  options.bootstrap()
}
