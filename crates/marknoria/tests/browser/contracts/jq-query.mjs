export async function jqQueryContract(moduleUrl) {
  const { mountJqQuery, readerJqProvider } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const root = document.createElement('main'),
    focus = document.createElement('button')
  focus.textContent = 'Host'
  root.append(focus)
  document.body.append(root)
  focus.focus()
  const revision = 'public-jq-revision',
    translate = createTranslator(),
    closed = [],
    requests = [],
    copied = [],
    states = []
  let query,
    mode = 'valid',
    currentId,
    foreignId = crypto.randomUUID(),
    late,
    lateSignal,
    assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0))
  const wait = async (test) => {
    const deadline = performance.now() + 3000
    while (!test() && performance.now() < deadline) await tick()
    if (!test()) throw Error('jq did not settle')
  }
  const result = (id, page = 0) => ({
    kind: 'jqQuery',
    id,
    revision,
    page,
    pages: 2,
    totalValues: 101,
    text: page
      ? '["last",null,9007199254740993]\n'
      : '{"id":"0003","literal":"<script>","unicode":"中文😀"}\n',
    elapsedMs: 1.5
  })
  const provider = {
    run: async (request) => {
      requests.push(request)
      if (request.filter === 'missing_filter') throw Error('Reader request failed (400): Invalid jq filter at line 1, column 1: undefined filter missing_filter/0 <script>inert</script>')
      currentId = crypto.randomUUID()
      if (mode === 'late') {
        lateSignal = request.signal
        return new Promise((resolve) => (late = () => resolve(result(currentId))))
      }
      const value = result(currentId)
      if (mode === 'foreign-revision') value.revision = 'other'
      if (mode === 'oversized') value.text = '😀'.repeat(12289)
      if (mode === 'invalid-kind') value.kind = 'sqlQuery'
      return value
    },
    page: async (request) => {
      requests.push(request)
      const value = result(request.id, request.page)
      if (mode === 'foreign-page') value.id = foreignId
      if (mode === 'wrong-page') value.page = 0
      if (mode === 'changed-total') value.totalValues++
      if (mode === 'changed-pages') value.pages++
      return value
    },
    close: async ({ id }) => closed.push(id)
  }
  const mount = (options) =>
    mountJqQuery(root, {
      provider,
      revision,
      translate,
      copy: async (text) => copied.push(text),
      onState: (value) => states.push(value),
      ...options
    })
  const initialStyles = document.querySelectorAll('link[href$="/jq-query.css"]').length
  try {
    query = mount()
    await query.ready
    check(
      requests.length === 0 && query.state().id === null,
      'Ordinary jq unexpectedly computed a full input'
    )
    await query.execute('select(.active)', { slurp: true })
    check(
      requests[0].filter === 'select(.active)' &&
        requests[0].slurp === true &&
        query.state().totalValues === 101,
      'jq lost filter/slurp or complete-result count'
    )
    check(
      root.querySelector('.jq-results').textContent === result(currentId).text &&
        !root.querySelector('.jq-results script'),
      'jq changed exact text or interpreted markup'
    )
    await wait(() => getComputedStyle(root.querySelector('dialog')).maxWidth !== 'none')
    const bounds = root.querySelector('dialog').getBoundingClientRect()
    check(
      bounds.left >= 0 &&
        bounds.top >= 0 &&
        bounds.right <= innerWidth &&
        bounds.bottom <= innerHeight,
      'jq dialog escaped its viewport'
    )
    const help = root.querySelector('.jq-help')
    help.open = true
    check(getComputedStyle(help).display !== 'none' && help.textContent.includes('nested array') && help.textContent.includes('jaq'), 'jq help is hidden or omits actual input/dialect semantics')
    const beforeExample = requests.length
    help.querySelector('.query-help-example button').click()
    check(requests.length === beforeExample && root.querySelector('textarea').value === '.', 'jq help example executed without Run')
    const lastExample = help.querySelector('.query-help-example:last-child')
    lastExample.scrollIntoView({ block: 'nearest' })
    const helpBounds = help.getBoundingClientRect(), exampleBounds = lastExample.getBoundingClientRect()
    check(exampleBounds.bottom <= helpBounds.bottom + 1 && exampleBounds.top >= helpBounds.top - 1, 'jq help examples are clipped or unreachable')
    check(help.scrollWidth <= help.clientWidth + 1, 'jq help overflowed horizontally')
    help.open = false
    root.querySelector('.table-query-shell').scrollTop = 0
    const beforeError = query.state().id
    await query.execute('missing_filter').catch(() => {})
    check(query.state().id === beforeError && root.querySelector('textarea').value === 'missing_filter' && !root.querySelector('.jq-run').disabled, 'jq error lost input/results or retry')
    check(!query.state().message.includes('Reader request failed') && query.state().message.includes('line 1, column 1') && !root.querySelector('[role="status"] script'), 'jq diagnostic lost position or interpreted markup')
    await query.go(1)
    check(
      query.state().page === 1 &&
        root.querySelector('.jq-results').textContent.includes('9007199254740993'),
      'jq paging reparsed or rounded exact output'
    )
    root.querySelector('footer button:last-child').click()
    await wait(() => copied.length)
    check(copied[0] === result(currentId, 1).text, 'jq copy changed the result page')
    check(
      await query.go(2).then(
        () => false,
        () => true
      ),
      'Out-of-range jq page succeeded'
    )
    const beforeFilter = requests.length
    const rejectedWide = await query.execute('"' + '界'.repeat(6000) + '"').then(
      () => false,
      () => true
    )
    check(
      rejectedWide && requests.length === beforeFilter,
      'Oversized UTF-8 jq filter reached the provider'
    )
    root.querySelector('textarea').value = '"' + '界'.repeat(6000) + '"'
    root.querySelector('.jq-run').click()
    await tick()
    check(
      query.state().message.includes('16 KiB') && requests.length === beforeFilter,
      'UI silently ignored a bounded-filter rejection'
    )
    const kept = query.state().id
    for (const invalid of ['foreign-page', 'wrong-page', 'changed-total', 'changed-pages']) {
      mode = invalid
      await query.go(1).catch(() => {})
      check(
        query.state().id === kept &&
          query.state().page === 1 &&
          root.querySelector('.jq-results').textContent === result(kept, 1).text,
        'Malformed jq page displaced the last immutable window'
      )
    }
    check(!closed.includes(foreignId), 'A bad page closed an unowned jq result')
    for (const invalid of ['foreign-revision', 'oversized', 'invalid-kind']) {
      mode = invalid
      await query.execute('.').catch(() => {})
      check(
        query.state().id === kept && closed.includes(currentId) && !closed.includes(kept),
        'Rejected jq candidate leaked or displaced the valid result'
      )
    }
    mode = 'valid'
    await query.execute('.')
    check(
      query.state().id !== kept && closed.includes(kept),
      'Accepted jq replacement retained the old result'
    )
    const text = root.querySelector('.jq-results').textContent,
      requestCount = requests.length,
      stateCount = states.length
    translate.update({
      locale: 'fr',
      messages: {
        'Query JSON with jq': 'Interroger JSON',
        'Run filter': 'Exécuter',
        'jq filter': 'Filtre jq',
        'Page {page} / {pages} · {values} values': 'Page {page} / {pages} · {values} valeurs',
        [text.trim()]: 'WRONG DATA'
      }
    })
    check(
      root.querySelector('h2').textContent === 'Interroger JSON' &&
        root.querySelector('.jq-run').textContent === 'Exécuter' &&
        root.querySelector('textarea').getAttribute('aria-label') === 'Filtre jq',
      'Open jq UI did not update locale'
    )
    check(
      root.querySelector('.jq-results').textContent === text &&
        requests.length === requestCount &&
        states.length === stateCount,
      'jq locale change reran input, changed data or emitted computation state'
    )
    root.querySelector('textarea').value = '.unicode'
    root
      .querySelector('textarea')
      .dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }))
    await wait(() => !query.state().busy)
    check(requests.at(-1).filter === '.unicode', 'jq Ctrl+Enter lost the current filter')
    const active = query.state().id
    mode = 'late'
    const pending = query.execute('.late')
    await wait(() => late)
    await query.dispose()
    check(
      lateSignal.aborted && closed.includes(active),
      'jq disposal did not abort the provider and close the active result'
    )
    const before = states.length
    late()
    await pending
    await tick()
    check(
      closed.includes(currentId) && !root.querySelector('dialog') && states.length === before,
      'Late jq result leaked or resurrected UI'
    )
    check(
      document.activeElement === focus &&
        translate.state().subscribers === 0 &&
        document.querySelectorAll('link[href$="/jq-query.css"]').length === initialStyles,
      'jq disposal retained styles/listeners or lost host focus'
    )
    mode = 'valid'
    query = mount({ autoRun: true })
    await query.ready
    check(
      query.state().id && requests.at(-1).filter === '.' && !requests.at(-1).slurp,
      'Same-node jq autoRun failed to use default filter'
    )
    const second = mount({ autoRun: true })
    await second.ready
    check(second.state().id !== query.state().id, 'Independent jq instances shared a result')
    const secondId = second.state().id
    await second.dispose()
    check(
      closed.includes(secondId) && !closed.includes(query.state().id),
      'One jq disposal closed another instance result'
    )
    await query.dispose()
    const fill = Array.from({ length: 256 }, () => translate.subscribe(() => {}))
    let failed = false
    try {
      mount()
    } catch {
      failed = true
    }
    check(
      failed &&
        !root.querySelector('dialog') &&
        document.querySelectorAll('link[href$="/jq-query.css"]').length === initialStyles,
      'Exhausted jq setup leaked styles or DOM'
    )
    for (const remove of fill) remove()
    const calls = [],
      controller = new AbortController(),
      adapter = readerJqProvider(
        {
          request: (...args) => {
            calls.push(args)
            return Promise.resolve()
          }
        },
        { documentId: 'owner', revision }
      )
    await adapter.run({ filter: '.', slurp: false, signal: controller.signal })
    await adapter.page({ id: foreignId, page: 1, signal: controller.signal })
    await adapter.close({ id: foreignId })
    check(
      calls.map((call) => call[0]).join(',') === 'jq,jqPage,jqClose' &&
        calls.every((call) => call[1].documentId === 'owner' && call[1].revision === revision) &&
        calls[0][2].signal === controller.signal,
      'Public jq adapter lost owner/revision, cancellation or command authority'
    )
    return {
      assertions,
      exactText: true,
      completeInputProvider: true,
      immutablePaging: true,
      liveLocale: true,
      cancellation: true,
      lifecycle: true
    }
  } finally {
    await query?.dispose()
    root.remove()
  }
}
