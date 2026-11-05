// Synthetic ordinary-DOM contract; does not inspect any reader private state.
export async function diagramLifecycleContract(moduleUrl) {
  const { diagramController } = await import(moduleUrl)
  const root = document.createElement('div'); root.style.cssText = 'position:fixed;inset:20px;width:500px;height:400px;overflow:auto;background:white;z-index:10000'
  const article = document.createElement('article'), figure = document.createElement('figure'), pre = document.createElement('pre')
  figure.dataset.diagramId = '0'; figure.dataset.diagramKind = 'mermaid'; pre.textContent = 'Public synthetic diagram'; figure.append(pre); article.append(figure); root.append(article); document.body.append(root)
  const savedFetch = window.fetch
  let requests = [], aborted = 0, delayed = false, controller, assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const until = async condition => {
    const deadline = performance.now() + 5000
    while (!condition() && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10))
    check(condition(), 'Diagram did not reach its expected state')
  }
  window.fetch = async (url, options) => {
    const id = options.headers['x-reader-document']; requests.push(id)
    if (delayed) await new Promise((_, reject) => options.signal.addEventListener('abort', () => { aborted++; reject(new DOMException('Cancelled', 'AbortError')) }, { once: true }))
    return new Response(JSON.stringify({ id: 0, revision: id, result: { svg: `<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><text>${id}</text></svg>`, elements: 2 } }), { headers: { 'content-type': 'application/json' } })
  }
  const mount = documentId => {
    controller = diagramController(root, () => {}, { documentId, revision: documentId, token: 'synthetic', browserDiagrams: false })
    controller.mount(article); controller.mount(article)
  }
  try {
    mount('public-A'); await until(() => figure.dataset.diagramState === 'rendered')
    check(figure.querySelectorAll('.diagram-canvas').length === 1, 'Repeated mount duplicated controls')
    check((await (await savedFetch(figure.querySelector('img').src)).text()).includes('public-A'), 'Wrong first document SVG')
    controller.dispose(); check(!figure.querySelector('.diagram-canvas') && !pre.hidden, 'Disposal did not restore the host DOM')
    mount('public-B'); await until(() => figure.dataset.diagramState === 'rendered')
    check((await (await savedFetch(figure.querySelector('img').src)).text()).includes('public-B'), 'Diagram reused another document SVG')
    check(requests.join(',') === 'public-A,public-B', 'Document/revision cache keys collided')
    controller.dispose(); mount('public-A'); await until(() => figure.dataset.diagramState === 'rendered')
    check(requests.length === 2, 'Same-document remount lost its bounded cache')
    controller.dispose(); delayed = true; mount('public-cancel')
    await until(() => requests.includes('public-cancel'))
    controller.dispose(); await until(() => aborted === 1)
    check(!figure.querySelector('img') && controller.tracked === 0, 'Disposed diagram mounted late or retained tracked elements')
    return { assertions, documentIsolation: true, remount: true, cancellation: true }
  } finally { controller?.dispose(); window.fetch = savedFetch; root.remove() }
}
