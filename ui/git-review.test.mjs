import assert from 'node:assert/strict'
import test from 'node:test'

import { parseUnifiedDiff, reviewFileStatus, visibleReviewFiles } from './git-review.mjs'

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
  assert.equal(reviewFileStatus({ indexStatus: 'A', worktreeStatus: ' ' }).title, '新增')
})
