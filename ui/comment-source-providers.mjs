// Generated from ui-src; run npm run build:ui. Do not edit.
import { commentRecord } from './comment-core.mjs';
import { createFileRangeTarget, fileAnnotationAnchor, fileDisplayName, lineRangeForTarget, normalizeAnnotationTarget, } from './document-review.mjs';
export const CHAT_COMMENT_PROVIDER = 'chat';
export const DOCUMENT_COMMENT_PROVIDER = 'document';
export function chatCommentSource({ turnId = null, itemId = null, startOffset = null, endOffset = null, sessionKey = null } = {}) {
    return {
        provider: CHAT_COMMENT_PROVIDER,
        version: 1,
        anchor: {
            turnId: boundedIdentifier(turnId),
            itemId: boundedIdentifier(itemId),
            startOffset: boundedOffset(startOffset),
            endOffset: boundedOffset(endOffset),
            ...(sessionKey ? { sessionKey: String(sessionKey).slice(0, 320) } : {}),
        },
    };
}
export function documentCommentSource(target) {
    const normalized = normalizeAnnotationTarget({ target });
    return {
        provider: DOCUMENT_COMMENT_PROVIDER,
        version: 1,
        anchor: normalized.kind === 'fileRange' ? { ...normalized, kind: undefined } : {},
    };
}
export function legacyCommentSource(draft = {}) {
    const target = normalizeAnnotationTarget(draft);
    return target.kind === 'fileRange'
        ? documentCommentSource(target)
        : chatCommentSource({ turnId: target.turnId, itemId: target.itemId });
}
export function createChatCommentProvider() {
    return {
        id: CHAT_COMMENT_PROVIDER,
        normalizeAnchor: normalizeChatAnchor,
        describe(draft, context) {
            const index = Number(context.index) + 1;
            const turnId = boundedIdentifier(draft.source.anchor.turnId);
            return `${context.translate?.('Reply comment {index}', { index }) || `Comment ${index}`}${turnId ? ` · ${turnId.slice(0, 8)}` : ''}`;
        },
    };
}
export function createDocumentCommentProvider() {
    return {
        id: DOCUMENT_COMMENT_PROVIDER,
        normalizeAnchor: normalizeDocumentAnchor,
        describe(draft, context) {
            const target = fileTarget(draft.source.anchor);
            if (target.readerReference) {
                const reference = commentRecord(target.readerReference);
                if (reference.querySnapshot)
                    return `${fileDisplayName(target.filePath)} · SQL result snapshot`;
                return `${fileDisplayName(target.filePath)} · ${commentRecord(reference.location).kind} · ${reference.precision}`;
            }
            const range = lineRangeForTarget(target, context.contentForSource?.(draft.source));
            const label = range.startLine
                ? ` · L${range.startLine}${range.endLine != null && range.endLine > range.startLine ? `–${range.endLine}` : ''}`
                : '';
            return `${fileDisplayName(target.filePath)}${label}`;
        },
        promptAnchor(draft, context) {
            const target = fileTarget(draft.source.anchor);
            if (target.readerReference) {
                const reference = commentRecord(target.readerReference);
                if (reference.querySnapshot) {
                    const snapshot = commentRecord(reference.querySnapshot), anchor = commentRecord(snapshot.anchor);
                    return [target.filePath, 'SQL result snapshot; not a source cell', `source revision ${snapshot.revision}`,
                        `provenance ${JSON.stringify(anchor.query)}`, anchor.quoteTruncated ? 'truncated excerpt' : ''].filter(Boolean).join(' / ');
                }
                return [target.filePath, `reader v1 ${JSON.stringify(reference.location)}`,
                    `precision ${reference.precision}`, `revision ${reference.revision}`,
                    commentRecord(reference.location).kind === 'queryCell' ? 'transient SQL result; not a source location' : '',
                    reference.quoteTruncated ? 'truncated excerpt' : ''].filter(Boolean).join(' / ');
            }
            return fileAnnotationAnchor(target, context.contentForSource?.(draft.source));
        },
        promptInstruction(draft, context) {
            if (commentRecord(fileTarget(draft.source.anchor).readerReference).querySnapshot) {
                return context.translate?.('SQL result snapshots are historical derived data. Use the saved value, query and source revision; result coordinates are not source rows. Opening a snapshot does not rerun its query.') || '';
            }
            return context.translate?.('Document comments: read the current file at each annotated path, then make changes using the quote, location, and surrounding context.') || '';
        },
        async reopen(draft, context) {
            const target = fileTarget(draft.source.anchor);
            if (commentRecord(target.readerReference).querySnapshot) {
                await context.openQuerySnapshot?.(commentRecord(commentRecord(target.readerReference).querySnapshot), target);
                return;
            }
            await context.openDocument?.(target, draft.excerpt);
        },
    };
}
export function relocateDocumentComment(value, file, excerpt) {
    const source = commentRecord(value);
    const target = fileTarget(source?.anchor);
    const relocated = createFileRangeTarget(file, excerpt);
    return {
        target,
        startOffset: relocated.startOffset ?? target.startOffset,
        endOffset: relocated.endOffset ?? target.endOffset,
    };
}
function normalizeChatAnchor(value = {}) {
    const anchor = commentRecord(value);
    return {
        turnId: boundedIdentifier(anchor.turnId),
        itemId: boundedIdentifier(anchor.itemId),
        startOffset: boundedOffset(anchor.startOffset),
        endOffset: boundedOffset(anchor.endOffset),
        ...(anchor.sessionKey ? { sessionKey: String(anchor.sessionKey).slice(0, 320) } : {}),
    };
}
function normalizeDocumentAnchor(value = {}) {
    const anchor = commentRecord(value);
    const target = normalizeAnnotationTarget({ target: { ...anchor, kind: 'fileRange' } });
    const { kind: _kind, ...normalized } = target;
    return { ...normalized,
        previewStartOffset: boundedOffset(anchor.previewStartOffset),
        previewEndOffset: boundedOffset(anchor.previewEndOffset),
        previewHash: String(anchor.previewHash || '').slice(0, 128),
    };
}
function fileTarget(value = {}) {
    return { kind: 'fileRange', ...commentRecord(value) };
}
function boundedIdentifier(value) {
    return value ? String(value).slice(0, 256) : null;
}
function boundedOffset(value) {
    if (value == null || value === '')
        return null;
    const offset = Number(value);
    return Number.isSafeInteger(offset) && offset >= 0 ? offset : null;
}
// Compare owned positions, never quote text: repeated words at different
// locations remain independent comments, and changed revisions never coalesce.
export function sameCommentSelection(left, right) {
    const a = commentRecord(commentRecord(left).source), b = commentRecord(commentRecord(right).source);
    if (!a.provider || a.provider !== b.provider)
        return false;
    const x = commentRecord(a.anchor), y = commentRecord(b.anchor);
    const rangeEqual = () => typeof x.startOffset === 'number' && Number.isSafeInteger(x.startOffset) && x.startOffset >= 0
        && typeof x.endOffset === 'number' && Number.isSafeInteger(x.endOffset) && x.endOffset > x.startOffset
        && x.startOffset === y.startOffset && x.endOffset === y.endOffset;
    if (a.provider === CHAT_COMMENT_PROVIDER)
        return Boolean(x.turnId && x.itemId
            && x.turnId === y.turnId && x.itemId === y.itemId && x.sessionKey === y.sessionKey && rangeEqual());
    if (!['document', 'epub'].includes(String(a.provider)) || !x.filePath || x.filePath !== y.filePath || x.root !== y.root)
        return false;
    if (x.readerReference || y.readerReference) {
        const r = commentRecord(x.readerReference), t = commentRecord(y.readerReference);
        if (!r.revision || r.revision !== t.revision || r.precision === 'draft' || t.precision === 'draft'
            || r.querySnapshot || t.querySnapshot || r.resourceId && t.resourceId && r.resourceId !== t.resourceId)
            return false;
        if (r.textRange || t.textRange)
            return Boolean(r.textRange && t.textRange && canonical(r.textRange) === canonical(t.textRange));
        return r.precision === 'exact' && t.precision === 'exact'
            && ['source', 'pdf', 'epub'].includes(String(commentRecord(r.location).kind))
            && canonical(r.location) === canonical(t.location);
    }
    if (a.provider === 'epub')
        return Boolean(x.bookHash && x.bookHash === y.bookHash && x.cfiRange && x.cfiRange === y.cfiRange);
    return Boolean(x.baseHash && x.baseHash === y.baseHash && rangeEqual());
}
function canonical(value) {
    if (Array.isArray(value))
        return '[' + value.map(canonical).join(',') + ']';
    if (value && typeof value === 'object')
        return '{' + Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => JSON.stringify(k) + ':' + canonical(v)).join(',') + '}';
    return JSON.stringify(value) ?? 'null';
}
