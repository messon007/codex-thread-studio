import assert from 'node:assert/strict'
import test from 'node:test'
import { CommentSourceRegistry, createCommentDraft } from './comment-core.mjs'
import { browserCommentSource, createBrowserCommentProvider } from './browser-comment-provider.mjs'

test('Browser comments plug into Comment Core through one provider', async () => {
  let reopened = ''
  const registry = new CommentSourceRegistry().register(createBrowserCommentProvider())
  const draft = createCommentDraft({
    excerpt: 'Selected web text',
    source: browserCommentSource({ url: 'https://example.com/article', title: 'Article' }),
  }, { registry, idFactory: () => 'web-1', now: () => 'now' })

  assert.equal(registry.describe(draft), 'Article')
  assert.equal(registry.promptAnchor(draft), 'https://example.com/article')
  await registry.reopen(draft, { openWebSource: async (url) => { reopened = url } })
  assert.equal(reopened, 'https://example.com/article')
})

test('Browser provider rejects active and credentialed source URLs', () => {
  const registry = new CommentSourceRegistry().register(createBrowserCommentProvider())
  const draft = createCommentDraft({
    excerpt: 'Unsafe source',
    source: browserCommentSource({ url: 'javascript:alert(1)' }),
  }, { registry, idFactory: () => 'web-2', now: () => 'now' })
  assert.equal(draft.source.anchor.url, '')
})
