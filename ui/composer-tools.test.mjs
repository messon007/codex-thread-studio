import test from 'node:test'
import assert from 'node:assert/strict'

import {
  composerTrigger,
  isPreviewableImageFile,
  isPreviewableTextFile,
  matchingSkills,
  matchingSlashCommands,
  replaceComposerTrigger,
  previewableFileKind,
  selectedFileReference,
  selectedSkillReference,
  shellCommandFromComposer,
  transcriptUpdateKind,
} from './composer-tools.mjs'

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
  for (const path of ['README', 'LICENSE.md', 'docs/guide.markdown', 'src/main.rs', 'config.yaml', '.gitignore', 'hooks/commit-msg.sample', 'config/app.conf.example', 'build/CMakeLists.txt']) {
    assert.equal(isPreviewableTextFile({ path }), true, path)
  }
  for (const path of ['image.png', 'diagram.svg', 'archive.zip', 'program.exe', 'data.bin', 'unknown']) {
    assert.equal(isPreviewableTextFile({ path }), false, path)
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
