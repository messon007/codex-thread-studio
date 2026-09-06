import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  artifactSearchAvailable,
  artifactInlineSearchAvailable,
  createFileRangeTarget,
  findTextMatchRanges,
  fileAnnotationAnchor,
  fileDisplayName,
  formatLineAnchor,
  isHtmlFile,
  lineRangeForOffsets,
  lineRangeForTarget,
  lineNumberAt,
  locateQuote,
  normalizeAnnotationTarget,
  renderStructuredTextPreview,
  resolveMarkdownFileLink,
  resolveMarkdownImagePath,
  snapshotAnnotationSelection,
  STATIC_HTML_FORBIDDEN_ATTRIBUTES,
  STATIC_HTML_FORBIDDEN_TAGS,
  structuredPreviewSourceRange,
  structuredTextPreviewKind,
} from './document-review.mjs'

const documentReviewHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const documentReviewApp = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const documentReviewStyles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
const documentWorkspaceSource = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
const reviewNotesSource = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')

function textFromHighlightedMarkup(html) {
  return String(html || '')
    .replace(/<span class="artifact-token-[a-z]+">|<\/span>/gu, '')
    .replaceAll('&#39;', "'")
    .replaceAll('&quot;', '"')
    .replaceAll('&gt;', '>')
    .replaceAll('&lt;', '<')
    .replaceAll('&amp;', '&')
}

test('locates a selected file range and preserves nearby anchors', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: 'one\ntwo\nthree' }
  const target = createFileRangeTarget(file, 'two')
  assert.equal(target.startOffset, 4)
  assert.equal(target.endOffset, 7)
  assert.deepEqual({ startLine: target.startLine, endLine: target.endLine }, { startLine: 2, endLine: 2 })
  assert.equal(target.filePath, '/work/docs/guide.md')
  assert.equal(lineNumberAt(file.content, target.startOffset), 2)
})

test('marks repeated quotes as ambiguous', () => {
  assert.deepEqual(locateQuote('same and same', 'same'), { startOffset: 0, endOffset: 4, ambiguous: true })
})

test('normalizes legacy chat annotations without losing anchors', () => {
  assert.deepEqual(normalizeAnnotationTarget({ itemId: 'i1', turnId: 't1' }), {
    kind: 'chatRange', itemId: 'i1', turnId: 't1',
  })
})

test('normalizes invalid zero-length file anchors as chat fallback fields', () => {
  assert.deepEqual(normalizeAnnotationTarget({ target: { kind: 'fileRange', filePath: '/work/docs/guide.md', baseHash: 'abc', startOffset: 0, endOffset: 0 } }), {
    kind: 'fileRange',
    filePath: '/work/docs/guide.md',
    root: '',
    baseHash: 'abc',
    startOffset: null,
    endOffset: null,
    startLine: null,
    endLine: null,
    prefix: '',
    suffix: '',
  })
})

test('does not coerce absent or partial file offsets to zero', () => {
  const target = {
    kind: 'fileRange',
    filePath: '/work/docs/guide.md',
    startOffset: null,
    endOffset: null,
  }
  assert.equal(normalizeAnnotationTarget({ target }).startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target: { ...target, endOffset: 5 } }).startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target: { ...target, startOffset: 5 } }).endOffset, null)
  assert.equal(normalizeAnnotationTarget({ target: { ...target, startOffset: '', endOffset: 5 } }).startOffset, null)
})

test('maps exclusive source offsets to an inclusive line range', () => {
  assert.deepEqual(lineRangeForOffsets('one\ntwo\nthree', 4, 13), { startLine: 2, endLine: 3 })
  assert.deepEqual(lineRangeForOffsets('one\ntwo\nthree', 4, 8), { startLine: 2, endLine: 2 })
  assert.deepEqual(lineRangeForOffsets('one\ntwo', null, 4), { startLine: null, endLine: null })
})

test('formats stable line anchors for annotation prompts', () => {
  assert.equal(formatLineAnchor(2, 2), 'line 2')
  assert.equal(formatLineAnchor(2, 4), 'lines 2-4')
  assert.equal(formatLineAnchor(null, 4), '')
  const target = {
    kind: 'fileRange', filePath: '/work/guide.md', baseHash: 'abc',
    startOffset: 4, endOffset: 13, startLine: 2, endLine: 3,
  }
  assert.deepEqual(lineRangeForTarget(target), { startLine: 2, endLine: 3 })
  assert.equal(fileAnnotationAnchor(target), '/work/guide.md / lines 2-3 / base abc')
  assert.equal(
    fileAnnotationAnchor({ ...target, startLine: null, endLine: null }, 'one\ntwo\nthree'),
    '/work/guide.md / lines 2-3 / base abc',
  )
  assert.equal(
    fileAnnotationAnchor({ ...target, startLine: null, endLine: null }),
    '/work/guide.md / offset 4-13 / base abc',
  )
})

test('keeps document search available in preview, source, and edit modes', () => {
  const file = { content: '# Document', loading: false, error: '' }
  assert.equal(artifactSearchAvailable(file, 'preview'), true)
  assert.equal(artifactSearchAvailable(file, 'source'), true)
  assert.equal(artifactSearchAvailable(file, 'edit'), true)
  assert.equal(artifactInlineSearchAvailable(file, 'preview'), true)
  assert.equal(artifactInlineSearchAvailable(file, 'edit'), false)
  assert.equal(artifactSearchAvailable({ ...file, loading: true }, 'source'), false)

  const searchPosition = documentReviewHtml.indexOf('id="artifact-search-toggle"')
  const viewPosition = documentReviewHtml.indexOf('id="artifact-view-switch"')
  assert.ok(searchPosition >= 0 && searchPosition < viewPosition)
  assert.ok(documentReviewHtml.includes('id="artifact-search-panel"'))
})

test('resolves Markdown images from the document directory without escaping the workspace', () => {
  const file = {
    root: '/work/project',
    path: '/work/project/docs/guide.md',
    relativePath: 'docs/guide.md',
  }
  assert.deepEqual(resolveMarkdownImagePath(file, 'images/example.jpg'), { path: 'docs/images/example.jpg' })
  assert.deepEqual(resolveMarkdownImagePath(file, '../images/example.jpg'), { path: 'images/example.jpg' })
  assert.deepEqual(resolveMarkdownImagePath(file, '/images/example.jpg'), { path: 'images/example.jpg' })
  assert.deepEqual(resolveMarkdownImagePath(file, 'images/a%20b.png?raw=1#preview'), { path: 'docs/images/a b.png' })
  assert.deepEqual(resolveMarkdownImagePath(file, 'data:image/png;base64,abc'), { embedded: 'data:image/png;base64,abc' })
  assert.equal(resolveMarkdownImagePath(file, '../../../outside.png'), null)
  assert.equal(resolveMarkdownImagePath(file, 'https://example.com/image.png'), null)
  assert.match(documentReviewApp, /gatewayFetch\('\/studio\/review-image'/u)
  assert.match(documentReviewApp, /hydrateMarkdownImages\(file, content\)/u)
})

test('resolves Markdown images beside a read-only shared document', () => {
  const file = {
    root: '/work/project',
    documentRoot: '/shared/library',
    path: '/shared/library/docs/guide.md',
    relativePath: '/shared/library/docs/guide.md',
    readOnly: true,
  }
  assert.deepEqual(
    resolveMarkdownImagePath(file, 'images/example.png'),
    { path: '/shared/library/docs/images/example.png' },
  )
  assert.match(documentWorkspaceSource, /const editable = textReady && !file\.readOnly/u)
  assert.match(documentWorkspaceSource, /view === 'edit' && state\.artifact\.readOnly/u)
})

test('resolves generated file links with source locations', () => {
  assert.deepEqual(
    resolveMarkdownFileLink('/home/rui/project/docs/RICO%20Profile%20%E7%AD%96%E7%95%A5%E7%9F%A9%E9%98%B5.zh-CN.md:12:4'),
    { path: '/home/rui/project/docs/RICO Profile \u7b56\u7565\u77e9\u9635.zh-CN.md', line: 12, column: 4 },
  )
  assert.deepEqual(resolveMarkdownFileLink('docs/guide.md#L8C3'), { path: 'docs/guide.md', line: 8, column: 3 })
  assert.deepEqual(resolveMarkdownFileLink('C:\\work\\docs\\guide.md:7'), { path: 'C:\\work\\docs\\guide.md', line: 7, column: undefined })
  assert.deepEqual(resolveMarkdownFileLink('./guide.md:5'), { path: 'guide.md', line: 5, column: undefined })
  assert.deepEqual(resolveMarkdownFileLink('./docs/guide.md?raw=1'), { path: 'docs/guide.md', line: undefined, column: undefined })
  assert.equal(resolveMarkdownFileLink('https://example.com/guide.md'), null)
  assert.equal(resolveMarkdownFileLink('mailto:user@example.com'), null)
})

test('document outline stays inside the document shell as a wide overlay drawer', () => {
  const shellPosition = documentReviewHtml.indexOf('id="artifact-reader-shell"')
  const outlinePosition = documentReviewHtml.indexOf('id="artifact-outline"')
  const contentPosition = documentReviewHtml.indexOf('id="artifact-content"')
  assert.ok(documentReviewHtml.includes('id="artifact-outline-toggle" class="icon-button artifact-action-icon hidden"'))
  assert.ok(shellPosition >= 0 && shellPosition < outlinePosition && outlinePosition < contentPosition)
  assert.match(documentReviewStyles, /\.artifact-reader-shell\.outline-open \.artifact-outline-backdrop \{ display: block; \}/u)
  assert.match(documentReviewStyles, /\.artifact-outline \{ width: min\(420px, calc\(100% - 20px\)\);[\s\S]*position: absolute;/u)
  assert.match(documentReviewStyles, /font: var\(--ui-font-emphasis\) var\(--ui-font-compact\)\/1\.45 var\(--ui-font-family\)/u)
  assert.doesNotMatch(documentReviewStyles, /artifact-reader-shell\.compact/u)
  assert.doesNotMatch(documentReviewApp, /artifactOutlineOpen:/u)
})

test('artifact identity uses the available header width before truncating', () => {
  const heading = documentReviewStyles.slice(
    documentReviewStyles.indexOf('.artifact-heading {'),
    documentReviewStyles.indexOf('.artifact-header-actions .segmented-control'),
  )

  assert.match(heading, /\.artifact-heading \{[^}]*flex: 1;/u)
  assert.match(heading, /\.artifact-heading > div \{ min-width: 0; flex: 1; \}/u)
  assert.match(heading, /\.artifact-heading h2 \{ max-width: 100%;/u)
  assert.match(heading, /\.artifact-heading p \{ max-width: 100%;/u)
  assert.doesNotMatch(heading, /max-width: min\(330px|max-width: min\(280px/u)
  assert.match(documentWorkspaceSource, /\$\('#artifact-path'\)\.title = artifactPath/u)
})

test('static HTML preview blocks executable and externally loaded content', () => {
  for (const tag of ['script', 'iframe', 'object', 'form', 'style']) assert.ok(STATIC_HTML_FORBIDDEN_TAGS.includes(tag))
  for (const attribute of ['src', 'href', 'style', 'srcdoc']) assert.ok(STATIC_HTML_FORBIDDEN_ATTRIBUTES.includes(attribute))
})

test('recognizes common structured text formats without treating source code as a preview', () => {
  assert.deepEqual(structuredTextPreviewKind('/work/settings.json'), { language: 'json', label: 'JSON' })
  assert.deepEqual(structuredTextPreviewKind('/work/events.ndjson'), { language: 'json', label: 'NDJSON' })
  assert.deepEqual(structuredTextPreviewKind('C:\\work\\compose.yaml'), { language: 'yaml', label: 'YAML' })
  assert.deepEqual(structuredTextPreviewKind('/work/feed.atom'), { language: 'xml', label: 'ATOM' })
  assert.deepEqual(structuredTextPreviewKind('/work/pyproject.toml'), { language: 'toml', label: 'TOML' })
  assert.deepEqual(structuredTextPreviewKind('/work/.env.local'), { language: 'config', label: 'ENV' })
  assert.deepEqual(structuredTextPreviewKind('/work/.editorconfig'), { language: 'config', label: 'EDITORCONFIG' })
  assert.deepEqual(structuredTextPreviewKind('/work/Cargo.lock'), { language: 'toml', label: 'TOML' })
  assert.equal(structuredTextPreviewKind('/work/server.log'), null)
  assert.equal(structuredTextPreviewKind('/work/query.sql'), null)
  assert.equal(structuredTextPreviewKind('/work/app.js'), null)
})

test('structured previews highlight safe display text and escape executable markup', () => {
  const samples = [
    ['/work/settings.json', '{\n  "name": "<script>alert(1)</script>",\n  "enabled": true\n}\n'],
    ['/work/compose.yaml', 'services:\n  app: &app\n    image: "demo:1" # local\n'],
    ['/work/feed.xml', '<?xml version="1.0"?>\n<root attr="x">&amp; value</root>\n'],
    ['/work/pyproject.toml', '[project]\nname = "studio"\nenabled = true\n'],
    ['/work/.env', 'API_URL = https://example.test?a=1&b=2\nDEBUG = true\n'],
  ]

  for (const [path, expected] of samples) {
    const source = path.endsWith('/.env') ? expected.replaceAll(' = ', '=') : expected
    const preview = renderStructuredTextPreview(path, source)
    assert.ok(preview?.highlighted)
    assert.equal(textFromHighlightedMarkup(preview.html), expected)
    assert.equal(preview.text, expected)
    assert.doesNotMatch(preview.html, /<script>/iu)
  }
  assert.match(renderStructuredTextPreview('/work/settings.json', '{"enabled":true}').html, /artifact-token-key/u)
  assert.match(renderStructuredTextPreview('/work/settings.json', '{"enabled":true}').html, /artifact-token-literal/u)
})

test('structured previews pretty-print nested JSON and XML without changing source values', () => {
  const jsonSource = '{"name":"studio","nested":{"enabled":true,"items":[1,2]}}'
  const json = renderStructuredTextPreview('/work/settings.json', jsonSource)
  assert.equal(json.formatted, true)
  assert.equal(json.text, [
    '{',
    '  "name": "studio",',
    '  "nested": {',
    '    "enabled": true,',
    '    "items": [',
    '      1,',
    '      2',
    '    ]',
    '  }',
    '}',
  ].join('\n'))

  const xml = renderStructuredTextPreview('/work/feed.xml', '<root><item id="1">first</item><item id="2"/></root>')
  assert.equal(xml.formatted, true)
  assert.equal(xml.text, '<root>\n  <item id="1">first</item>\n  <item id="2"/>\n</root>')

  const mixedXml = '<p>Hello <strong>there</strong>!</p>'
  const mixed = renderStructuredTextPreview('/work/page.xml', mixedXml)
  assert.equal(mixed.formatted, false)
  assert.equal(mixed.text, mixedXml)
})

test('formatted structured preview selections map back to exact source offsets', () => {
  const source = '{"name":"studio","nested":{"enabled":true,"items":[1,2]}}'
  const preview = renderStructuredTextPreview('/work/settings.json', source)
  const selected = '"nested": {\n    "enabled": true'
  const start = preview.text.indexOf(selected)
  const range = structuredPreviewSourceRange(preview.sourceMap, start, start + selected.length)
  assert.deepEqual(range, { startOffset: 17, endOffset: 41 })
  assert.equal(source.slice(range.startOffset, range.endOffset), '"nested":{"enabled":true')
  assert.equal(structuredPreviewSourceRange(Int32Array.from([-1, -1]), 0, 2), null)
})

test('structured preview formatting falls back safely for semantic indentation or malformed structure', () => {
  const yaml = 'services:\n  app:\n    image: demo\n'
  const yamlPreview = renderStructuredTextPreview('/work/compose.yaml', yaml)
  assert.equal(yamlPreview.formatted, false)
  assert.equal(yamlPreview.text, yaml)

  const malformedJson = '{"nested":[1,2}'
  const jsonPreview = renderStructuredTextPreview('/work/settings.json', malformedJson)
  assert.equal(jsonPreview.formatted, false)
  assert.equal(jsonPreview.text, malformedJson)
})

test('document workspace enables formatted structured previews with source-mapped annotation offsets', () => {
  assert.match(documentWorkspaceSource, /structuredTextPreviewKind\(result\.path\) \? 'preview' : 'source'/u)
  assert.match(documentWorkspaceSource, /!markdown && !html && !structured/u)
  assert.match(documentWorkspaceSource, /artifact-content artifact-structured-preview/u)
  assert.match(documentWorkspaceSource, /artifact-source artifact-structured-source/u)
  assert.match(documentWorkspaceSource, /renderStructuredTextPreview\(file\.path, renderedContent\)/u)
  assert.match(documentReviewStyles, /\.artifact-source \{[^}]*var\(--code-font/u)
  assert.match(reviewNotesSource, /structuredPreviewSourceRange/u)
  assert.match(reviewNotesSource, /structuredPreview\.sourceMap/u)
})

test('keeps an unresolved rendered selection unresolved after normalization', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: '**formatted text**' }
  const target = createFileRangeTarget(file, 'formatted text plus another paragraph')
  assert.equal(target.startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target }).startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target }).endOffset, null)
})

test('finds every non-overlapping match in one text node', () => {
  assert.deepEqual(findTextMatchRanges('Test and test and TEST', 'test'), [
    { start: 0, end: 4 },
    { start: 9, end: 13 },
    { start: 18, end: 22 },
  ])
})

test('createFileRangeTarget rejects zero-length quotes', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: 'one\ntwo\nthree' }
  assert.deepEqual(createFileRangeTarget(file, '', 0), {
    kind: 'fileRange',
    filePath: '/work/docs/guide.md',
    root: '/work',
    baseHash: 'abc',
    startOffset: null,
    endOffset: null,
    startLine: null,
    endLine: null,
    prefix: '',
    suffix: '',
    ambiguous: false,
  })
})

test('extracts a portable file display name', () => {
  assert.equal(fileDisplayName('C:\\work\\README.md'), 'README.md')
  assert.equal(isHtmlFile('/work/index.html'), true)
  assert.equal(isHtmlFile('/work/page.htm'), true)
  assert.equal(isHtmlFile('/work/page.md'), false)
})

test('snapshots a document annotation independently from transient selection state', () => {
  const selection = {
    quote: 'selected paragraph',
    target: {
      kind: 'fileRange', filePath: '/work/README.md', root: '/work', baseHash: 'abc',
      startOffset: 12, endOffset: 30, prefix: 'before', suffix: 'after',
    },
  }
  const snapshot = snapshotAnnotationSelection(selection)
  selection.quote = ''
  selection.target.filePath = '/changed.md'

  assert.equal(snapshot.quote, 'selected paragraph')
  assert.equal(snapshot.target.filePath, '/work/README.md')
  assert.equal(snapshot.target.startOffset, 12)
})
