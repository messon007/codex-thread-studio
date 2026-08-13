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

test('workspace tool launchers remain visible in narrow windows', () => {
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.doesNotMatch(styles, /\.workspace-tool-launchers\s*\{[^}]*display:\s*none/u)
  assert.match(styles, /\.workspace-tool-launchers\s*\{[^}]*flex:\s*0 0 auto/u)
})

test('session actions use the same compact icon-button geometry as workspace tools', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  for (const id of ['rename-thread', 'fork-thread', 'archive-thread', 'delete-thread']) {
    assert.match(html, new RegExp(`id="${id}"[^>]*thread-action-icon[^>]*icon-only[^>]*>[\\s\\S]{0,240}<svg`, 'u'))
  }
  assert.match(styles, /\.workspace-tool-launcher, \.thread-action-icon \{[^}]*place-items: center/u)
})

test('workspace tools use one right-side slot without nested tool tabs', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.doesNotMatch(html, /id="workspace-(?:files|terminal)-tab"/u)
  assert.match(app, /function activateRightWorkspace\(tool\)/u)
  assert.match(app, /if \(tool !== 'browser'\) \{[\s\S]{0,220}studio-action:\/\/hide-browser/u)
  assert.match(app, /\|\| workspaceTools\.isOpen\(\)/u)
})

test('document actions use consistent SVG icon buttons', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  for (const id of ['artifact-save', 'refresh-artifact', 'close-artifact']) {
    assert.match(html, new RegExp(`id="${id}"[\\s\\S]{0,420}<svg`, 'u'))
  }
})

test('documents opened from Files and Review keep a return destination', () => {
  const workspace = readFileSync(new URL('./workspace-tools.mjs', import.meta.url), 'utf8')
  const review = readFileSync(new URL('./git-review.mjs', import.meta.url), 'utf8')
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(workspace, /openFile\?\.\(\{ root: state\.root, path \}, \{ returnTool: 'files' \}\)/u)
  assert.match(review, /returnTool: 'review'/u)
  assert.match(app, /workspaceTools\.open\(returnTool\)/u)
})

test('workspace rails share the persisted document width and shield pointer resizing', () => {
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.match(app, /state\.rightRailWidthRatio = normalizeRightRailWidthRatio/u)
  assert.match(app, /event\.preventDefault\(\)[\s\S]{0,180}removeAllRanges/u)
  for (const id of ['session-map', 'artifact', 'workspace-tools', 'annotation', 'favorites']) {
    assert.match(html, new RegExp(`id="${id}-resizer"[\\s\\S]{0,180}app-right-rail-resizer`, 'u'))
  }
  for (const rail of ['annotation', 'favorites', 'session-map', 'artifact', 'workspace-tools']) {
    assert.match(styles, new RegExp(`\\.${rail}-rail \\{[^}]*var\\(--right-rail-width\\)`, 'u'))
  }
  assert.match(styles, /body\.resizing-right-rail::after/u)
})
