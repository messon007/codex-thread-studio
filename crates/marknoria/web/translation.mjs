// Explicit UI bindings only: never walk or translate document/query content.
export const TRANSLATION_PROFILE = Object.freeze({
  maxMessages: 512,
  maxBytes: 65536,
  maxBindings: 1024,
  maxSubscribers: 256
})
const bytes = (value) => new TextEncoder().encode(value).length
const numberFormats = new Map()
function number(value, locale = 'en') {
  if (!numberFormats.has(locale)) {
    if (numberFormats.size >= 8) numberFormats.delete(numberFormats.keys().next().value)
    numberFormats.set(locale, new Intl.NumberFormat(locale))
  }
  return numberFormats.get(locale).format(value)
}
function variables(value = {}) {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw TypeError('Invalid message variables')
  const result = Object.create(null)
  let count = 0
  for (const key in value)
    if (Object.hasOwn(value, key)) {
      const item = value[key]
      if (
        ++count > 16 ||
        !/^[a-zA-Z][a-zA-Z0-9_]{0,63}$/.test(key) ||
        !['string', 'number', 'boolean'].includes(typeof item) ||
        (typeof item === 'number' &&
          (!Number.isFinite(item) || Math.abs(item) > Number.MAX_SAFE_INTEGER)) ||
        String(item).length > 256
      )
        throw TypeError('Message variables exceed their budget')
      result[key] = item
    }
  return result
}
function interpolate(text, values, locale) {
  if (typeof text !== 'string' || text.length > 16384)
    throw TypeError('Translated text exceeds its budget')
  let result = '',
    offset = 0
  for (const match of text.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/g)) {
    result +=
      text.slice(offset, match.index) +
      (Object.hasOwn(values, match[1])
        ? typeof values[match[1]] === 'number'
          ? number(values[match[1]], locale)
          : values[match[1]]
        : match[0])
    if (result.length > 16384) throw TypeError('Formatted message exceeds its budget')
    offset = match.index + match[0].length
  }
  result += text.slice(offset)
  if (result.length > 16384) throw TypeError('Formatted message exceeds its budget')
  return result
}
function checkedMessages(messages) {
  if (!messages || typeof messages !== 'object' || Array.isArray(messages))
    throw TypeError('Invalid UI messages')
  const result = Object.create(null)
  let count = 0,
    total = 0
  for (const key in messages)
    if (Object.hasOwn(messages, key)) {
      const value = messages[key]
      if (
        ++count > TRANSLATION_PROFILE.maxMessages ||
        !key.length ||
        key.length > 256 ||
        typeof value !== 'string' ||
        value.length > 4096 ||
        [...value.matchAll(/\{[A-Za-z][A-Za-z0-9_]*\}/g)].length > 32
      )
        throw TypeError('UI messages exceed their budget')
      total += bytes(key) + bytes(value)
      if (total > TRANSLATION_PROFILE.maxBytes)
        throw TypeError('UI messages exceed their byte budget')
      result[key] = value
    }
  return result
}
export function createTranslator(initial = {}) {
  let locale = 'en',
    messages = Object.create(null),
    revision = 0
  const listeners = new Set()
  const translate = (key, values) => {
    if (typeof key !== 'string' || key.length > 256) throw TypeError('Invalid UI message key')
    return interpolate(
      Object.hasOwn(messages, key) ? messages[key] : key,
      variables(values),
      locale
    )
  }
  Object.defineProperty(translate, 'locale', { get: () => locale })
  translate.state = () => ({
    locale,
    messageCount: Object.keys(messages).length,
    revision,
    subscribers: listeners.size
  })
  translate.update = (patch) => {
    if (!patch || typeof patch !== 'object' || Array.isArray(patch))
      throw TypeError('Invalid translation settings')
    const nextLocale = patch.locale ?? locale
    if (typeof nextLocale !== 'string' || nextLocale.length > 64)
      throw TypeError('Invalid UI locale')
    const canonical = Intl.getCanonicalLocales(nextLocale)
    if (canonical.length !== 1) throw TypeError('Invalid UI locale')
    const nextMessages = patch.messages === undefined ? messages : checkedMessages(patch.messages)
    locale = canonical[0]
    messages = nextMessages
    revision++
    for (const listener of [...listeners]) listener()
    return translate.state()
  }
  translate.subscribe = (listener, { signal } = {}) => {
    if (signal?.aborted) return () => {}
    if (typeof listener !== 'function' || listeners.size >= TRANSLATION_PROFILE.maxSubscribers)
      throw TypeError('UI translation subscription budget exhausted')
    listeners.add(listener)
    const remove = () => {
      listeners.delete(listener)
      signal?.removeEventListener('abort', remove)
    }
    signal?.addEventListener('abort', remove, { once: true })
    return remove
  }
  translate.update(initial)
  return translate
}

export function uiLabels(translate = (key) => key, { signal } = {}) {
  if (typeof translate !== 'function') throw TypeError('A UI translator must be a function')
  const bindings = new Set(),
    byNode = new WeakMap()
  let disposed = false
  const render = (binding) => {
    const node = binding.node.deref()
    if (!node) {
      bindings.delete(binding)
      return
    }
    const value =
      binding.date === undefined
        ? interpolate(translate(binding.key, binding.values), binding.values, translate.locale)
        : new Intl.DateTimeFormat(translate.locale || 'en').format(binding.date)
    if (binding.attribute === 'text') node.textContent = value
    else node.setAttribute(binding.attribute, value)
  }
  const refresh = () => {
    if (!disposed) for (const binding of [...bindings]) render(binding)
  }
  const unsubscribe = translate.subscribe?.(refresh, { signal })
  const dispose = () => {
    if (disposed) return
    disposed = true
    unsubscribe?.()
    bindings.clear()
    signal?.removeEventListener('abort', dispose)
  }
  signal?.addEventListener('abort', dispose, { once: true })
  if (signal?.aborted) dispose()
  const labels = {
    set(node, key, values, attribute = 'text') {
      if (disposed) throw new DOMException('UI labels disposed', 'AbortError')
      if (
        !node ||
        typeof key !== 'string' ||
        !key.length ||
        key.length > 256 ||
        !['text', 'title', 'aria-label', 'placeholder'].includes(attribute)
      )
        throw TypeError('Invalid UI binding')
      const args = variables(values)
      const value = interpolate(translate(key, args), args, translate.locale)
      if (!byNode.has(node)) byNode.set(node, new Map())
      const slots = byNode.get(node)
      let binding = slots.get(attribute)
      if (!binding || !bindings.has(binding)) {
        if (bindings.size >= TRANSLATION_PROFILE.maxBindings)
          for (const old of bindings) if (!old.node.deref()?.isConnected) bindings.delete(old)
        if (bindings.size >= TRANSLATION_PROFILE.maxBindings)
          throw TypeError('UI label binding budget exhausted')
        binding ||= { node: new WeakRef(node), attribute }
        slots.set(attribute, binding)
        bindings.add(binding)
      }
      Object.assign(binding, { key, values: args, date: undefined })
      if (attribute === 'text') node.textContent = value
      else node.setAttribute(attribute, value)
      return node
    },
    clear(node, attribute = 'text') {
      const slots = byNode.get(node),
        binding = slots?.get(attribute)
      if (binding) {
        bindings.delete(binding)
        slots.delete(attribute)
      }
    },
    date(node, value) {
      if (!Number.isFinite(value) || !Number.isFinite(new Date(value).getTime()))
        throw TypeError('Invalid UI date')
      labels.set(node, '{date}', {
        date: new Intl.DateTimeFormat(translate.locale || 'en').format(value)
      })
      const binding = byNode.get(node).get('text')
      binding.date = value
      render(binding)
      return node
    },
    refresh,
    dispose,
    state: () => ({ disposed, bindings: bindings.size })
  }
  return labels
}
