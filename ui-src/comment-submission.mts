import type { CommentDraft } from './comment-types.mjs'

export interface CommentSubmissionStore {
  annotationDrafts: Record<string, CommentDraft[]>
  annotationAdditional: Record<string, string>
}
export interface CommentSubmissionOptions {
  key: string
  store: CommentSubmissionStore
  prompt: string
  composerText: string
  insert: (text: string) => void
  send: () => Promise<boolean>
  persist: (key: string) => void
}

export function insertCommentPrompt(composerText: string, prompt: string): string {
  return [composerText.trim(), prompt].filter(Boolean).join('\n\n')
}

/** A send acknowledgement, not a DOM submit event, owns draft removal. */
export class CommentSubmissionCoordinator {
  private pending = new Set<string>()

  isPending(key: string): boolean { return this.pending.has(key) }

  async submit(options: CommentSubmissionOptions): Promise<'sent' | 'retained' | 'busy'> {
    const { key, store } = options
    if (this.pending.has(key)) return 'busy'
    const drafts = store.annotationDrafts[key] || []
    if (!drafts.length) return 'retained'
    const snapshots = new Map(drafts.map(draft => [draft.id, JSON.stringify(draft)]))
    const additional = store.annotationAdditional[key]
    this.pending.add(key)
    try {
      const existing = options.composerText.trim()
      // Failed sends preserve Composer text. Retrying must not append the same batch twice.
      const alreadyInserted = existing === options.prompt || existing.endsWith(`\n\n${options.prompt}`)
      options.insert(alreadyInserted ? existing : insertCommentPrompt(existing, options.prompt))
      if (!await options.send()) return 'retained'
      const remaining = (store.annotationDrafts[key] || []).filter(draft => snapshots.get(draft.id) !== JSON.stringify(draft))
      if (remaining.length) store.annotationDrafts[key] = remaining
      else delete store.annotationDrafts[key]
      if (store.annotationAdditional[key] === additional) delete store.annotationAdditional[key]
      options.persist(key)
      return 'sent'
    } finally {
      this.pending.delete(key)
    }
  }
}
