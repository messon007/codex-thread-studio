// Generated from ui-src; run npm run build:ui. Do not edit.
export function locateDocumentCommentIntervals(text, drafts, file) {
    const matching = drafts.filter(draft => draft.source?.provider === 'document'
        && draft.source.anchor.filePath === file.path
        && (!draft.source.anchor.root || draft.source.anchor.root === file.root));
    return locateCommentIntervals(text, matching.map(draft => {
        const anchor = draft.source.anchor;
        const unchanged = Boolean(!file.dirty && file.hash && anchor.previewHash === file.hash);
        return { ...draft, source: { ...draft.source, anchor: {
                    startOffset: unchanged ? anchor.previewStartOffset : null,
                    endOffset: unchanged ? anchor.previewEndOffset : null,
                } } };
    }));
}
export function locateCommentIntervals(text, drafts) {
    const content = String(text || '');
    const intervals = [];
    for (const draft of drafts || []) {
        const id = String(draft?.id || '');
        const excerpt = String(draft?.excerpt || '');
        if (!id || !excerpt)
            continue;
        const anchor = draft?.source?.anchor || {};
        const anchoredStart = boundedOffset(anchor.startOffset);
        const anchoredEnd = boundedOffset(anchor.endOffset);
        if (anchoredStart != null
            && anchoredEnd != null
            && anchoredEnd > anchoredStart
            && content.slice(anchoredStart, anchoredEnd) === excerpt) {
            intervals.push({ id, start: anchoredStart, end: anchoredEnd });
            continue;
        }
        const start = content.indexOf(excerpt);
        if (start < 0 || content.indexOf(excerpt, start + 1) >= 0)
            continue;
        intervals.push({ id, start, end: start + excerpt.length });
    }
    return intervals;
}
function boundedOffset(value) {
    if (value == null || value === '')
        return null;
    const offset = Number(value);
    return Number.isSafeInteger(offset) && offset >= 0 ? offset : null;
}
