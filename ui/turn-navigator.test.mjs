import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  activeTurnAtMarker,
  navigableTurns,
  turnNavigationLabel,
  turnHasUserInput,
  turnPromptPreview,
} from './turn-navigator.mjs'

test('builds a compact preview from the user message in a turn', () => {
  const turn = {
    items: [{
      type: 'userMessage',
      content: [{ type: 'text', text: '  explain\nthis   code  ' }, { type: 'localImage', path: '/tmp/a.png' }],
    }],
  }
  assert.equal(turnPromptPreview(turn), 'explain this code')
  assert.equal(turnNavigationLabel(turn, 2), 'User input 3: explain this code')
})

test('truncates long turn previews and handles missing user text', () => {
  const turn = {
    items: [{ type: 'userMessage', content: [{ type: 'text', text: 'abcdefghij' }] }],
  }
  assert.equal(turnPromptPreview(turn, 6), 'abcde…')
  assert.equal(turnPromptPreview({
    items: [{ type: 'userMessage', content: [{ type: 'text', text: `prefix ${'x'.repeat(100_000)}` }] }],
  }, 10), 'prefix xx…')
  assert.equal(turnPromptPreview({
    items: [{ type: 'userMessage', content: [{ type: 'text', text: 'A😀BCD' }] }],
  }, 4), 'A😀B…')
  assert.equal(turnPromptPreview({ items: [] }), '')
})

test('uses only actual user inputs as navigation split points', () => {
  const userTurn = { id: 'user', items: [{ type: 'userMessage', content: [{ type: 'text', text: 'Question' }] }] }
  const assistantTurn = { id: 'assistant', items: [{ type: 'agentMessage', text: 'Answer' }] }
  const activityTurn = { id: 'activity', items: [{ type: 'commandExecution', command: ['pwd'] }] }
  const imageTurn = { id: 'image', items: [{ type: 'userMessage', content: [{ type: 'localImage', path: '/tmp/a.png' }] }] }

  assert.equal(turnHasUserInput(userTurn), true)
  assert.equal(turnHasUserInput(assistantTurn), false)
  assert.deepEqual(navigableTurns([userTurn, assistantTurn, activityTurn, imageTurn]), [userTurn, imageTurn])
})

test('selects the last turn above the reading marker', () => {
  const positions = [
    { id: 'turn-1', top: 20 },
    { id: 'turn-2', top: 180 },
    { id: 'turn-3', top: 420 },
  ]
  assert.equal(activeTurnAtMarker(positions, 10), 'turn-1')
  assert.equal(activeTurnAtMarker(positions, 200), 'turn-2')
  assert.equal(activeTurnAtMarker(positions, 900), 'turn-3')
})

test('selects the final turn at the bottom of the transcript', () => {
  const positions = [{ id: 'turn-1', top: 20 }, { id: 'turn-2', top: 700 }]
  assert.equal(activeTurnAtMarker(positions, 200, true), 'turn-2')
  assert.equal(activeTurnAtMarker([], 200), null)
})

test('turn navigation records its final position before another session can be selected', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function handleTurnNavigatorClick(')
  const end = source.indexOf('\nfunction queueStreamingItemPatch', start)
  const handler = source.slice(start, end)

  assert.ok(handler.indexOf('pendingTranscriptViewRestore = null') < handler.indexOf('renderTranscript()'))
  assert.ok(handler.indexOf('transcriptPresentationCache.showTurn(') < handler.indexOf("querySelectorAll('.turn[data-turn-id]')"))
  assert.ok(handler.indexOf('transcript.scrollTop = Math.max(0, top)') < handler.indexOf('captureTranscriptViewState()'))
  assert.doesNotMatch(handler, /behavior:\s*['"]smooth['"]/u)
})

test('turn navigator reuses DOM only when ids, previews, and locale are unchanged', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function renderTurnNavigator(')
  const end = source.indexOf('\nfunction scheduleTurnNavigatorSync', start)
  const render = source.slice(start, end)
  assert.match(render, /const preview = turnPromptPreview\(turn\)/u)
  assert.match(render, /state\.language/u)
  assert.match(render, /signature === turnNavigatorSignature/u)
})

test('turn navigator click clears reminder for corresponding turn', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function handleTurnNavigatorClick(')
  const end = source.indexOf('\nfunction queueStreamingItemPatch', start)
  const handler = source.slice(start, end)

  assert.match(handler, /threadRouter\.markReminderReadForTurn\?\.?\(button\.dataset\.turnNavId\)/u)
})

test('builds the full turn navigator after the transcript first paint', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const transcriptStart = source.indexOf('function renderTranscript(')
  const transcriptEnd = source.indexOf('\nfunction markTranscriptUserScrollIntent', transcriptStart)
  const transcriptRender = source.slice(transcriptStart, transcriptEnd)
  const scheduleStart = source.indexOf('function scheduleTurnNavigatorRender(')
  const scheduleEnd = source.indexOf('\nfunction renderTurnNavigator(', scheduleStart)
  const scheduler = source.slice(scheduleStart, scheduleEnd)

  assert.match(transcriptRender, /scheduleTurnNavigatorRender\(\)/u)
  assert.doesNotMatch(transcriptRender, /\n\s*renderTurnNavigator\(\)/u)
  assert.match(scheduler, /turnNavigatorRenderFrame = requestAnimationFrame\(afterPaint\)/u)
  assert.match(scheduler, /requestIdleCallback\(run, \{ timeout: 750 \}\)/u)
  assert.match(scheduler, /key !== presentationThreadKey\(\)/u)
  assert.match(scheduler, /turnNavigatorRenderedKey !== key/u)
  assert.match(scheduler, /if \(alreadyScheduled\) return/u)
})
