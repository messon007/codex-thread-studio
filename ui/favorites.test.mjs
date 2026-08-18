import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  autoFavoriteTitle,
  favoriteCopyText,
  favoriteSourceKey,
  normalizeFavoriteTags,
  questionForTurn,
} from './favorites.mjs'

test('derives a clean title from markdown without losing Chinese text', () => {
  assert.equal(autoFavoriteTitle('## \u8BBE\u8BA1\u7ED3\u8BBA\n\n\u6B63\u6587'), '\u8BBE\u8BA1\u7ED3\u8BBA')
  assert.equal(autoFavoriteTitle(''), 'Favorite AI response')
})

test('finds the structured user question from the same turn', () => {
  const turn = {
    items: [
      { type: 'userMessage', content: [{ type: 'text', text: '\u5982\u4F55\u8BBE\u8BA1\uFF1F' }] },
      { type: 'agentMessage', text: '\u56DE\u7B54' },
    ],
  }
  assert.equal(questionForTurn(turn), '\u5982\u4F55\u8BBE\u8BA1\uFF1F')
})

test('normalizes tags and builds stable source identities', () => {
  assert.deepEqual(normalizeFavoriteTags('\u67B6\u6784, Rust\uFF0C\u67B6\u6784'), ['\u67B6\u6784', 'Rust'])
  assert.equal(
    favoriteSourceKey({ backend: 'codex', threadId: 't', turnId: 'u', itemId: 'i' }),
    'codex:t:u:i',
  )
})

test('copy output includes only fields stored in the favorite', () => {
  assert.equal(
    favoriteCopyText({ question: 'Q', content: 'A', note: '' }),
    'Question:\nQ\n\nAnswer:\nA',
  )
})

test('global favorites expose Markdown export and hide it in session scope', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(html, /id="export-favorites"[^>]*title="Export favorites"[^>]*>[\s\S]*?<svg[\s\S]*?<\/button>/)
  assert.doesNotMatch(html, /id="export-favorites"[^>]*>Export<\/button>/)
  assert.match(source, /gatewayFetch\('\/studio\/favorites\/export'/)
  assert.match(source, /codex-thread-studio-favorites\.md/)
  assert.match(source, /export-favorites'\)\.classList\.toggle\('hidden', state\.favoriteScope !== 'global'\)/)
})
