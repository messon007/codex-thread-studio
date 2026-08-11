import assert from 'node:assert/strict'
import test from 'node:test'
import {
  CommentSourceRegistry,
  createCommentDraft,
  normalizeCommentDrafts,
} from './comment-core.mjs'

test('Comment core stores opaque sources without knowing provider semantics', () => {
  const registry = new CommentSourceRegistry().register({
    id: 'epub',
    normalizeAnchor: (anchor) => ({ cfi: String(anchor?.cfi || '') }),
    describe: (draft) => draft.source.anchor.cfi,
  })
  const draft = createCommentDraft({
    excerpt: 'A selected passage',
    note: 'Revisit this argument',
    source: { provider: 'epub', anchor: { cfi: 'epubcfi(/6/4!/4/2)' } },
  }, { registry, idFactory: () => 'draft-1', now: () => '2026-08-12T00:00:00Z' })

  assert.deepEqual(draft, {
    id: 'draft-1',
    excerpt: 'A selected passage',
    note: 'Revisit this argument',
    createdAt: '2026-08-12T00:00:00Z',
    source: { provider: 'epub', version: 1, anchor: { cfi: 'epubcfi(/6/4!/4/2)' } },
  })
  assert.equal(registry.describe(draft), 'epubcfi(/6/4!/4/2)')
})

test('Legacy comment fields migrate into the stable model through a caller adapter', () => {
  const registry = new CommentSourceRegistry().register({ id: 'legacy', normalizeAnchor: (anchor) => anchor })
  const result = normalizeCommentDrafts({ thread: [{ quote: 'Old quote', comment: 'Old note' }] }, {
    registry,
    idFactory: () => 'migrated',
    migrateSource: () => ({ provider: 'legacy', anchor: { old: true } }),
  })

  assert.equal(result['codex:thread'][0].excerpt, 'Old quote')
  assert.equal(result['codex:thread'][0].note, 'Old note')
  assert.deepEqual(result['codex:thread'][0].source.anchor, { old: true })
})

test('Removing a provider leaves stored comments readable through the generic fallback', () => {
  const registry = new CommentSourceRegistry()
  const draft = createCommentDraft({
    excerpt: 'Preserved snapshot',
    source: { provider: 'browser', anchor: { url: 'https://example.com' } },
  }, { registry, idFactory: () => 'orphan', now: () => 'now' })

  assert.equal(draft.excerpt, 'Preserved snapshot')
  assert.equal(draft.source.provider, 'browser')
  assert.equal(registry.describe(draft, { unknownLabel: 'Saved comment' }), 'Saved comment')
})
