// Host-owned UI configuration. It never reads or rewrites document data.
import { createTranslator } from './translation.mjs'
export const PRESENTATION_VERSION = 1
const colors = ['panel', 'soft', 'text', 'muted', 'faint', 'border', 'brand', 'brand-soft']
const fonts = {
  fontFamily: '--ui-font-family',
  contentFontFamily: '--content-font-family',
  codeFontFamily: '--code-font-family'
}
function balancedFamily(value) {
  let quote = null
  for (const character of value) {
    if (character === quote) quote = null
    else if (!quote && (character === '"' || character === "'")) quote = character
  }
  return !quote
}
export function readerPresentation(
  root,
  { translate = createTranslator(), themeRoot = root } = {}
) {
  if (!(root instanceof HTMLElement) || typeof translate.update !== 'function')
    throw TypeError('Presentation requires a root and scoped translator')
  let current = {
      version: PRESENTATION_VERSION,
      theme: themeRoot.dataset.theme === 'dark' ? 'dark' : 'light',
      toolbar: 'reader',
      notes: 'reader',
      fontFamily: null,
      codeFontFamily: null,
      contentFontFamily: null,
      fontSize: 14,
      contentFontSize: 15,
      contentFontWeight: 500,
      codeFontSize: null,
      wrap: true,
      palette: {}
    },
    stopped = false
  const original = new Map(
    [
      ...Object.values(fonts),
      '--ui-font-size',
      '--content-font-size',
      '--content-font-weight',
      '--code-font-size',
      ...colors.map((name) => '--reader-' + name)
    ].map((name) => [
      name,
      [root.style.getPropertyValue(name), root.style.getPropertyPriority(name)]
    ])
  )
  const oldLang = root.getAttribute('lang'),
    oldToolbar = root.classList.contains('host-toolbar'),
    oldWrap = root.classList.contains('no-wrap'),
    oldTheme = themeRoot.dataset.theme
  const state = () => ({
    ...current,
    palette: { ...current.palette },
    locale: translate.locale,
    messageCount: translate.state().messageCount
  })
  return {
    translate,
    state,
    update(patch) {
      if (stopped) throw new DOMException('Presentation disposed', 'AbortError')
      if (!patch || typeof patch !== 'object' || Array.isArray(patch))
        throw TypeError('Invalid presentation settings')
      const next = { ...current, palette: { ...current.palette } }
      if (patch.theme != null) {
        if (!['light', 'dark'].includes(patch.theme)) throw TypeError('Invalid theme')
        next.theme = patch.theme
      }
      if (patch.toolbar != null) {
        if (!['reader', 'host'].includes(patch.toolbar)) throw TypeError('Invalid toolbar owner')
        next.toolbar = patch.toolbar
      }
      if (patch.notes != null) {
        if (!['reader', 'host'].includes(patch.notes)) throw TypeError('Invalid Notes owner')
        next.notes = patch.notes
      }
      for (const key of Object.keys(fonts))
        if (patch[key] !== undefined) {
          if (
            patch[key] !== null &&
            (typeof patch[key] !== 'string' ||
              !/^[\p{L}\p{N} ,"'._-]{1,256}$/u.test(patch[key]) ||
              !CSS.supports('font-family', patch[key]) ||
              !balancedFamily(patch[key]))
          )
            throw TypeError('Invalid font family')
          next[key] = patch[key]
        }
      for (const key of ['fontSize', 'contentFontSize', 'codeFontSize'])
        if (patch[key] != null) {
          if (!Number.isFinite(patch[key]) || patch[key] < 11 || patch[key] > 32)
            throw TypeError('Invalid font size')
          next[key] = patch[key]
        }
      if (patch.contentFontWeight != null) {
        if (
          !Number.isInteger(patch.contentFontWeight) ||
          patch.contentFontWeight < 100 ||
          patch.contentFontWeight > 900
        )
          throw TypeError('Invalid content font weight')
        next.contentFontWeight = patch.contentFontWeight
      }
      if (patch.wrap != null) {
        if (typeof patch.wrap !== 'boolean') throw TypeError('Invalid wrap preference')
        next.wrap = patch.wrap
      }
      if (patch.palette !== undefined) {
        if (!patch.palette || typeof patch.palette !== 'object' || Array.isArray(patch.palette))
          throw TypeError('Invalid UI palette')
        next.palette = {}
        let count = 0
        for (const name in patch.palette) {
          if (!Object.hasOwn(patch.palette, name)) continue
          const value = patch.palette[name]
          if (
            ++count > colors.length ||
            !colors.includes(name) ||
            typeof value !== 'string' ||
            !/^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(value)
          )
            throw TypeError('UI palette requires supported literal hex colors')
          next.palette[name] = value
        }
      }
      // Validate locale/dictionary atomically before changing any host styles.
      if (patch.locale !== undefined || patch.messages !== undefined)
        translate.update({ locale: patch.locale, messages: patch.messages })
      themeRoot.dataset.theme = next.theme
      root.lang = translate.locale
      root.classList.toggle('host-toolbar', next.toolbar === 'host')
      root.classList.toggle('no-wrap', !next.wrap)
      for (const [key, property] of Object.entries(fonts))
        next[key] === null
          ? root.style.removeProperty(property)
          : root.style.setProperty(property, next[key])
      root.style.setProperty('--ui-font-size', `${next.fontSize}px`)
      root.style.setProperty('--content-font-size', `${next.contentFontSize}px`)
      root.style.setProperty('--content-font-weight', String(next.contentFontWeight))
      if (patch.codeFontSize === null) next.codeFontSize = null
      next.codeFontSize === null
        ? root.style.removeProperty('--code-font-size')
        : root.style.setProperty('--code-font-size', `${next.codeFontSize}px`)
      for (const name of colors) {
        const property = '--reader-' + name
        next.palette[name]
          ? root.style.setProperty(property, next.palette[name])
          : root.style.removeProperty(property)
      }
      current = next
      return state()
    },
    dispose() {
      if (stopped) return
      stopped = true
      for (const [property, [value, priority]] of original)
        value
          ? root.style.setProperty(property, value, priority)
          : root.style.removeProperty(property)
      oldLang === null ? root.removeAttribute('lang') : root.setAttribute('lang', oldLang)
      root.classList.toggle('host-toolbar', oldToolbar)
      root.classList.toggle('no-wrap', oldWrap)
      if (oldTheme === undefined) delete themeRoot.dataset.theme
      else themeRoot.dataset.theme = oldTheme
    }
  }
}
