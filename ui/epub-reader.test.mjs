import assert from 'node:assert/strict'
import test from 'node:test'

import {
  chapterLabelForHref,
  epubPaperTheme,
  flattenEpubToc,
  normalizeEpubReaderState,
} from './epub-reader.mjs'

test('EPUB reader state validates persistent presentation values', () => {
  assert.deepEqual(normalizeEpubReaderState({
    cfi: 'epubcfi(/6/2!/4/2)', progress: 2, fontScale: 0.2, theme: 'unknown', flow: 'bad', tocOpen: 1,
  }), {
    cfi: 'epubcfi(/6/2!/4/2)', chapterLabel: '', progress: 1, fontScale: 0.75, theme: 'light', flow: 'paginated', tocOpen: true,
  })
})

test('EPUB table of contents stays hierarchical but renders as bounded rows', () => {
  const toc = [{ label: 'Part I', href: 'part.xhtml', subitems: [{ label: 'Chapter 1', href: 'chapter.xhtml#one' }] }]
  assert.deepEqual(flattenEpubToc(toc), [
    { label: 'Part I', href: 'part.xhtml', depth: 0 },
    { label: 'Chapter 1', href: 'chapter.xhtml#one', depth: 1 },
  ])
  assert.equal(chapterLabelForHref(toc, 'chapter.xhtml#paragraph'), 'Chapter 1')
})

test('EPUB paper themes expose high contrast foreground and background pairs', () => {
  assert.equal(epubPaperTheme('sepia').background, '#f4ecd8')
  assert.equal(epubPaperTheme('dark').foreground, '#e8ecec')
  assert.deepEqual(epubPaperTheme('invalid'), epubPaperTheme('light'))
})
