import type { MarkdownDocument } from './markdown-document-parser.mjs'

interface ParserWorker {
  onmessage: ((event: { data: { result?: MarkdownDocument; error?: string } }) => void) | null
  onerror: ((event: { message?: string; preventDefault?(): void }) => void) | null
  onmessageerror: (() => void) | null
  postMessage(value: { source: string }): void
  terminate(): void
}

export function createMarkdownDocumentClient({ createWorker = () => new Worker(new URL('./markdown-document-worker.mjs', import.meta.url), { type: 'module' }) as ParserWorker, timeoutMs = 30_000 } = {}) {
  let cancelActive: (() => void) | null = null
  function cancel() { cancelActive?.() }
  function parse(source: string): Promise<MarkdownDocument> {
    cancel()
    return new Promise((resolve, reject) => {
      let worker: ParserWorker
      let settled = false
      try { worker = createWorker() } catch (error) { reject(error); return }
      const finish = (error: Error | null, result?: MarkdownDocument) => {
        if (settled) return
        settled = true
        clearTimeout(timer)
        worker.onmessage = null
        worker.onerror = null
        worker.onmessageerror = null
        worker.terminate()
        if (cancelActive === abort) cancelActive = null
        if (error) reject(error)
        else resolve(result!)
      }
      const abort = () => finish(new DOMException('Markdown parsing cancelled', 'AbortError'))
      const timer = setTimeout(() => finish(new Error('Markdown parsing timed out')), timeoutMs)
      cancelActive = abort
      worker.onmessage = ({ data }) => {
        if (data?.error) finish(new Error(data.error))
        else if (data?.result && typeof data.result.html === 'string' && Array.isArray(data.result.outline)) finish(null, data.result)
        else finish(new Error('Invalid Markdown parser response'))
      }
      worker.onerror = event => { event.preventDefault?.(); finish(new Error(event.message || 'Markdown parser worker failed')) }
      worker.onmessageerror = () => finish(new Error('Invalid Markdown parser response'))
      try { worker.postMessage({ source }) } catch (error) { finish(error instanceof Error ? error : new Error(String(error))) }
    })
  }
  return { parse, cancel }
}
