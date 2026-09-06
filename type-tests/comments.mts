import { CommentSourceRegistry, createCommentDraft } from '../ui-src/comment-core.mjs'
import { createPdfCommentProvider } from '../ui-src/pdf-comment-provider.mjs'
import type { CommentContext, CommentSource } from '../ui-src/comment-types.mjs'

const registry = new CommentSourceRegistry().register(createPdfCommentProvider())
const draft = createCommentDraft({ excerpt: 'quote', source: { provider: 'pdf', anchor: { page: 2 } } }, { registry })
if (draft) {
  registry.describe(draft)
  registry.reopen(draft, { openPdfSource(anchor) { const page: number = anchor.page; return page } })
}
// @ts-expect-error Provider identity is required.
registry.register({ describe: () => 'invalid' })
// @ts-expect-error Source versions are numeric.
const badSource: CommentSource = { provider: 'pdf', version: '1', anchor: {} }
// @ts-expect-error PDF reopen handlers must accept PDF anchors, not URL strings.
const badContext: CommentContext = { openPdfSource: (url: string) => url }
void badSource; void badContext
