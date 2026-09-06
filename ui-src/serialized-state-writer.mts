export interface StateWriteRequest {
  method: 'PUT' | 'DELETE'
  headers: { 'Content-Type': 'application/json' }
  body: string
}
export type StateWriteTransport = (path: string, request: StateWriteRequest) => Promise<{ ok: boolean; status: number }>

/** One independent FIFO per persistence domain; snapshot before joining the queue. */
export function createSerializedStateWriter(transport: StateWriteTransport, onError: (error: unknown) => void) {
  let chain = Promise.resolve()
  function write(path: string, body: Record<string, unknown>, method: 'PUT' | 'DELETE' = 'PUT'): Promise<void> {
    const payload = JSON.stringify(body)
    const pending = chain.catch(() => {}).then(async () => {
      const response = await transport(path, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: payload,
      })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
    })
    chain = pending.catch(onError)
    return pending
  }
  return { write }
}
