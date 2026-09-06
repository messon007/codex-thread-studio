const transcriptModelRevisions = new WeakMap<object, number>()

export function markTranscriptModelChanged(model: unknown) {
  if (!model || typeof model !== 'object') return null
  const revision = (transcriptModelRevisions.get(model) || 0) + 1
  transcriptModelRevisions.set(model, revision)
  return revision
}

export function transcriptModelRevision(model: unknown) {
  return model && typeof model === 'object'
    ? transcriptModelRevisions.get(model) ?? null
    : null
}
