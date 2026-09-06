import assert from 'node:assert/strict'
import test from 'node:test'

import { createPendingGitReads, parseUnifiedDiff, reviewFileStatus, visibleReviewFiles } from './git-review.mjs'

test('Git reads coalesce only while pending, never cache results or failures', async () => {
  const read = createPendingGitReads()
  let resolve
  let calls = 0
  const fetch = () => { calls++; return new Promise(done => { resolve = done }) }
  const first = read('root:file:unstaged:1', fetch)
  const second = read('root:file:unstaged:1', fetch)
  assert.equal(first, second)
  await Promise.resolve()
  assert.equal(calls, 1)
  resolve('diff')
  assert.equal(await second, 'diff')
  assert.equal(await read('root:file:unstaged:1', () => 'new diff'), 'new diff')
  await assert.rejects(read('error', () => { throw new Error('offline') }), /offline/u)
  assert.equal(await read('error', () => 'retry'), 'retry')
  assert.deepEqual(await Promise.all([
    read('revision:1', () => 'before'), read('revision:2', () => 'after'),
  ]), ['before', 'after'])
})

test('unified diff parser keeps old and new line numbers aligned', () => {
  const rows = parseUnifiedDiff('diff --git a/a.md b/a.md\n@@ -2,3 +2,3 @@\n same\n-old\n+new\n tail')
  assert.deepEqual(rows.slice(2, 6).map((row) => [row.kind, row.oldLine, row.newLine, row.text]), [
    ['context', 2, 2, 'same'],
    ['deletion', 3, null, 'old'],
    ['addition', null, 3, 'new'],
    ['context', 4, 4, 'tail'],
  ])
})

test('review filters support staged, unstaged and path search', () => {
  const files = [
    { path: 'README.md', staged: false, unstaged: true },
    { path: 'data/report.csv', staged: true, unstaged: false },
    { path: 'src/app.js', staged: true, unstaged: true },
  ]
  assert.deepEqual(visibleReviewFiles(files, 'staged').map((file) => file.path), ['data/report.csv', 'src/app.js'])
  assert.deepEqual(visibleReviewFiles(files, 'unstaged', 'read').map((file) => file.path), ['README.md'])
})

test('review status gives conflicts and untracked files priority', () => {
  assert.equal(reviewFileStatus({ conflicted: true }).label, '!')
  assert.equal(reviewFileStatus({ untracked: true }).label, 'U')
  assert.equal(reviewFileStatus({ indexStatus: 'A', worktreeStatus: ' ' }).title, 'Added')
})
