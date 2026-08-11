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
  assert.equal(autoFavoriteTitle('## 设计结论\n\n正文'), '设计结论')
  assert.equal(autoFavoriteTitle(''), '收藏的 AI 回复')
})

test('finds the structured user question from the same turn', () => {
  const turn = {
    items: [
      { type: 'userMessage', content: [{ type: 'text', text: '如何设计？' }] },
      { type: 'agentMessage', text: '回答' },
    ],
  }
  assert.equal(questionForTurn(turn), '如何设计？')
})

test('normalizes tags and builds stable source identities', () => {
  assert.deepEqual(normalizeFavoriteTags('架构, Rust，架构'), ['架构', 'Rust'])
  assert.equal(
    favoriteSourceKey({ backend: 'codex', threadId: 't', turnId: 'u', itemId: 'i' }),
    'codex:t:u:i',
  )
})

test('copy output includes only fields stored in the favorite', () => {
  assert.equal(
    favoriteCopyText({ question: 'Q', content: 'A', note: '' }),
    '问题：\nQ\n\n回答：\nA',
  )
})

test('global favorites expose Markdown export and hide it in session scope', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(html, /id="export-favorites"[^>]*>导出<\/button>/)
  assert.match(source, /gatewayFetch\('\/studio\/favorites\/export'/)
  assert.match(source, /codex-thread-studio-favorites\.md/)
  assert.match(source, /export-favorites'\)\.classList\.toggle\('hidden', state\.favoriteScope !== 'global'\)/)
})
