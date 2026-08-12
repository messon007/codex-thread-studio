import { fileDisplayName } from './document-review.mjs'

export const EPUB_COMMENT_PROVIDER = 'epub'

export function epubCommentSource(anchor = {}) {
  return {
    provider: EPUB_COMMENT_PROVIDER,
    version: 1,
    anchor: normalizeEpubAnchor(anchor),
  }
}

export function createEpubCommentProvider() {
  return {
    id: EPUB_COMMENT_PROVIDER,
    normalizeAnchor: normalizeEpubAnchor,
    describe(draft, context) {
      const anchor = normalizeEpubAnchor(draft?.source?.anchor)
      const chapter = anchor.chapterLabel ? ` · ${anchor.chapterLabel}` : ''
      return `${fileDisplayName(anchor.filePath)}${chapter}`
    },
    promptAnchor(draft) {
      const anchor = normalizeEpubAnchor(draft?.source?.anchor)
      return [
        anchor.filePath,
        anchor.chapterLabel && `chapter ${anchor.chapterLabel}`,
        anchor.cfiRange && `EPUB CFI ${anchor.cfiRange}`,
        anchor.bookHash && `base ${anchor.bookHash}`,
      ].filter(Boolean).join(' / ')
    },
    promptInstruction(_draft, context) {
      return context.translate?.('电子书摘录：请结合所附引文和我的问题进行解释；说明核心含义、上下文与关键概念，不要假设你能直接修改电子书。') || ''
    },
    async reopen(draft, context) {
      await context.openEpubSource?.(normalizeEpubAnchor(draft?.source?.anchor), draft?.excerpt)
    },
  }
}

export function normalizeEpubAnchor(anchor = {}) {
  return {
    root: bounded(anchor.root, 4096),
    filePath: bounded(anchor.filePath, 4096),
    bookHash: bounded(anchor.bookHash, 128),
    cfiRange: bounded(anchor.cfiRange, 8192),
    href: bounded(anchor.href, 8192),
    chapterLabel: bounded(anchor.chapterLabel, 1024),
  }
}

function bounded(value, limit) {
  return String(value || '').trim().slice(0, limit)
}
