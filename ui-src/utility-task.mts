export type UtilityResult<T> = { status: 'running' } | { status: 'failed'; error: string } | ({ status: 'completed' } & T)
export interface UtilityThread { turns?: readonly { status?: string; error?: { message?: string }; items?: readonly { type?: string; text?: unknown }[] }[] }

/** Polls the existing source (in-memory notifications or remote history), never both. */
export async function waitForUtilityResult<T>(options: {
  read: () => Promise<UtilityResult<T>>
  ensureCurrent: () => void
  intervalMs: number
  timeoutMs: number
  timeoutMessage: string
  errorMessage: (message: string) => string
  now?: () => number
  sleep?: (milliseconds: number) => Promise<void>
}): Promise<{ status: 'completed' } & T> {
  const now = options.now || Date.now
  const sleep = options.sleep || ((milliseconds: number) => new Promise<void>(resolve => setTimeout(resolve, milliseconds)))
  const deadline = now() + options.timeoutMs
  while (now() < deadline) {
    options.ensureCurrent()
    const result = await options.read()
    options.ensureCurrent()
    if (result.status === 'completed') return result
    if (result.status === 'failed') throw new Error(options.errorMessage(result.error))
    await sleep(options.intervalMs)
  }
  throw new Error(options.timeoutMessage)
}
