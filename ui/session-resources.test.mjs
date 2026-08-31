import assert from 'node:assert/strict'
import test from 'node:test'

import {
  buildSessionResourceIndex,
  extractPathCandidates,
  extractUrlCandidates,
  normalizeResourceCandidate,
  sessionResourceRevision,
} from './session-resources.mjs'

test('URL extraction trims prose punctuation but keeps balanced URL parentheses', () => {
  const resources = extractUrlCandidates('See https://example.com/a_(b)?q=1, then https://openai.com/docs.')
  assert.deepEqual(resources.map((resource) => resource.raw), [
    'https://example.com/a_(b)?q=1',
    'https://openai.com/docs',
  ])
})

test('URL normalization deduplicates fragments conservatively and blocks credentials', () => {
  const first = normalizeResourceCandidate({ raw: 'HTTPS://Example.COM:443/a?q=1#one', kind: 'web' })
  const second = normalizeResourceCandidate({ raw: 'https://example.com/a?q=1#two', kind: 'web' })
  assert.equal(first.canonical, 'https://example.com/a?q=1')
  assert.equal(first.id, second.id)
  assert.equal(normalizeResourceCandidate({ raw: 'https://user:secret@example.com/a', kind: 'web' }).state, 'blocked')
})

test('file extraction understands Linux, Windows and code locations', () => {
  const candidates = extractPathCandidates('Open src/main.rs:42:7, docs/guide.md and C:\\work\\app\\main.ts:8.')
  assert.deepEqual(candidates.map((candidate) => candidate.raw), [
    'src/main.rs:42:7',
    'docs/guide.md',
    'C:\\work\\app\\main.ts:8',
  ])
  const windows = normalizeResourceCandidate({ raw: 'C:\\work\\app\\main.ts:8', kind: 'code' }, {
    backend: 'codex', root: 'C:\\work\\app',
  })
  assert.equal(windows.target.path, 'main.ts')
  assert.equal(windows.target.line, 8)
  assert.equal(windows.state, 'unresolved')
})

test('paths outside the session root remain visible but blocked', () => {
  const resource = normalizeResourceCandidate({ raw: '/tmp/result.json', kind: 'file' }, {
    backend: 'codex', root: '/home/rui/project',
  })
  assert.equal(resource.state, 'blocked')
  assert.match(resource.reason, /session root/u)
})

test('configured shared document paths remain openable across sessions', () => {
  const context = {
    backend: 'codex',
    root: '/home/rui/project',
    sharedDocumentDirectories: ['/tmp/shared-library'],
  }
  const shared = normalizeResourceCandidate(
    { raw: '/tmp/shared-library/reports/result.json', kind: 'file' },
    { backend: 'codex', root: '/home/rui/project' },
    context,
  )
  assert.equal(shared.state, 'unresolved')
  assert.equal(shared.target.path, '/tmp/shared-library/reports/result.json')
  assert.equal(normalizeResourceCandidate(
    { raw: '/tmp/shared-library-copy/result.json', kind: 'file' },
    { backend: 'codex', root: '/home/rui/project' },
    context,
  ).state, 'blocked')
})

test('session index merges resources without losing occurrences', () => {
  const model = {
    turns: [{
      id: 't1',
      items: [
        { id: 'u1', type: 'userMessage', content: 'Read https://example.com/guide#overview' },
        { id: 'a1', type: 'agentMessage', text: 'Done: [the guide](https://example.com/guide).' },
      ],
    }],
  }
  const index = buildSessionResourceIndex(model, { backend: 'codex', threadId: 'thread-1', root: '/repo' })
  assert.equal(index.counts().all, 1)
  assert.equal(index.counts().occurrences, 2)
  assert.equal(index.occurrences(index.resources()[0].id).length, 2)
})

test('only the latest turn contributes resources', () => {
  const model = {
    turns: [
      { id: 'old', items: [{ id: 'old-answer', type: 'agentMessage', text: 'Old: docs/history.md' }] },
      { id: 'latest', items: [{ id: 'answer', type: 'agentMessage', text: 'Current: docs/current.md' }] },
    ],
  }
  const index = buildSessionResourceIndex(model, { backend: 'codex', threadId: 'thread', root: '/repo' })
  assert.deepEqual(index.resources().map((resource) => resource.target.path), ['docs/current.md'])
})

test('resource revision changes only when the latest narrative content changes', () => {
  const model = {
    turns: [
      { id: 'old', items: [{ id: 'old-answer', type: 'agentMessage', text: 'docs/old.md' }] },
      { id: 'latest', items: [{ id: 'answer', type: 'agentMessage', text: 'docs/current.md' }] },
    ],
  }
  const initial = sessionResourceRevision(model)
  assert.equal(sessionResourceRevision(model), initial)
  model.turns[0].items[0].text = 'docs/changed.md'
  assert.equal(sessionResourceRevision(model), initial)
  model.turns[1].items[0].text += '?updated'
  assert.notEqual(sessionResourceRevision(model), initial)
})

test('resource revision notices a replaced latest item with equal-length text', () => {
  const model = { turns: [{ id: 'latest', items: [{ id: 'answer', type: 'agentMessage', text: 'docs/one.md' }] }] }
  const initial = sessionResourceRevision(model)
  model.turns[0].items[0] = { id: 'answer', type: 'agentMessage', text: 'docs/two.md' }
  assert.notEqual(sessionResourceRevision(model), initial)
})

test('resource revision notices an in-place equal-length change outside sampled positions', () => {
  const text = `docs/${'a'.repeat(90)}.md`
  const item = { id: 'answer', type: 'agentMessage', text }
  const model = { turns: [{ id: 'latest', items: [item] }] }
  const initial = sessionResourceRevision(model)
  item.text = `${text.slice(0, 1)}X${text.slice(2)}`
  assert.equal(item.text.length, text.length)
  assert.notEqual(sessionResourceRevision(model), initial)
})

test('file changes are activity rather than resources', () => {
  const model = {
    turns: [{
      id: 'turn',
      items: [
        { id: 'change', type: 'fileChange', changes: [{ kind: 'update', path: 'docs/spec.md' }] },
        { id: 'answer', type: 'agentMessage', text: 'Implementation completed.' },
      ],
    }],
  }
  const index = buildSessionResourceIndex(model, { backend: 'opencode', threadId: 'thread', root: '/repo' })
  assert.equal(index.counts().all, 0)
})

test('fenced code and Mermaid diagrams do not create resources', () => {
  const model = {
    turns: [{
      id: 'turn',
      items: [{
        id: 'answer',
        type: 'agentMessage',
        text: [
          'The real document is `docs/design.md`.',
          '```mermaid',
          'flowchart LR',
          '  A[Chat/UI] --> B[Browser/Panel]',
          '```',
          '```html',
          '<script src="assets/example.js"></script>',
          '<a href="https://example.invalid/demo">demo</a>',
          '```',
        ].join('\n'),
      }],
    }],
  }
  const index = buildSessionResourceIndex(model, { backend: 'codex', threadId: 'thread', root: '/repo' })
  assert.deepEqual(index.resources().map((resource) => resource.target.path || resource.target.url), ['docs/design.md'])
})

test('path extraction rejects markup, route-like fragments and bare Mermaid labels', () => {
  const candidates = extractPathCandidates('</div> API/v1 Chat/UI src/session-resources.mjs')
  assert.deepEqual(candidates.map((candidate) => candidate.raw), ['src/session-resources.mjs'])
})

test('command and tool payloads are not treated as user-facing resources', () => {
  const model = {
    turns: [{
      id: 'turn',
      items: [
        { id: 'command', type: 'commandExecution', aggregatedOutput: 'Compiling src/generated.rs\nhttps://logs.invalid/build' },
        { id: 'tool', type: 'mcpToolCall', result: { source: 'docs/tool-result.md' } },
        { id: 'answer', type: 'agentMessage', text: 'Result: [report](docs/report.md)' },
      ],
    }],
  }
  const index = buildSessionResourceIndex(model, { backend: 'codex', threadId: 'thread', root: '/repo' })
  assert.deepEqual(index.resources().map((resource) => resource.target.path || resource.target.url), ['docs/report.md'])
})
