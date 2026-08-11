export const BROWSER_COMMENT_PROVIDER = 'browser'

export function browserCommentSource({ url, title = '', targetId = null, selector = null } = {}) {
  return {
    provider: BROWSER_COMMENT_PROVIDER,
    version: 1,
    anchor: { url, title, targetId, selector },
  }
}

export function createBrowserCommentProvider() {
  return {
    id: BROWSER_COMMENT_PROVIDER,
    normalizeAnchor(anchor = {}) {
      return {
        url: safeHttpUrl(anchor.url),
        title: bounded(anchor.title, 512),
        targetId: bounded(anchor.targetId, 256) || null,
        selector: bounded(anchor.selector, 4096) || null,
      }
    },
    describe(draft, context) {
      const { title, url } = draft.source.anchor
      return title || hostLabel(url) || context.unknownLabel || 'Web comment'
    },
    promptAnchor(draft) {
      return draft.source.anchor.url
    },
    async reopen(draft, context) {
      const url = draft.source.anchor.url
      if (url) await context.openWebSource?.(url)
    },
  }
}

function safeHttpUrl(value) {
  try {
    const url = new URL(String(value || ''))
    return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password
      ? url.toString()
      : ''
  } catch {
    return ''
  }
}

function hostLabel(value) {
  try { return new URL(value).host }
  catch { return '' }
}

function bounded(value, limit) {
  return String(value || '').trim().slice(0, limit)
}
