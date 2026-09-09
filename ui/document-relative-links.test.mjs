import test from 'node:test'
import assert from 'node:assert/strict'
import { resolveMarkdownFileLink } from './document-review.mjs'

test('document links resolve relative to the document, without changing absolute links', () => {
  for (const [href, document, expected] of [
    ['./guide.md', 'docs/index.md', 'docs/guide.md'],
    ['../guide.md', '/project/docs/index.md', '/project/docs/../guide.md'],
    ['sub/page.md', '/tmp/shared/index.md', '/tmp/shared/sub/page.md'],
    ['next%20page.md#L3', 'C:\\project\\docs\\index.md', 'C:/project/docs/next page.md'],
    ['/tmp/absolute.md', 'docs/index.md', '/tmp/absolute.md'],
    ['C:\\docs\\page.md', 'docs/index.md', 'C:\\docs\\page.md'],
    ['guide.md', '', 'guide.md'],
  ]) assert.equal(resolveMarkdownFileLink(href, document).path, expected)
  assert.equal(resolveMarkdownFileLink('next.md#L3', 'docs/index.md').line, 3)
  assert.equal(resolveMarkdownFileLink('#section', 'docs/index.md'), null)
  assert.equal(resolveMarkdownFileLink('https://example.com', 'docs/index.md'), null)
})
