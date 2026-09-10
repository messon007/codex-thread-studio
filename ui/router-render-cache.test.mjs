import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

test('completed Router HTML cache invalidates for content, favorite, locale and expansion changes', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function renderRouterTargetTurn(')
  const end = source.indexOf('\nfunction conversationTrackIcon', start)
  let count = 0, favorite = false, locale = 'en-US', open = []
  const cache = new Map()
  const render = new Function('document', 'CSS', 'reviewNotes', 'getLocale', 'readMarkdownRenderCache', 'writeMarkdownRenderCache', 'MAX_MARKDOWN_RENDER_CACHE_BYTES', 'presentRoutedTurn', 'renderPresentationBlock', `${source.slice(start, end)}; return renderRouterTargetTurn`)(
    { querySelectorAll: () => open.map(id => ({ dataset: { activityId: id } })) }, { escape: String },
    { favoriteForSource: () => favorite }, () => locale, key => cache.get(key), (key, value) => cache.set(key, value), 1024 * 1024,
    () => { count++; return { blocks: [{}] } }, () => '<p>Result</p>',
  )
  const turn = { id: 't', status: 'completed', items: [{ id: 'a', type: 'agentMessage', text: 'reply' }] }
  const ref = { backend: 'codex', id: 'worker', key: 'codex:worker' }
  render(turn, ref); render(turn, ref); assert.equal(count, 1)
  favorite = true; render(turn, ref); assert.equal(count, 2)
  locale = 'zh-CN'; render(turn, ref); assert.equal(count, 3)
  open = ['activity']; render(turn, ref); assert.equal(count, 4)
  turn.items[0].text = 'updated'; render(turn, ref); assert.equal(count, 5)
  turn.status = 'inProgress'; render(turn, ref); render(turn, ref); assert.equal(count, 7)
})
