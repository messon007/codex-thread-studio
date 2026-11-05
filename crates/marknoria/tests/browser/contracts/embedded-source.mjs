export async function embeddedSourceContract(moduleUrl, sourceUrl) {
  const { createReaderClient } = await import(moduleUrl)
  const frame = document.createElement('iframe')
  frame.src = sourceUrl
  frame.title = 'Exact source reference reader'
  frame.style.cssText = 'width:640px;height:360px'
  document.body.append(frame)
  const client = createReaderClient(frame)
  let assertions = 0
  const check = (condition, message) => {
    if (!condition) throw Error(message)
    assertions++
  }
  try {
    const owner = await client.ready()
    const original = await client.request('sourceWindow', { line: 1 })
    const normalized = original.result.text.replaceAll('\r\n', '\n')
    const from = normalized.indexOf('😀')
    const reference = await client.request('sourceSelection', { start: original.result.start, end: original.result.end, fromUtf16: from, toUtf16: from + 2 })
    check(reference.quote === '😀' && reference.precision === 'exact', 'Original source reference missing')
    const result = await client.request('navigate', { location: reference.location })
    check(result.view === 'source' && result.revision === owner.revision, 'Range did not open Source')
    const selection = await client.request('selection')
    check(selection.quote === reference.quote, 'Restored source quote changed')
    check(JSON.stringify(selection.location) === JSON.stringify(reference.location), 'Original byte coordinates changed')
    check(selection.precision === 'exact', 'Source precision lost')
    check(await client.request('navigate', { location: reference.location, revision: 'b'.repeat(64) }).then(() => false, () => true), 'Stale navigation accepted')
    const range = await client.request('sourceWindow', { offset: reference.location.startByte, end: reference.location.endByte })
    check(range.result.text === '😀', 'Explicit bounded source range changed bytes')
    check(await client.request('sourceWindow', { offset: 0, end: 128 * 1024 + 1 }).then(() => false, () => true), 'Over-budget range accepted')
    return { assertions, sourceByteRoundTrip: true }
  } finally {
    await client.dispose({ force: true })
    frame.remove()
  }
}
