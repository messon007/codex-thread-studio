import { commentRecord } from './comment-core.mjs'
import type { CommentProvider, CommentRecord } from './comment-types.mjs'
import {
  createFileRangeTarget,
  fileAnnotationAnchor,
  fileDisplayName,
  lineRangeForTarget,
  normalizeAnnotationTarget,
} from './document-review.mjs'

export const CHAT_COMMENT_PROVIDER = 'chat'
export const DOCUMENT_COMMENT_PROVIDER = 'document'

export function chatCommentSource({ turnId = null, itemId = null, startOffset = null, endOffset = null }: { turnId?: unknown; itemId?: unknown; startOffset?: unknown; endOffset?: unknown } = {}) {
  return {
    provider: CHAT_COMMENT_PROVIDER,
    version: 1,
    anchor: {
      turnId: boundedIdentifier(turnId),
      itemId: boundedIdentifier(itemId),
      startOffset: boundedOffset(startOffset),
      endOffset: boundedOffset(endOffset),
    },
  }
}

export function documentCommentSource(target: unknown) {
  const normalized = normalizeAnnotationTarget({ target })
  return {
    provider: DOCUMENT_COMMENT_PROVIDER,
    version: 1,
    anchor: normalized.kind === 'fileRange' ? { ...normalized, kind: undefined } : {},
  }
}

export function legacyCommentSource(draft: unknown = {}) {
  const target = normalizeAnnotationTarget(draft)
  return target.kind === 'fileRange'
    ? documentCommentSource(target)
    : chatCommentSource({ turnId: target.turnId, itemId: target.itemId })
}

export function createChatCommentProvider(): CommentProvider {
  return {
    id: CHAT_COMMENT_PROVIDER,
    normalizeAnchor: normalizeChatAnchor,
    describe(draft, context) {
      const index = Number(context.index) + 1
      const turnId = boundedIdentifier(draft.source.anchor.turnId)
      return `${context.translate?.('Reply comment {index}', { index }) || `Comment ${index}`}${turnId ? ` · ${turnId.slice(0, 8)}` : ''}`
    },
  }
}

export function createDocumentCommentProvider(): CommentProvider {
  return {
    id: DOCUMENT_COMMENT_PROVIDER,
    normalizeAnchor: normalizeDocumentAnchor,
    describe(draft, context) {
      const target = fileTarget(draft.source.anchor)
      const range = lineRangeForTarget(target, context.contentForSource?.(draft.source))
      const label = range.startLine
        ? ` · L${range.startLine}${range.endLine != null && range.endLine > range.startLine ? `–${range.endLine}` : ''}`
        : ''
      return `${fileDisplayName(target.filePath)}${label}`
    },
    promptAnchor(draft, context) {
      const target = fileTarget(draft.source.anchor)
      return fileAnnotationAnchor(target, context.contentForSource?.(draft.source))
    },
    promptInstruction(_draft, context) {
      return context.translate?.('Document comments: read the current file at each annotated path, then make changes using the quote, location, and surrounding context.') || ''
    },
    async reopen(draft, context) {
      const target = fileTarget(draft.source.anchor)
      await context.openDocument?.(target, draft.excerpt)
    },
  }
}

export function relocateDocumentComment(value: unknown, file: unknown, excerpt: unknown) {
  const source = commentRecord(value)
  const target = fileTarget(source?.anchor)
  const relocated = createFileRangeTarget(file, excerpt)
  return {
    target,
    startOffset: relocated.startOffset ?? target.startOffset,
    endOffset: relocated.endOffset ?? target.endOffset,
  }
}

function normalizeChatAnchor(value: unknown = {}) {
  const anchor = commentRecord(value)
  return {
    turnId: boundedIdentifier(anchor.turnId),
    itemId: boundedIdentifier(anchor.itemId),
    startOffset: boundedOffset(anchor.startOffset),
    endOffset: boundedOffset(anchor.endOffset),
  }
}

function normalizeDocumentAnchor(value: unknown = {}) {
  const anchor = commentRecord(value)
  const target = normalizeAnnotationTarget({ target: { ...anchor, kind: 'fileRange' } })
  const { kind: _kind, ...normalized } = target
  return normalized
}

function fileTarget(value: unknown = {}): CommentRecord {
  return { kind: 'fileRange', ...commentRecord(value) }
}

function boundedIdentifier(value: unknown) {
  return value ? String(value).slice(0, 256) : null
}

function boundedOffset(value: unknown) {
  if (value == null || value === '') return null
  const offset = Number(value)
  return Number.isSafeInteger(offset) && offset >= 0 ? offset : null
}
