import test from 'node:test'
import assert from 'node:assert/strict'

import {
  composerTrigger,
  matchingSlashCommands,
  replaceComposerTrigger,
  selectedFileReference,
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

test('classifies high-frequency transcript updates', () => {
  assert.equal(transcriptUpdateKind('item/agentMessage/delta'), 'stream')
  assert.equal(transcriptUpdateKind('item/reasoning/summaryTextDelta'), 'stream')
  assert.equal(transcriptUpdateKind('item/commandExecution/outputDelta'), 'stream')
  assert.equal(transcriptUpdateKind('item/completed'), 'item')
  assert.equal(transcriptUpdateKind('thread/tokenUsage/updated'), 'metadata')
  assert.equal(transcriptUpdateKind('turn/started'), 'full')
})
