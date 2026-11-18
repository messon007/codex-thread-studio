// Test-only observations/input. Public embedded settings/navigation never use
// diagnostic globals or mutate the virtualizer to make the geometry pass.
(() => {
  const config = __MARKNORIA_LAYOUT_TEST__
  if (!config.origins.includes(location.origin)) return
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
  const wait = async (test, label) => {
    const end = performance.now() + 15000
    while (!test()) { if (performance.now() > end) throw Error(`Layout probe timeout: ${label}; ${JSON.stringify({geometry:geometry(),spaceWidth:document.getElementById('space')?.style.width})}`); await pause(20) }
  }
  const geometry = () => {
    const reader = window.readerExperiment, viewport = document.getElementById('viewport')
    const measurements = reader?.virtualizer?.getMeasurements() || []
    const pages = [...document.querySelectorAll('#space > article.page')].map(node => {
      const box = node.getBoundingClientRect(), index = Number(node.dataset.index), item = measurements[index]
      return { index, ready: node.dataset.ready === 'true', table: node.dataset.table ?? null,
        top: box.top, bottom: box.bottom, height: box.height, virtual: item?.size, start: item?.start }
    }).sort((a, b) => a.index - b.index)
    const editor = reader?.editor, code = editor?.visible ? editor.code : reader?.source?.visible ? reader.source.code : null
    return { pages, mounted: pages.length, scrollTop: viewport?.scrollTop, position: reader?.position(), pending: reader?.pendingNavigation,
      viewport: viewport ? { top:viewport.getBoundingClientRect().top, width:viewport.clientWidth, height:viewport.clientHeight } : null,
      dirty: editor?.dirty ?? false, code: code ? { text:code.view.state.doc.toString(), from:code.view.state.selection.main.from, to:code.view.state.selection.main.to } : null }
  }
  const run = async (action = 'sample', params = {}) => {
    await wait(() => window.readerExperiment?.metrics.firstContentMs != null, 'ready reader')
    const reader = window.readerExperiment, viewport = document.getElementById('viewport')
    if (action === 'font') {
      const input = document.getElementById(params.code ? 'reading-code-size' : 'reading-text-size')
      input.value = params.value == null ? '' : String(params.value)
      input.dispatchEvent(new Event('change'))
    } else if (action === 'fontLoad') {
      const face = new FontFace('LayoutLoaded', `url(data:font/woff2;base64,${config.font})`)
      document.fonts.add(face)
      await face.load(); await document.fonts.ready
    } else if (action === 'scroll') {
      viewport.dispatchEvent(new WheelEvent('wheel', { deltaY:params.top, bubbles:true }))
      viewport.scrollTop = params.top < 0 ? viewport.scrollHeight : params.top
    } else if (action === 'width') document.documentElement.style.width = `${params.width}px`
    else if (action === 'theme') window.dispatchEvent(new CustomEvent('marknoria:theme', { detail:{theme:params.theme} }))
    else if (action === 'view') {
      document.getElementById(`${params.view}-mode`).click()
      await wait(() => !document.getElementById('preview-mode').disabled && (params.view === 'edit' ? reader.editor?.visible : params.view === 'source' ? reader.source?.visible : !reader.source?.visible && !reader.editor?.visible), 'view')
    } else if (action === 'draft') {
      if (!reader.editor?.visible) throw Error('Draft input requires Edit')
      // A real CodeMirror editing transaction exercises the editor's dirty/undo
      // pipeline; execCommand is not reliably forwarded to cross-origin frames
      // by native WebKit. Never assign dirty state or replace the tested buffer.
      const view = reader.editor.code.view, selected = view.state.selection.main
      const insert = 'UNSAVED-LAYOUT-DRAFT '
      view.dispatch({ changes:{from:selected.from,to:selected.to,insert}, selection:{anchor:selected.from+insert.length} })
      await wait(() => reader.editor.dirty, 'dirty draft')
    } else if (action === 'select') {
      const code = reader.source?.code
      if (!code || !reader.source.visible) throw Error('Source selection requires Source')
      code.view.dispatch({ selection:{anchor:2,head:12} })
    } else if (action === 'go') {
      document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'g',ctrlKey:true,bubbles:true,cancelable:true}))
      const dialog = document.querySelector('.reader-go-to-line')
      if (!dialog.open) throw Error('Go to line did not open')
      dialog.querySelector('input').value = String(params.line)
      dialog.querySelector('form').requestSubmit()
      await wait(() => !dialog.open, 'Go to line')
    } else if (!['sample','geometry','settle'].includes(action)) throw Error('Unknown layout probe action')
    if (action === 'geometry') return geometry()
    if (!reader.source?.visible && !reader.editor?.visible) {
      await wait(() => {
        const value = geometry()
        return value.pending == null && value.pages.length >= 1 && value.pages.every(page => page.ready) &&
          Number.parseFloat(document.getElementById('space').style.width) === viewport.clientWidth
      }, 'mounted pages')
    } else await wait(() => !reader.source?.busy && !reader.editor?.busy, 'code view')
    const samples = []
    for (let frame = 0; frame < 8; frame++) {
      // rAF executes before layout/ResizeObserver. Read in the following task,
      // after that rendering update, rather than forcing a pre-observer layout.
      await new Promise(resolve => requestAnimationFrame(() => setTimeout(resolve, 0)))
      samples.push(geometry())
    }
    await pause(200); samples.push(geometry())
    return samples
  }
  window.readerLayoutProbe = run
  addEventListener('message', async event => {
    if (event.source !== parent || event.origin !== config.parentOrigin || event.data?.readerLayoutTest !== config.token) return
    const { id, action, params } = event.data
    try { parent.postMessage({readerLayoutTest:config.token,id,result:await run(action,params)},event.origin) }
    catch (error) { parent.postMessage({readerLayoutTest:config.token,id,error:String(error),stack:error.stack},event.origin) }
  })
})()
