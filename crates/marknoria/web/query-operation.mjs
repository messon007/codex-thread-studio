// Reserve before dispatch: aborting a fetch alone does not stop Rust computation.
// Registration is awaited even if the owner closes in flight, so its accepted
// operation can always be cancelled with a separate, live request.
export async function queryOperation(api, revision, signal, execute) {
  signal?.throwIfAborted()
  const accepted = await api('/api/operations', AbortSignal.timeout(10000), {
    method: 'POST', body: JSON.stringify({ action: 'begin', revision }),
  })
  let cancelled = false
  const cancel = () => {
    if (cancelled) return
    cancelled = true
    void api('/api/operations', undefined, {
      method: 'POST', body: JSON.stringify({ action: 'cancel', revision, id: accepted.id }),
    }).catch(() => {})
  }
  signal?.addEventListener('abort', cancel, { once: true })
  try {
    signal?.throwIfAborted()
    return await execute(accepted.id)
  } finally {
    signal?.removeEventListener('abort', cancel)
    cancel()
  }
}
