export async function translationContract(moduleUrl) {
  const { createTranslator, uiLabels, TRANSLATION_PROFILE } = await import(moduleUrl)
  const { readerMessages, readerMessageKeys } = await import(new URL('./ui-messages.mjs',new URL(moduleUrl,location.href)))
  const host = document.createElement('section'),
    other = document.createElement('section'),
    literal = document.createElement('pre')
  host.append(literal)
  document.body.append(host, other)
  literal.textContent = 'Copy {count} 中文😀 <script>literal</script>'
  const a = createTranslator(),
    b = createTranslator({ locale: 'fr', messages: { Copy: 'Copier' } })
  const lifetime = new AbortController(),
    left = uiLabels(a, { signal: lifetime.signal }),
    right = uiLabels(b)
  let assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  const button = document.createElement('button'),
    second = document.createElement('button')
  host.append(button)
  other.append(second)
  try {
    const dictionary = readerMessages(key => key)
    check(readerMessageKeys.length < TRANSLATION_PROFILE.maxMessages && createTranslator({messages:dictionary}).state().messageCount === readerMessageKeys.length, 'Host vocabulary exceeds translation profile')
    check(dictionary['Clear comments'] === 'Clear comments' && dictionary['Filter contents'] === 'Filter contents', 'Shared vocabulary lost Notes or outline keys')
    left.set(button, 'Copy')
    right.set(second, 'Copy')
    check(
      button.textContent === 'Copy' && second.textContent === 'Copier',
      'Translation crossed owning scopes'
    )
    a.update({ locale: 'zh-cn', messages: { Copy: '复制', 'Rows {count}': '共 {count} 行' } })
    check(
      button.textContent === '复制' && a.locale === 'zh-CN',
      'Existing UI labels did not change locale'
    )
    left.set(button, 'Rows {count}', { count: 120 })
    left.set(button, 'Copy', {}, 'aria-label')
    check(
      button.textContent === '共 120 行' && button.getAttribute('aria-label') === '复制',
      'Message parameters or accessibility text changed'
    )
    check(
      literal.textContent === 'Copy {count} 中文😀 <script>literal</script>' &&
        !host.querySelector('script'),
      'Document content was translated or executed'
    )
    a.update({ messages: { 'Rows {count}': '<script>{count}</script>', Copy: '<img src=x>' } })
    check(
      button.textContent === '<script>120</script>' && !button.querySelector('script,img'),
      'Translation was inserted as markup'
    )
    const before = JSON.stringify(a.state())
    for (const patch of [
      { locale: 'bad locale' },
      { messages: { x: 'a'.repeat(70000) } },
      { messages: Object.fromEntries(Array.from({ length: 513 }, (_, n) => ['key' + n, 'x'])) },
      { messages: { x: '{count}'.repeat(33) } }
    ]) {
      let failed = false
      try {
        a.update(patch)
      } catch {
        failed = true
      }
      check(
        failed && JSON.stringify(a.state()) === before,
        'Invalid translation partially replaced current settings'
      )
    }
    const hostile = JSON.parse(
      '{"__proto__":"Literal prototype key","constructor":"Literal constructor"}'
    )
    a.update({ messages: hostile })
    check(
      a('__proto__') === 'Literal prototype key' &&
        a('constructor') === 'Literal constructor' &&
        !{}.polluted,
      'Prototype keys escaped their dictionary'
    )
    const custom = document.createElement('button')
    host.append(custom)
    const plain = uiLabels((key, vars) =>
      typeof vars.count === 'number' ? 'Count ' + vars.count : key
    )
    plain.set(custom, 'Count {count}', { count: 3 })
    check(custom.textContent === 'Count 3', 'Host translation callbacks lost numeric parameters')
    plain.dispose()
    a.update({ locale: 'ar-EG', messages: { 'Rows {count}': 'عدد {count}' } })
    left.set(button, 'Rows {count}', { count: 123456 })
    check(
      button.textContent.includes(new Intl.NumberFormat('ar-EG').format(123456)),
      'Numeric UI variables ignored their locale'
    )
    const scoped = left.state().bindings
    left.set(button, 'Copy')
    check(left.state().bindings === scoped, 'Repeated binding retained old records')
    const date = document.createElement('time')
    host.append(date)
    const timestamp = Date.UTC(2024, 0, 2, 12)
    left.date(date, timestamp)
    a.update({ locale: 'de-DE' })
    check(
      date.textContent === new Intl.DateTimeFormat('de-DE').format(timestamp),
      'Open date labels retained another locale'
    )
    left.clear(date)
    date.textContent = 'Literal date'
    a.update({ locale: 'en-US' })
    check(date.textContent === 'Literal date', 'Cleared labels were resurrected')
    const detached = document.createElement('span')
    left.set(detached, 'Copy')
    for (let n = 0; n < TRANSLATION_PROFILE.maxBindings; n++)
      left.set(document.createElement('span'), 'Copy')
    left.set(detached, 'Copy')
    host.append(detached)
    a.update({ messages: { Copy: 'Rebound' } })
    check(
      detached.textContent === 'Rebound' &&
        left.state().bindings <= TRANSLATION_PROFILE.maxBindings,
      'Pruning broke a remounted binding or its budget'
    )
    lifetime.abort()
    const text = button.textContent
    a.update({ messages: { Copy: 'After dispose' } })
    check(
      button.textContent === text && a.state().subscribers === 0 && left.state().disposed,
      'Disposal retained subscriptions or changed DOM'
    )
    right.dispose()
    check(b.state().subscribers === 0, 'Another translator retained disposed subscribers')
    check(TRANSLATION_PROFILE.maxBytes === 65536, 'Translation profile changed silently')
    return {
      assertions,
      scoped: true,
      literalData: true,
      liveUpdates: true,
      bounded: true,
      hostCallback: true,
      lifecycle: true
    }
  } finally {
    left.dispose()
    right.dispose()
    host.remove()
    other.remove()
  }
}
