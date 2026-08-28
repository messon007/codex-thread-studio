const transcriptModelRevisions = new WeakMap()

export function markTranscriptModelChanged(model) {
  if (!model || typeof model !== 'object') return null
  const revision = (transcriptModelRevisions.get(model) || 0) + 1
  transcriptModelRevisions.set(model, revision)
  return revision
}

export function transcriptModelRevision(model) {
  return model && typeof model === 'object'
    ? transcriptModelRevisions.get(model) ?? null
    : null
}
