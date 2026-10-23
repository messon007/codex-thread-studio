import assert from 'node:assert/strict'
import test from 'node:test'

import {
  extractHtmlOutline,
  extractMarkdownOutline,
  filterDocumentOutline,
  normalizeDocumentOutline,
  outlineItemForLocation,
} from './document-outline.mjs'

test('Markdown outline ignores fenced pseudo headings and disambiguates duplicates', () => {
  const outline = extractMarkdownOutline('# Guide\n\n## Setup\n\n```md\n# Not a heading\n```\n\n## Setup')
  assert.deepEqual(outline.map(({ id, label, depth, parentId, target }) => ({ id, label, depth, parentId, line: target.line })), [
    { id: 'guide', label: 'Guide', depth: 0, parentId: '', line: 1 },
    { id: 'setup', label: 'Setup', depth: 1, parentId: 'guide', line: 3 },
    { id: 'setup-2', label: 'Setup', depth: 1, parentId: 'guide', line: 9 },
  ])
})

test('HTML outline preserves safe source order and heading levels', () => {
  const outline = extractHtmlOutline('<h1 id="start">Start</h1><script>"<h2>Hidden</h2>"</script><p>x</p><h3>Details &amp; limits</h3>')
  assert.deepEqual(outline.map(({ id, label, depth, parentId }) => ({ id, label, depth, parentId })), [
    { id: 'start', label: 'Start', depth: 0, parentId: '' },
    { id: 'details-limits', label: 'Details & limits', depth: 2, parentId: 'start' },
  ])
})

test('Markdown outline preserves source locations across paragraphs and fenced code', () => {
  const source = ['', '# Top', '', 'Paragraph', '', 'Setext', '---', '', '```md', '# Hidden', '```', '', '## End', ''].join('\n')
  const outline = extractMarkdownOutline(source)
  assert.deepEqual(outline.map(({ target }) => target.line), [2, 6, 13])
  assert.deepEqual(outline.map(({ target }) => target.offset), ['# Top', 'Setext', '## End'].map((heading) => source.indexOf(heading)))
})

test('HTML outline maps adjacent headings and skips multiline comments without shifting source lines', () => {
  const source = ['<h1>First</h1><h2>Second</h2>', '<!--<h3>Ignored</h3>', 'still ignored-->', '<p>Gap</p><h2>Third</h2>', '<h2>Fourth</h2>'].join('\r\n')
  const outline = extractHtmlOutline(source)
  assert.deepEqual(outline.map(({ target }) => target.line), [1, 1, 4, 5])
  assert.deepEqual(outline.map(({ target }) => target.offset), ['<h1>First', '<h2>Second', '<h2>Third', '<h2>Fourth'].map((heading) => source.indexOf(heading)))
})

test('outline filtering retains ancestors as context', () => {
  const outline = normalizeDocumentOutline([
    { id: 'one', label: 'One', depth: 0 },
    { id: 'two', label: 'Two', depth: 1 },
    { id: 'three', label: 'Three', depth: 2 },
  ])
  assert.deepEqual(filterDocumentOutline(outline, 'three').map(({ id, contextOnly }) => ({ id, contextOnly })), [
    { id: 'one', contextOnly: true },
    { id: 'two', contextOnly: true },
    { id: 'three', contextOnly: false },
  ])
})

test('rich document locations resolve to the closest outline item', () => {
  const epub = normalizeDocumentOutline([{ id: 'chapter', label: 'Chapter', target: { href: 'chapter.xhtml#one' } }])
  assert.equal(outlineItemForLocation(epub, 'chapter.xhtml#two')?.id, 'chapter')
  const pdf = normalizeDocumentOutline([
    { id: 'p1', label: 'Start', target: { page: 1 } },
    { id: 'p5', label: 'Later', target: { page: 5 } },
  ])
  assert.equal(outlineItemForLocation(pdf, { page: 7 })?.id, 'p5')
})

test('source lines resolve to the closest preceding preview heading', () => {
  const markdown = extractMarkdownOutline('# Guide\n\nIntro\n\n## Setup\n\nSteps\n\n## Finish')
  assert.equal(outlineItemForLocation(markdown, { line: 1 })?.id, 'guide')
  assert.equal(outlineItemForLocation(markdown, { line: 7 })?.id, 'setup')
  assert.equal(outlineItemForLocation(markdown, { line: 9 })?.id, 'finish')
  assert.equal(outlineItemForLocation(markdown, { line: 0 }), null)
})
