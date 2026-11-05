// Runs the real Mermaid, PlantUML and D2 WASM engines in either browser engine.
export async function diagramEngineContract(moduleUrl) {
  const { createDiagramRenderer } = await import(moduleUrl)
  const NativeWorker = globalThis.Worker, create = URL.createObjectURL, revoke = URL.revokeObjectURL
  const nativeFetch = globalThis.fetch
  const workers = new Set(), urls = new Set(), owners = []
  let assertions = 0, created = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  globalThis.Worker = function (...args) {
    const worker = new NativeWorker(...args), terminate = worker.terminate.bind(worker)
    workers.add(worker); created++
    worker.terminate = () => { workers.delete(worker); terminate() }
    return worker
  }
  globalThis.Worker.prototype = NativeWorker.prototype
  URL.createObjectURL = value => { const url = create.call(URL, value); urls.add(url); return url }
  URL.revokeObjectURL = url => { urls.delete(url); return revoke.call(URL, url) }
  const owner = () => { const value = createDiagramRenderer(); owners.push(value); return value }
  let controller, root
  try {
    const first = owner()
    for (const [kind, source] of [
      ['mermaid', 'flowchart LR\nA[中文] --> B[Complete]'],
      ['plantuml', 'Alice -> Bob: Complete'],
      ['d2', 'a: Complete\nb: Diagram\na -> b'],
    ]) {
      console.info('Diagram contract: rendering ' + kind)
      const result = await first.render(kind, source, 'contract-' + crypto.randomUUID())
      check(result.svg.includes('<svg') && result.elements > 1, `${kind} did not render real SVG`)
      check(!/<script|<image|<iframe/i.test(result.svg), `${kind} retained unsafe nodes`)
      console.info('Diagram contract: rendered ' + kind)
    }
    check(workers.size === 1, 'D2 did not retain exactly its own worker')
    check(urls.size === 0, 'D2 worker URL survived initialization')
    const second = owner()
    await second.render('d2', 'other -> document', 'contract-' + crypto.randomUUID())
    check(workers.size === 2, 'Independent owners shared a worker')
    first.dispose(); first.dispose()
    check(workers.size === 1, 'Disposal did not synchronously stop the owned worker')
    await second.render('d2', 'remaining -> reader', 'contract-' + crypto.randomUUID())
    check(workers.size === 1, 'Disposal affected another owner')
    second.dispose()
    for (let n = 0; n < 3; n++) {
      const current = owner()
      const pending = current.render('d2', 'cancel -> initialization', 'contract-' + crypto.randomUUID()).then(() => false, () => true)
      const deadline = performance.now() + 3000
      while (!workers.size && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 1))
      check(workers.size === 1, 'Cancellation fixture never created a worker')
      current.dispose()
      check(await pending, 'Disposed render resolved successfully')
      check(workers.size === 0 && urls.size === 0, 'Initialization cancellation retained workers or URLs')
    }
    const invalid = owner()
    const rejected = await invalid.render('plantuml', '!include https://untrusted.example/private', 'contract-' + crypto.randomUUID()).then(() => false, e => /disabled/.test(e.message))
    check(rejected, 'External includes were accepted')
    invalid.dispose()
    const { diagramController } = await import(new URL('./diagrams.mjs', new URL(moduleUrl, location.href)))
    root = document.createElement('div'); root.style.cssText = 'position:fixed;inset:20px;overflow:auto'
    const figure = document.createElement('figure'), pre = document.createElement('pre')
    figure.dataset.diagramId = '0'; figure.dataset.diagramKind = 'd2'; pre.textContent = 'controller -> owned'; figure.append(pre); root.append(figure); document.body.append(root)
    globalThis.fetch = async (url, options) => {
      if (String(url).includes('/api/diagram-source')) return new Response(JSON.stringify({ id: 0, revision: 'controller-revision', kind: 'd2', source: pre.textContent }), { headers: { 'content-type': 'application/json' } })
      return nativeFetch(url, options)
    }
    controller = diagramController(root, () => {}, { documentId: 'controller-document', revision: 'controller-revision', token: 'synthetic', browserDiagrams: true })
    controller.mount(root)
    const deadline = performance.now() + 30000
    while (figure.dataset.diagramState !== 'rendered' && performance.now() < deadline) {
      if (figure.dataset.diagramState === 'error') throw Error(figure.textContent)
      await new Promise(resolve => setTimeout(resolve, 10))
    }
    check(figure.dataset.diagramState === 'rendered' && workers.size === 1, 'Reader controller did not render with its owned real engine')
    controller.dispose()
    check(!pre.hidden && !figure.querySelector('img') && workers.size === 0, 'Reader controller did not release its engine and DOM')
    check(workers.size === 0 && urls.size === 0, 'Owned renderer resources leaked')
    return { assertions, renders: 6, workersCreated: created, liveWorkers: workers.size, liveWorkerUrls: urls.size }
  } finally {
    controller?.dispose(); root?.remove()
    for (const value of owners) value.dispose()
    globalThis.fetch = nativeFetch
    globalThis.Worker = NativeWorker; URL.createObjectURL = create; URL.revokeObjectURL = revoke
  }
}
