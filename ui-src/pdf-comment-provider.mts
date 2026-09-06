import { commentRecord } from './comment-core.mjs'
import type { CommentProvider, PdfAnchor } from './comment-types.mjs'
import { fileDisplayName } from './document-review.mjs'

export const PDF_COMMENT_PROVIDER = 'pdf'

export function pdfCommentSource(anchor: unknown = {}) {
  return { provider: PDF_COMMENT_PROVIDER, version: 1, anchor: normalizePdfAnchor(anchor) }
}

export function createPdfCommentProvider(): CommentProvider {
  return {
    id: PDF_COMMENT_PROVIDER,
    normalizeAnchor: normalizePdfAnchor,
    describe(draft) {
      const anchor = normalizePdfAnchor(draft?.source?.anchor)
      return `${fileDisplayName(anchor.filePath)} · page ${anchor.page}`
    },
    promptAnchor(draft) {
      const anchor = normalizePdfAnchor(draft?.source?.anchor)
      return [anchor.filePath, `page ${anchor.page}`, anchor.rects.length && `regions ${JSON.stringify(anchor.rects)}`, anchor.documentHash && `base ${anchor.documentHash}`].filter(Boolean).join(' / ')
    },
    promptInstruction(_draft, context) {
      return context.translate?.('PDF excerpt: answer using the page, region, and quote. Treat the PDF as read-only.') || ''
    },
    async reopen(draft, context) {
      await context.openPdfSource?.(normalizePdfAnchor(draft?.source?.anchor), draft?.excerpt)
    },
  }
}

export function normalizePdfAnchor(value: unknown = {}): PdfAnchor {
  const anchor = commentRecord(value)
  return {
    root: bounded(anchor.root, 4096),
    filePath: bounded(anchor.filePath, 4096),
    documentHash: bounded(anchor.documentHash, 128),
    page: Math.max(1, Number.parseInt(String(anchor.page), 10) || 1),
    rects: Array.isArray(anchor.rects) ? anchor.rects.slice(0, 128).map((value: unknown) => {
      const rect = commentRecord(value)
      return ({
      x: finite(rect.x), y: finite(rect.y), width: finite(rect.width), height: finite(rect.height),
    })}) : [],
  }
}

function finite(value: unknown) { const number = Number(value); return Number.isFinite(number) ? Math.round(number * 10000) / 10000 : 0 }
function bounded(value: unknown, limit: number) { return String(value || '').trim().slice(0, limit) }

