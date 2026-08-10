import assert from 'node:assert/strict'
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
