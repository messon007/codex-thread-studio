// Generated from ui-src; run npm run build:ui. Do not edit.
import { commentRecord } from './comment-core.mjs';
import { fileDisplayName } from './document-review.mjs';
export const TABLE_COMMENT_PROVIDER = 'table';
export function tableCommentSource(anchor = {}) {
    return { provider: TABLE_COMMENT_PROVIDER, version: 1, anchor: normalizeTableAnchor(anchor) };
}
export function createTableCommentProvider() {
    return {
        id: TABLE_COMMENT_PROVIDER,
        normalizeAnchor: normalizeTableAnchor,
        describe(draft) {
            const anchor = normalizeTableAnchor(draft?.source?.anchor);
            return `${fileDisplayName(anchor.filePath)} · ${anchor.sheet || 'Sheet'}!${anchor.range}`;
        },
        promptAnchor(draft) {
            const anchor = normalizeTableAnchor(draft?.source?.anchor);
            return [anchor.filePath, `${anchor.sheet || 'Sheet'}!${anchor.range}`, anchor.documentHash && `base ${anchor.documentHash}`].filter(Boolean).join(' / ');
        },
        promptInstruction(_draft, context) {
            return context.translate?.('Table excerpt: answer using the sheet, cell range, headers, and quoted value.') || '';
        },
        async reopen(draft, context) {
            await context.openTableSource?.(normalizeTableAnchor(draft?.source?.anchor), draft?.excerpt);
        },
    };
}
export function normalizeTableAnchor(value = {}) {
    const anchor = commentRecord(value);
    return {
        root: bounded(anchor.root, 4096), filePath: bounded(anchor.filePath, 4096),
        documentHash: bounded(anchor.documentHash, 128), sheet: bounded(anchor.sheet, 512),
        range: bounded(anchor.range || 'A1', 128),
    };
}
function bounded(value, limit) { return String(value || '').trim().slice(0, limit); }
