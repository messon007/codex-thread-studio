export async function checkDocumentLinks(page) {
  return page.evaluate(async () => {
    const { resolveMarkdownFileLink } = await import('/document-review.mjs')
    const source = await (await fetch('/app.js')).text()
    const start = source.indexOf('async function handleTranscriptClick(')
    const end = source.indexOf('\nfunction ', start)
    const host = document.createElement('div')
    host.id = 'artifact-content'
    host.innerHTML = '<div class="markdown-body"><a data-resource-target="../guide.md#L3">Guide</a></div>'
    const state = { artifact: { root: '/worker', path: '/worker/docs/index.md', sourceSessionKey: 'ept-codex:worker', returnTool: 'files' } }
    const calls = []
    const handler = new Function('state', 'threadRouter', 'resolveMarkdownFileLink', 'selectedThread', 'openArtifact', '$', 'jumpArtifactToLine', `${source.slice(start, end)}; return handleTranscriptClick`)(
      state, { sourceContext: () => null }, resolveMarkdownFileLink, () => ({ cwd: '/router' }),
      async (...args) => calls.push(args), () => null, (line) => calls.push(line),
    )
    await handler({ target: host.querySelector('a'), currentTarget: host, preventDefault() {} })
    const [file, options] = calls[0]
    if (file.root !== '/worker' || file.path !== '/worker/docs/../guide.md' || file.sourceSessionKey !== 'ept-codex:worker' || options.returnTool !== 'files' || calls[1] !== 3) throw Error('Document link lost its document base, source or line anchor')
    return 'PASS: real document click handler preserves relative base, Router source session, return destination and line anchor'
  })
}
