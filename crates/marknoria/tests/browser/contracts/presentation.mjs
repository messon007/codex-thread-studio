export async function presentationContract(moduleUrl) {
  const { readerPresentation } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const { readerUiLabels } = await import(
    new URL('./reader-ui.mjs', new URL(moduleUrl, location.href))
  )
  const root = document.createElement('section'),
    other = document.createElement('section')
  root.style.setProperty('--ui-font-size', '17px', 'important')
  root.lang = 'de'
  root.innerHTML =
    '<button id="preview-mode">Preview</button><button id="edit-mode">Edit *</button><div class="reader-settings-panel"><label>Column width <input></label></div><input id="query" aria-label="Search document" placeholder="Find words"><div id="space"><article>Preview</article></div>'
  document.body.append(root, other)
  const translate = createTranslator(),
    lifetime = new AbortController(),
    labels = readerUiLabels(root, translate, { signal: lifetime.signal })
  let first = readerPresentation(root, { translate }),
    second = readerPresentation(other),
    assertions = 0
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  try {
    const page = root.querySelector('article'),
      edit = root.querySelector('#edit-mode'),
      start = first.state()
    check(
      start.version === 1 && start.locale === 'en' && start.contentFontSize === 15,
      'Missing bounded presentation defaults'
    )
    check(start.selectionMenu === 'reader', 'Selection menu default changed')
    check(start.highlightSelectionMatches === false, 'Matching-word highlights must start disabled')
    first.update({ highlightSelectionMatches: true })
    check(first.state().highlightSelectionMatches && !second.state().highlightSelectionMatches, 'Word preference escaped its presentation scope')
    let invalidWords = false
    try { first.update({ highlightSelectionMatches: 'all', wrap: false }) } catch { invalidWords = true }
    check(invalidWords && first.state().highlightSelectionMatches && first.state().wrap, 'Word preference validation is not atomic')
    let invalidMenu = false
    try { first.update({ selectionMenu: 'invalid' }) } catch { invalidMenu = true }
    check(invalidMenu && first.state().selectionMenu === 'reader', 'Invalid menu policy was accepted')
    const changed = first.update({
      locale: 'fr',
      messages: {
        Preview: 'Aperçu',
        'Column width': 'Largeur',
        'Search document': 'Rechercher',
        'Find words': 'Mots'
      },
      theme: 'dark',
      toolbar: 'host',
      notes: 'host',
      selectionMenu: 'host',
      fontFamily: 'Noto Sans, sans-serif',
      contentFontFamily: 'serif',
      codeFontFamily: 'monospace',
      fontSize: 18,
      contentFontSize: 22,
      contentFontWeight: 400,
      codeFontSize: 19,
      wrap: false,
      palette: { brand: '#123456', panel: '#102030ff' }
    })
    check(
      changed.locale === 'fr' &&
        changed.notes === 'host' && changed.selectionMenu === 'host' &&
        root.lang === 'fr' &&
        root.dataset.theme === 'dark' &&
        root.classList.contains('host-toolbar') &&
        root.classList.contains('no-wrap'),
      'Presentation settings did not apply to their explicit root'
    )
    check(
      root.style.getPropertyValue('--content-font-size') === '22px' &&
        root.style.getPropertyValue('--code-font-size') === '19px' &&
        root.style.getPropertyValue('--content-font-weight') === '400' &&
        root.style.getPropertyValue('--content-font-family') === 'serif' &&
        root.style.getPropertyValue('--reader-brand') === '#123456',
      'Fonts/content size or palette omitted'
    )
    check(
      root.querySelector('#preview-mode').textContent === 'Aperçu' &&
        root.querySelector('.reader-settings-panel label span').textContent === 'Largeur' &&
        root.querySelector('#query').getAttribute('aria-label') === 'Rechercher',
      'Reader chrome did not localize explicit nodes'
    )
    check(
      root.querySelector('article') === page &&
        page.textContent === 'Preview' &&
        edit.textContent === 'Edit *',
      'UI translation scanned document content or reset dirty indication'
    )
    check(
      second.state().locale === 'en' && !other.hasAttribute('lang') && !other.style.length,
      'Presentation leaked into another pane'
    )
    const stable = JSON.stringify(first.state()),
      styles = root.getAttribute('style'),
      text = root.textContent
    for (const patch of [
      { theme: 'bad', fontSize: 20 },
      { notes: 'none', fontSize: 20 },
      { locale: 'bad_locale', fontSize: 20 },
      { messages: { Copy: 9 }, fontSize: 20 },
      { palette: { brand: 'url(https://example.invalid)' } },
      { palette: { unknown: '#123456' } },
      { fontFamily: 'serif; color:red' },
      { fontFamily: '"unclosed' },
      { fontSize: Infinity },
      { contentFontWeight: 99 },
      { contentFontWeight: 400.5 },
      { codeFontSize: 100 },
      { wrap: 'true' }
    ]) {
      let failed = false
      try {
        first.update(patch)
      } catch {
        failed = true
      }
      check(
        failed &&
          JSON.stringify(first.state()) === stable &&
          root.getAttribute('style') === styles &&
          root.textContent === text,
        'Invalid settings partially changed style or UI'
      )
    }
    first.update({ palette: {}, codeFontSize: null, fontFamily: null })
    check(
      !root.style.getPropertyValue('--reader-brand') &&
        !root.style.getPropertyValue('--code-font-size') &&
        !root.style.getPropertyValue('--ui-font-family'),
      'Explicit presentation reset did not restore CSS defaults'
    )
    lifetime.abort()
    check(translate.state().subscribers === 0, 'Chrome disposal retained locale listeners')
    first.dispose()
    first.dispose()
    check(
      root.lang === 'de' &&
        !root.hasAttribute('data-theme') &&
        root.style.getPropertyValue('--ui-font-size') === '17px' &&
        root.style.getPropertyPriority('--ui-font-size') === 'important' &&
        !root.classList.contains('host-toolbar'),
      'Presentation disposal did not restore host styles/attributes'
    )
    let failed = false
    try {
      first.update({ locale: 'en' })
    } catch (error) {
      failed = error.name === 'AbortError'
    }
    check(failed, 'Disposed presentation accepted an update')
    return {
      assertions,
      scoped: true,
      atomicValidation: true,
      literalData: true,
      dirtyPreserved: true,
      lifecycle: true
    }
  } finally {
    labels.dispose()
    lifetime.abort()
    first.dispose()
    second.dispose()
    root.remove()
    other.remove()
  }
}
