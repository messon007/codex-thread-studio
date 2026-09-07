export async function checkRouterMentions(page) {
  return page.evaluate(async () => {
    const app = await (await fetch('/app.js')).text()
    const { fuzzyFileLabel, reviewableFileKind, replaceComposerTrigger, selectedFileReference } = await import('/composer-tools.mjs')
    const check = (value, message) => { if (!value) throw Error(message) }
    const extract = (name, next, async = false) => {
      const start = app.indexOf(`${async ? 'async ' : ''}function ${name}(`)
      const end = app.indexOf(`\n${next}`, start)
      check(start >= 0 && end > start, `missing ${name}`)
      return app.slice(start, end)
    }
    const host = document.createElement('section')
    host.innerHTML = '<div id="composer-form"><button id="router-choose-target">Automatic routing</button><textarea id="composer-input">Question @read</textarea><div id="composer-menu"></div></div>'
    document.body.append(host)
    const state = { backend: 'codex', selectedId: 'router', routerRuntime: { selectedTarget: 'opencode:worker' }, composerMenu: {
      type: 'router', trigger: { start: 9, end: 14, query: 'read' }, generation: 1, selected: 0,
      options: [{ kind: 'session', key: 'codex:books', title: 'Reading', backend: 'codex', cwd: '/books' }], fileMessage: 'Searching files',
    } }
    let requested, chooseCalls = 0, resolveSearch
    const dependencies = {
      state, $: selector => host.querySelector(selector), fuzzyFileLabel, reviewableFileKind, replaceComposerTrigger, selectedFileReference,
      t: value => value, escapeHtml: value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;'),
      backendDescriptor: backend => ({ name: backend }), randomId: () => 'fixture',
      rpc: async (_method, params) => { requested = params; return new Promise(resolve => { resolveSearch = resolve }) },
      threadRouter: { chooseTarget(key) { chooseCalls++; state.routerRuntime.selectedTarget = key } },
      renderComposerState: () => {},
      setCurrentComposerValue: value => { host.querySelector('textarea').value = value },
      hideComposerMenu: () => { state.composerMenu.type = null; state.composerMenu.generation++ },
    }
    const source = [
      extract('renderComposerMenu', 'async function performRouterFileSearch'),
      extract('performRouterFileSearch', 'function searchComposerFiles', true),
      extract('selectComposerOption', 'function showCommandDialog'),
    ].join('\n')
    const api = new Function(...Object.keys(dependencies), `${source}; return { renderComposerMenu, performRouterFileSearch, selectComposerOption }`)(...Object.values(dependencies))
    try {
      const target = { key: 'opencode:worker', cwd: '/worker' }
      const pending = api.performRouterFileSearch(state.composerMenu.trigger, 1, target)
      check(requested.roots[0] === '/worker', 'search used Router directory')
      resolveSearch({ files: [{ path: 'README.md' }] })
      await pending
      check(host.textContent.includes('Session') && host.textContent.includes('Files'), 'group headers missing')
      check(state.composerMenu.options.length === 2, 'files replaced session suggestions')
      api.selectComposerOption(1)
      check(host.querySelector('textarea').value.includes('README.md'), 'file was not inserted')
      check(chooseCalls === 0 && state.routerRuntime.selectedTarget === target.key, 'file changed routing target')
      state.composerMenu.type = 'router'
      const stale = api.performRouterFileSearch(state.composerMenu.trigger, state.composerMenu.generation, target)
      dependencies.hideComposerMenu()
      resolveSearch({ files: [{ path: 'stale.md' }] })
      await stale
      check(!state.composerMenu.options.some(option => option.path === 'stale.md'), 'closed menu accepted stale search results')
      state.composerMenu.type = 'router'
      state.composerMenu.trigger = { start: 0, end: 0, query: '' }
      state.composerMenu.options = [{ kind:'session', key:'codex:books', title:'Reading' }]
      api.selectComposerOption(0)
      check(host.querySelector('textarea').value.startsWith('@Reading '), 'Confirmed session did not insert a named reference')
      const pickerStart = app.indexOf('openTargetPicker: options => {')
      const pickerEnd = app.indexOf('\n    renderItem,',pickerStart)
      const pickerDeps = {...dependencies,renderComposerMenu:api.renderComposerMenu}
      const picker = new Function(...Object.keys(pickerDeps), 'return ({'+app.slice(pickerStart,pickerEnd)+'}).openTargetPicker')(...Object.values(pickerDeps))
      const choices = [{key:'codex:books',title:'Reading',backend:'codex',cwd:'/books'}]
      const draft = host.querySelector('textarea').value
      picker(choices)
      check(state.composerMenu.options[0].automatic && host.querySelector('#router-choose-target').getAttribute('aria-expanded')==='true','Automatic choice or open state missing')
      picker(choices)
      check(state.composerMenu.type===null && host.querySelector('textarea').value===draft,'Second click must close without changing draft')
      picker(choices)
      api.selectComposerOption(0)
      check(state.routerRuntime.selectedTarget==='' && host.querySelector('textarea').value===draft,'Switching to automatic must clear target without editing draft')
      return 'PASS: grouped sessions/files, target directory, file insertion, stale-result rejection'
    } finally { host.remove() }
  })
}
