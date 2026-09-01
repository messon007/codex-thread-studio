import test from 'node:test'
import assert from 'node:assert/strict'

import {
  CONTINUE_PROMPTS,
  composerTrigger,
  createComposerDraftStore,
  isPreviewableImageFile,
  isPreviewableTextFile,
  matchingSkills,
  matchingSlashCommands,
  randomContinuePrompt,
  replaceComposerTrigger,
  previewableFileKind,
  reviewableFileKind,
  selectedFileReference,
  selectedSkillReference,
  shellCommandFromComposer,
  transcriptUpdateKind,
} from './composer-tools.mjs'

test('selects quick Continue prompts from a small explicit phrase set', () => {
  assert.deepEqual(CONTINUE_PROMPTS, [
    'Continue.',
    'Go on.',
    'Keep going.',
    'Please continue.',
    'Continue with the task.',
  ])
  assert.equal(randomContinuePrompt(() => 0), 'Continue.')
  assert.equal(randomContinuePrompt(() => 0.41), 'Keep going.')
  assert.equal(randomContinuePrompt(() => 0.999), 'Continue with the task.')
})

test('keeps composer drafts isolated by backend and session', () => {
  const drafts = createComposerDraftStore()
  assert.equal(drafts.switchTo('codex:thread-a', ''), '')
  drafts.update('codex:thread-a', 'draft A')
  assert.equal(drafts.switchTo('opencode:thread-a', 'draft A'), '')
  drafts.update('opencode:thread-a', 'draft OpenCode')
  assert.equal(drafts.switchTo('codex:thread-a', 'draft OpenCode'), 'draft A')
})

test('discarding the visible composer draft does not restore its DOM value', () => {
  const drafts = createComposerDraftStore()
  drafts.switchTo('codex:deleted', '')
  drafts.update('codex:deleted', 'do not restore')
  drafts.discard('codex:deleted')
  assert.equal(drafts.switchTo('', 'do not restore'), '')
  assert.equal(drafts.value('codex:deleted'), '')
})

test('detects slash commands only at the beginning of the composer', () => {
  assert.deepEqual(composerTrigger('/mod'), { type: 'slash', query: 'mod', start: 0, end: 4 })
  assert.equal(composerTrigger('please use /model'), null)
})

test('detects file mentions at the active cursor token', () => {
  assert.deepEqual(composerTrigger('review @src/app'), { type: 'file', query: 'src/app', start: 7, end: 15 })
  assert.equal(composerTrigger('mail@example.com'), null)
})

test('detects skill mentions at the active cursor token', () => {
  assert.deepEqual(composerTrigger('use $github'), { type: 'skill', query: 'github', start: 4, end: 11 })
  assert.equal(composerTrigger('price is $5 today'), null)
})

test('detects a shell command only from the composer prefix', () => {
  assert.equal(shellCommandFromComposer('!git status --short'), 'git status --short')
  assert.equal(shellCommandFromComposer('  ! printf "a | b"\n'), 'printf "a | b"')
  assert.equal(shellCommandFromComposer('explain !important'), null)
  assert.equal(shellCommandFromComposer('!'), '')
})

test('replaces only the active composer trigger', () => {
  const source = 'review @src/ap please'
  const trigger = composerTrigger(source, 14)
  assert.deepEqual(replaceComposerTrigger(source, trigger, '@src/app.js '), {
    value: 'review @src/app.js  please',
    cursor: 19,
  })
})

test('filters slash commands and formats file references', () => {
  assert.equal(matchingSlashCommands('comp')[0].name, 'compact')
  assert.equal(selectedFileReference({ path: 'src/main.rs' }), 'src/main.rs ')
  assert.equal(selectedFileReference({ path: 'docs/design notes.md' }), '"docs/design notes.md" ')
})

test('enables document preview only for known text file types', () => {
  for (const path of ['README', 'LICENSE.md', 'docs/guide.markdown', 'src/main.rs', 'config.yaml', '.gitignore', 'hooks/commit-msg.sample', 'config/app.conf.example', 'build/CMakeLists.txt', 'benchmarks/cases/01-layered-pipeline.d2']) {
    assert.equal(isPreviewableTextFile({ path }), true, path)
  }
  for (const path of ['image.png', 'diagram.svg', 'archive.zip', 'program.exe', 'data.bin', 'unknown']) {
    assert.equal(isPreviewableTextFile({ path }), false, path)
  }
})

test('lets the UTF-8 review endpoint decide unknown text formats without opening known binaries', () => {
  assert.equal(reviewableFileKind({ path: 'design/system.customdsl' }), 'text')
  assert.equal(reviewableFileKind({ path: 'scripts/tool' }), 'text')
  for (const path of ['archive.zip', 'program.exe', 'document.docx', 'database.sqlite3']) {
    assert.equal(reviewableFileKind({ path }), null, path)
  }
})

test('enables static image previews without treating other binaries as documents', () => {
  for (const path of ['image.png', 'photo.JPG', 'diagram.webp', 'animation.gif', 'drawing.svg']) {
    assert.equal(isPreviewableImageFile({ path }), true, path)
    assert.equal(previewableFileKind({ path }), 'image', path)
  }
  assert.equal(previewableFileKind({ path: 'docs/guide.md' }), 'text')
  assert.equal(previewableFileKind({ path: 'books/guide.epub' }), 'epub')
  assert.equal(previewableFileKind({ path: 'papers/report.pdf' }), 'pdf')
  assert.equal(previewableFileKind({ path: 'data/report.xlsx' }), 'table')
  assert.equal(previewableFileKind({ path: 'data/report.csv' }), 'table')
  assert.equal(previewableFileKind({ path: 'archive.zip' }), null)
})

test('filters and formats app-server skills', () => {
  const skills = [
    { name: 'github', description: 'Work with repositories', enabled: true },
    { name: 'imagegen', description: 'Create images', enabled: true },
    { name: 'disabled', description: 'Hidden', enabled: false },
  ]
  assert.deepEqual(matchingSkills('repo', skills).map((skill) => skill.name), ['github'])
  assert.deepEqual(matchingSkills('', skills).map((skill) => skill.name), ['github', 'imagegen'])
  assert.equal(selectedSkillReference(skills[0]), '$github ')
})

test('classifies high-frequency transcript updates', () => {
  assert.equal(transcriptUpdateKind('item/agentMessage/delta'), 'stream')
  assert.equal(transcriptUpdateKind('item/reasoning/summaryTextDelta'), 'stream')
  assert.equal(transcriptUpdateKind('item/commandExecution/outputDelta'), 'stream')
  assert.equal(transcriptUpdateKind('item/completed'), 'item')
  assert.equal(transcriptUpdateKind('thread/tokenUsage/updated'), 'metadata')
  assert.equal(transcriptUpdateKind('turn/started'), 'full')
})
