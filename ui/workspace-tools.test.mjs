import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  createWorkspaceTools,
  safeWorkspaceRelativePath,
  workspaceRootName,
  workspaceStateKey,
} from './workspace-tools.mjs'

function mockElement(initialClasses = []) {
  const classes = new Set(initialClasses)
  let html = ''
  return {
    classList: {
      add: (...names) => names.forEach((name) => classes.add(name)),
      remove: (...names) => names.forEach((name) => classes.delete(name)),
      contains: (name) => classes.has(name),
      toggle: (name, force) => force ? classes.add(name) : classes.delete(name),
    },
    setAttribute() {},
    textContent: '',
    value: '',
    innerHTMLWrites: 0,
    get innerHTML() { return html },
    set innerHTML(value) {
      html = value
      this.innerHTMLWrites += 1
    },
  }
}

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

test('session status synchronization does not replace an already loaded file tree', async () => {
  const previousDocument = globalThis.document
  const elements = Object.fromEntries([
    'workspace-tools-rail',
    'workspace-files-pane',
    'workspace-terminal-pane',
    'workspace-review-pane',
    'open-workspace-files',
    'open-workspace-terminal',
    'open-workspace-review',
    'workspace-tools-path',
    'workspace-root-name',
    'workspace-terminal-cwd',
    'workspace-file-filter',
    'workspace-tools-title',
    'workspace-file-tree',
  ].map((id) => [id, mockElement(id === 'workspace-tools-rail' ? ['hidden'] : [])]))
  globalThis.document = { getElementById: (id) => elements[id] || null }

  try {
    const thread = { id: 'thread-a', cwd: '/home/rui/project' }
    const tools = createWorkspaceTools({
      getThread: () => thread,
      getBackend: () => 'codex',
      gatewayFetch: async () => ({
        ok: true,
        json: async () => ({ entries: [{ kind: 'file', name: 'README.md', path: 'README.md', size: 10 }] }),
      }),
    })

    await tools.open('files')
    const writesAfterLoad = elements['workspace-file-tree'].innerHTMLWrites
    tools.sync(thread)
    await Promise.resolve()

    assert.equal(elements['workspace-file-tree'].innerHTMLWrites, writesAfterLoad)
  } finally {
    globalThis.document = previousDocument
  }
})

test('workspace tool launchers remain visible in narrow windows', () => {
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.doesNotMatch(styles, /\.workspace-tool-launchers\s*\{[^}]*display:\s*none/u)
  assert.match(styles, /\.workspace-tool-launchers\s*\{[^}]*flex:\s*0 0 auto/u)
})

test('session management actions live in More while right areas stay compact', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const menu = html.match(/id="thread-more-menu"[\s\S]*?<\/div>\s*<\/div>\s*<\/div>\s*<\/header>/u)?.[0] || ''
  for (const id of ['rename-thread', 'fork-thread', 'archive-thread', 'delete-thread']) {
    assert.match(menu, new RegExp(`id="${id}"[\\s\\S]{0,240}<svg`, 'u'))
  }
  for (const id of ['open-thread-comments', 'open-thread-favorites', 'open-thread-resources', 'open-workspace-files', 'open-workspace-terminal', 'open-workspace-review']) {
    assert.match(html, new RegExp(`id="${id}"[^>]*workspace-tool-launcher[^>]*icon-only`, 'u'))
  }
  assert.doesNotMatch(html, /thread-action-divider/u)
  assert.doesNotMatch(html.slice(0, html.indexOf('id="thread-more-menu"')), /id="(?:rename|fork|archive|delete)-thread"/u)
})

test('CodeMirror stays constrained to the document rail when long lines are present', () => {
  const editor = readFileSync(new URL('./workspace-editor.mjs', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

  assert.match(styles, /\.artifact-editor-shell \{[^}]*min-width: 0;[^}]*overflow: hidden;/u)
  assert.match(editor, /'&': \{[\s\S]{0,180}width: '100%'[\s\S]{0,120}minWidth: '0'[\s\S]{0,120}maxWidth: '100%'[\s\S]{0,120}flex: '1 1 auto'/u)
  assert.match(editor, /'\.cm-scroller': \{ width: '100%', minWidth: '0', overflow: 'auto'/u)
  assert.match(editor, /openSearchPanel/u)
  assert.match(editor, /openSearch: \(\) => openSearchPanel\(view\)/u)
})

test('session and every right-area header share one exact divider height', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.match(styles, /--app-header-height:\s*75px/u)
  for (const selector of ['\\.thread-toolbar', '\\.annotation-rail > header', '\\.artifact-header', '\\.workspace-tools-header', '\\.favorites-rail > header', '\\.session-map-header', '\\.resources-header']) {
    assert.match(styles, new RegExp(`${selector} \\{[^}]*height: var\\(--app-header-height\\);[^}]*min-height: var\\(--app-header-height\\);[^}]*border-bottom: 1px solid var\\(--border\\)`, 'u'))
  }
  assert.doesNotMatch(html, /class="native-chrome"/u)
  assert.match(styles, /\.native-workspace \{[^}]*overflow: hidden;[^}]*background: var\(--panel\);/u)
  assert.doesNotMatch(styles.match(/\.native-workspace \{[^}]*\}/u)?.[0] || '', /margin:|border:|border-radius:|box-shadow:/u)
})

test('workspace tools use one right-side slot without nested tool tabs', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const sessionMap = readFileSync(new URL('./session-map-controller.mjs', import.meta.url), 'utf8')
  assert.doesNotMatch(html, /id="workspace-(?:files|terminal)-tab"/u)
  assert.match(app, /function activateRightWorkspace\(tool\)/u)
  assert.match(app, /if \(tool !== 'browser'\) \{[\s\S]{0,220}studio-action:\/\/hide-browser/u)
  assert.match(sessionMap, /\|\| workspaceTools\.isOpen\(\)/u)
})

test('document actions use consistent SVG icon buttons', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  for (const id of ['artifact-save', 'refresh-artifact', 'close-artifact', 'refresh-resources', 'close-resources']) {
    assert.match(html, new RegExp(`id="${id}"[\\s\\S]{0,420}<svg`, 'u'))
  }
  assert.match(html, /id="refresh-resources"[^>]*class="icon-button artifact-action-icon"/u)
  assert.match(html, /id="close-resources"[^>]*class="icon-button artifact-action-icon"/u)
})

test('session resources provide an explicit favorite action', () => {
  const resources = readFileSync(new URL('./session-resources-ui.mjs', import.meta.url), 'utf8')
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const reviewNotes = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')
  assert.match(resources, /data-resource-favorite=/u)
  assert.match(resources, /favoriteResource\?\.\(resource, occurrence\)/u)
  assert.match(app, /favoriteResource: \(resource, occurrence\) => reviewNotes\.openFavoriteForResource/u)
  assert.match(reviewNotes, /favorite\.presentation === 'resource'/u)
})

test('documents opened from Files and Review keep a return destination', () => {
  const workspace = readFileSync(new URL('./workspace-tools.mjs', import.meta.url), 'utf8')
  const review = readFileSync(new URL('./git-review.mjs', import.meta.url), 'utf8')
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const documentWorkspace = readFileSync(new URL('./document-workspace-controller.mjs', import.meta.url), 'utf8')
  assert.match(workspace, /openFile\?\.\(\{ root: state\.root, path \}, \{ returnTool: 'files' \}\)/u)
  assert.match(review, /returnTool: 'review'/u)
  assert.match(app, /openWorkspaceTool: \(tool\) => workspaceTools\.open\(tool\)/u)
  assert.match(documentWorkspace, /openWorkspaceTool\(returnTool\)/u)
})

test('workspace rails share the persisted document width and shield pointer resizing', () => {
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.match(app, /state\.rightRailWidthRatio = normalizeRightRailWidthRatio/u)
  assert.match(app, /event\.preventDefault\(\)[\s\S]{0,180}removeAllRanges/u)
  for (const id of ['session-map', 'artifact', 'workspace-tools', 'annotation', 'favorites', 'resources']) {
    assert.match(html, new RegExp(`id="${id}-resizer"[\\s\\S]{0,180}app-right-rail-resizer`, 'u'))
  }
  for (const rail of ['annotation', 'favorites', 'session-map', 'artifact', 'workspace-tools', 'resources']) {
    assert.match(styles, new RegExp(`\\.${rail}-rail \\{[^}]*var\\(--right-rail-width\\)`, 'u'))
  }
  assert.match(styles, /body\.resizing-right-rail::after/u)
})
