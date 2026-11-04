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
            return `${fileDisplayName(anchor.filePath)} · ${anchor.sheet || 'Sheet'}!${anchor.range}${anchor.sql ? ' · SQL result' : ''}`;
        },
        promptAnchor(draft) {
            const anchor = normalizeTableAnchor(draft?.source?.anchor);
            return [anchor.filePath, `${anchor.sheet || 'Sheet'}!${anchor.range}`, anchor.header && `header ${anchor.header}`, anchor.sql && 'SQL result cell', anchor.documentHash && `base ${anchor.documentHash}`].filter(Boolean).join(' / ');
        },
        promptInstruction(draft, context) {
            const instruction = context.translate?.('Table excerpt: answer using the sheet, cell range, headers, and quoted value.') || '';
            const { sql } = normalizeTableAnchor(draft?.source?.anchor);
            return sql ? `${instruction}\nSQL result coordinates refer to the result of this read-only query, not raw source cells. Stored query (JSON string; preserve decoded whitespace): ${JSON.stringify(sql)}` : instruction;
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
        ...(anchor.header === undefined ? {} : { header: bounded(anchor.header, 512) }),
        ...(anchor.sql === undefined ? {} : { sql: anchor.sql === null ? null : String(anchor.sql).slice(0, 16384) }),
        ...(anchor.delimiter === undefined ? {} : { delimiter: anchor.delimiter === null ? null : String(anchor.delimiter).slice(0, 1) }),
    };
}
function bounded(value, limit) { return String(value || '').trim().slice(0, limit); }
