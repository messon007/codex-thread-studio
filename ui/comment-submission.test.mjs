import assert from 'node:assert/strict'
import test from 'node:test'
import { CommentSubmissionCoordinator, insertCommentPrompt } from './comment-submission.mjs'

const draft = (id = 'one') => ({ id, excerpt: 'quote', note: 'question', createdAt: 'now', source: { provider: 'chat', version: 1, anchor: {} } })
function fixture() {
  const store = { annotationDrafts: { 'codex:a': [draft()], 'codex:b': [draft('other')] }, annotationAdditional: { 'codex:a': 'note' } }
  let text = 'existing'
  const persisted = []
  return { store, persisted, text: () => text, options: {
    key: 'codex:a', store, prompt: 'comments', composerText: text,
    insert: value => { text = value }, persist: key => persisted.push(key),
  } }
}

test('Insert only composes text; failed send retains drafts and retry does not duplicate text', async () => {
  assert.equal(insertCommentPrompt(' existing ', 'comments'), 'existing\n\ncomments')
  const f = fixture(), flow = new CommentSubmissionCoordinator()
  assert.equal(await flow.submit({ ...f.options, send: async () => false }), 'retained')
  assert.equal(f.store.annotationDrafts['codex:a'].length, 1)
  assert.deepEqual(f.persisted, [])
  assert.equal(await flow.submit({ ...f.options, composerText: f.text(), send: async () => true }), 'sent')
  assert.equal(f.text(), 'existing\n\ncomments')
  assert.equal(f.store.annotationDrafts['codex:a'], undefined)
  assert.equal(f.store.annotationDrafts['codex:b'].length, 1)
  assert.deepEqual(f.persisted, ['codex:a'])
})

test('double click is rejected while awaiting acknowledgement and new or edited drafts survive', async () => {
  const f = fixture(), flow = new CommentSubmissionCoordinator()
  let resolve
  const gate = new Promise(done => { resolve = done })
  const first = flow.submit({ ...f.options, send: () => gate })
  assert.equal(flow.isPending('codex:a'), true)
  assert.equal(await flow.submit({ ...f.options, send: () => { throw Error('duplicate send') } }), 'busy')
  f.store.annotationDrafts['codex:a'][0].note = 'edited while sending'
  f.store.annotationDrafts['codex:a'].push(draft('new'))
  f.store.annotationAdditional['codex:a'] = 'new additional note'
  resolve(true)
  assert.equal(await first, 'sent')
  assert.deepEqual(f.store.annotationDrafts['codex:a'].map(d => d.id), ['one', 'new'])
  assert.equal(f.store.annotationAdditional['codex:a'], 'new additional note')
  assert.equal(flow.isPending('codex:a'), false)
})

test('rejected sends unlock only their session and never clear drafts', async () => {
  const f = fixture(), flow = new CommentSubmissionCoordinator()
  await assert.rejects(flow.submit({ ...f.options, send: async () => { throw Error('offline') } }), /offline/)
  assert.equal(flow.isPending('codex:a'), false)
  assert.equal(f.store.annotationDrafts['codex:a'].length, 1)
})
