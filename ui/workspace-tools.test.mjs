import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  safeWorkspaceRelativePath,
  workspaceRootName,
  workspaceStateKey,
} from './workspace-tools.mjs'

test('workspace tools keep file paths relative to the session root', () => {
  assert.equal(safeWorkspaceRelativePath('./docs/README.md'), 'docs/README.md')
  assert.equal(safeWorkspaceRelativePath('src\\main.rs'), 'src/main.rs')
  assert.equal(safeWorkspaceRelativePath('../outside.txt'), null)
  assert.equal(safeWorkspaceRelativePath('/etc/passwd'), null)
  assert.equal(safeWorkspaceRelativePath('C:\\Windows\\system.ini'), null)
})

test('workspace state keeps terminal lifecycle isolated per session and backend', () => {
  const thread = { id: 'thread-a', cwd: '/home/rui/project' }
  assert.equal(workspaceStateKey(thread, 'codex'), 'codex:thread-a:/home/rui/project')
  assert.notEqual(workspaceStateKey({ ...thread, id: 'thread-b' }, 'codex'), workspaceStateKey(thread, 'codex'))
  assert.notEqual(workspaceStateKey(thread, 'codex'), workspaceStateKey(thread, 'opencode'))
})

test('workspace root name supports Unix and Windows paths', () => {
  assert.equal(workspaceRootName('/home/rui/project/'), 'project')
  assert.equal(workspaceRootName('C:\\Users\\rui\\project'), 'project')
})

test('workspace tools use one right-side slot without nested tool tabs', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.doesNotMatch(html, /id="workspace-(?:files|terminal)-tab"/u)
  assert.match(app, /function activateRightWorkspace\(tool\)/u)
  assert.match(app, /\|\| workspaceTools\.isOpen\(\)/u)
})

test('document actions use consistent SVG icon buttons', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  for (const id of ['artifact-save', 'refresh-artifact', 'close-artifact']) {
    assert.match(html, new RegExp(`id="${id}"[\\s\\S]{0,420}<svg`, 'u'))
  }
})
