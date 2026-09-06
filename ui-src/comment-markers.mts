import type { CommentDraft } from './comment-types.mjs'

export function locateCommentIntervals(text: unknown, drafts: readonly CommentDraft[]) {
  const content = String(text || '')
  const intervals = []
  for (const draft of drafts || []) {
    const id = String(draft?.id || '')
    const excerpt = String(draft?.excerpt || '')
    if (!id || !excerpt) continue
    const anchor = draft?.source?.anchor || {}
    const anchoredStart = boundedOffset(anchor.startOffset)
    const anchoredEnd = boundedOffset(anchor.endOffset)
    if (anchoredStart != null
      && anchoredEnd != null
      && anchoredEnd > anchoredStart
      && content.slice(anchoredStart, anchoredEnd) === excerpt) {
      intervals.push({ id, start: anchoredStart, end: anchoredEnd })
      continue
    }
    const start = content.indexOf(excerpt)
    if (start < 0 || content.indexOf(excerpt, start + 1) >= 0) continue
    intervals.push({ id, start, end: start + excerpt.length })
  }
  return intervals
}

function boundedOffset(value: unknown) {
  if (value == null || value === '') return null
  const offset = Number(value)
  return Number.isSafeInteger(offset) && offset >= 0 ? offset : null
}

