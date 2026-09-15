import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createPendingGitReads, parseUnifiedDiff, reviewFileStatus, visibleReviewCommits, visibleReviewFiles } from './git-review.mjs'

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
  assert.equal(reviewFileStatus({ status: 'R' }).title, 'Rename')
})

test('commit history filters by hash, subject and author', () => {
  const commits = [
    { hash: 'a'.repeat(40), shortHash: 'aaaaaaa', subject: 'Add history', authorName: 'Ada' },
    { hash: 'b'.repeat(40), shortHash: 'bbbbbbb', subject: 'Fix review', authorEmail: 'lin@example.com' },
  ]
  assert.deepEqual(visibleReviewCommits(commits, 'history').map((commit) => commit.shortHash), ['aaaaaaa'])
  assert.deepEqual(visibleReviewCommits(commits, 'LIN@').map((commit) => commit.shortHash), ['bbbbbbb'])
  assert.deepEqual(visibleReviewCommits(commits, 'bbbb').map((commit) => commit.shortHash), ['bbbbbbb'])
})

test('Git Review wires history tabs to read-only commit endpoints', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const review = readFileSync(new URL('./git-review.mjs', import.meta.url), 'utf8')
  assert.match(html, /data-review-mode="history"/u)
  assert.match(html, /id="workspace-review-commits"/u)
  assert.match(html, /id="workspace-review-commit-files"/u)
  assert.match(review, /'\/studio\/git\/history'/u)
  assert.match(review, /'\/studio\/git\/commit'/u)
  assert.match(review, /'\/studio\/git\/commit-diff'/u)
})
