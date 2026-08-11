import {
  createFileRangeTarget,
  fileAnnotationAnchor,
  fileDisplayName,
  lineRangeForTarget,
  normalizeAnnotationTarget,
} from './document-review.mjs'

export const CHAT_COMMENT_PROVIDER = 'chat'
export const DOCUMENT_COMMENT_PROVIDER = 'document'

export function chatCommentSource({ turnId = null, itemId = null } = {}) {
  return {
    provider: CHAT_COMMENT_PROVIDER,
    version: 1,
    anchor: {
      turnId: boundedIdentifier(turnId),
      itemId: boundedIdentifier(itemId),
    },
  }
}

export function documentCommentSource(target) {
  const normalized = normalizeAnnotationTarget({ target })
  return {
    provider: DOCUMENT_COMMENT_PROVIDER,
    version: 1,
    anchor: normalized.kind === 'fileRange' ? { ...normalized, kind: undefined } : {},
  }
}

export function legacyCommentSource(draft = {}) {
  const target = normalizeAnnotationTarget(draft)
  return target.kind === 'fileRange'
    ? documentCommentSource(target)
    : chatCommentSource({ turnId: target.turnId, itemId: target.itemId })
}

export function createChatCommentProvider() {
  return {
    id: CHAT_COMMENT_PROVIDER,
    normalizeAnchor: normalizeChatAnchor,
    describe(draft, context) {
      const index = Number(context.index) + 1
      const turnId = draft.source.anchor.turnId
      return `${context.translate?.('回复批注 {index}', { index }) || `Comment ${index}`}${turnId ? ` · ${turnId.slice(0, 8)}` : ''}`
    },
    promptAnchor(draft) {
      const { turnId, itemId } = draft.source.anchor
      return [turnId && `Turn ${turnId}`, itemId && `Item ${itemId}`].filter(Boolean).join(' / ')
    },
  }
}

export function createDocumentCommentProvider() {
  return {
    id: DOCUMENT_COMMENT_PROVIDER,
    normalizeAnchor: normalizeDocumentAnchor,
    describe(draft, context) {
      const target = fileTarget(draft.source.anchor)
      const range = lineRangeForTarget(target, context.contentForSource?.(draft.source))
      const label = range.startLine
        ? ` · L${range.startLine}${range.endLine > range.startLine ? `–${range.endLine}` : ''}`
        : ''
      return `${fileDisplayName(target.filePath)}${label}`
    },
    promptAnchor(draft, context) {
      const target = fileTarget(draft.source.anchor)
      return fileAnnotationAnchor(target, context.contentForSource?.(draft.source))
    },
    promptInstruction(_draft, context) {
      return context.translate?.('文档批注：请先读取标注路径的当前文件，再依据引用、位置和上下文完成修改。') || ''
    },
    async reopen(draft, context) {
      const target = fileTarget(draft.source.anchor)
      await context.openDocument?.(target, draft.excerpt)
    },
  }
}

export function relocateDocumentComment(source, file, excerpt) {
  const target = fileTarget(source?.anchor)
  const relocated = createFileRangeTarget(file, excerpt)
  return {
    target,
    startOffset: relocated.startOffset ?? target.startOffset,
    endOffset: relocated.endOffset ?? target.endOffset,
  }
}

function normalizeChatAnchor(anchor = {}) {
  return {
    turnId: boundedIdentifier(anchor.turnId),
    itemId: boundedIdentifier(anchor.itemId),
  }
}

function normalizeDocumentAnchor(anchor = {}) {
  const target = normalizeAnnotationTarget({ target: { ...anchor, kind: 'fileRange' } })
  const { kind: _kind, ...normalized } = target
  return normalized
}

function fileTarget(anchor = {}) {
  return { kind: 'fileRange', ...anchor }
}

function boundedIdentifier(value) {
  return value ? String(value).slice(0, 256) : null
}
