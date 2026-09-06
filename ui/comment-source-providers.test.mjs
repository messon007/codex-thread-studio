import assert from 'node:assert/strict'
import test from 'node:test'
import { CommentSourceRegistry, createCommentDraft } from './comment-core.mjs'
import {
  createChatCommentProvider,
  createDocumentCommentProvider,
  legacyCommentSource,
} from './comment-source-providers.mjs'

const registry = new CommentSourceRegistry()
  .register(createChatCommentProvider())
  .register(createDocumentCommentProvider())

test('legacy chat targets stay internal and do not enter the user message', () => {
  const source = legacyCommentSource({ itemId: 'item-1', turnId: 'turn-1' })
  const draft = createCommentDraft({ excerpt: 'Answer', source }, { registry, idFactory: () => 'id', now: () => 'now' })
  assert.equal(draft.source.provider, 'chat')
  assert.equal(registry.promptAnchor(draft), '')
  assert.equal(draft.source.anchor.turnId, 'turn-1')
  assert.equal(draft.source.anchor.itemId, 'item-1')
})

test('chat comments preserve bounded rendered-text offsets', () => {
  const source = legacyCommentSource({ itemId: 'item-1', turnId: 'turn-1' })
  const positioned = createCommentDraft({
    excerpt: 'Answer',
    source: { ...source, anchor: { ...source.anchor, startOffset: 12, endOffset: 18 } },
  }, { registry, idFactory: () => 'id', now: () => 'now' })
  assert.deepEqual(positioned.source.anchor, {
    turnId: 'turn-1',
    itemId: 'item-1',
    startOffset: 12,
    endOffset: 18,
  })
})

test('legacy file targets migrate through the document provider', () => {
  const source = legacyCommentSource({
    target: {
      kind: 'fileRange',
      filePath: '/work/guide.md',
      root: '/work',
      baseHash: 'fnv1a64:abc',
      startOffset: 4,
      endOffset: 8,
      startLine: 2,
      endLine: 2,
    },
  })
  const draft = createCommentDraft({ excerpt: 'text', source }, { registry, idFactory: () => 'id', now: () => 'now' })
  assert.equal(draft.source.provider, 'document')
  assert.equal(registry.promptAnchor(draft), '/work/guide.md / line 2 / base fnv1a64:abc')
})
