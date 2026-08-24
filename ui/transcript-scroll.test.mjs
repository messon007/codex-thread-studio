import test from 'node:test'
import assert from 'node:assert/strict'

import {
  createTranscriptContentObserver,
  createTranscriptScrollFollower,
  distanceFromBottom,
  shouldFollowLatestOnReturn,
} from './transcript-scroll.mjs'

test('calculates transcript distance from the bottom', () => {
  assert.equal(distanceFromBottom({ scrollHeight: 1200, scrollTop: 700, clientHeight: 500 }), 0)
  assert.equal(distanceFromBottom({ scrollHeight: 1500, scrollTop: 700, clientHeight: 500 }), 300)
})

test('content growth alone does not disable an active follower', () => {
  const follower = createTranscriptScrollFollower()
  assert.equal(follower.following, true)
  // A DOM mutation can increase scrollHeight without emitting a user scroll event.
  assert.equal(follower.following, true)
})

test('return policy follows the latest two turns but preserves older reading positions', () => {
  const orderedTurnIds = ['turn-1', 'turn-2', 'turn-3', 'turn-4']
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-4' }), true)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-3' }), true)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-2' }), false)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, bottomDistance: 20 }), true)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, bottomDistance: 200 }), false)
})

test('user scrolling pauses and returning to the bottom resumes following', () => {
  const follower = createTranscriptScrollFollower({ threshold: 48 })
  assert.equal(follower.handleScroll({ scrollHeight: 1600, scrollTop: 700, clientHeight: 500 }), false)
  assert.equal(follower.handleScroll({ scrollHeight: 1600, scrollTop: 1060, clientHeight: 500 }), true)
  follower.pause()
  assert.equal(follower.following, false)
  follower.reset()
  assert.equal(follower.following, true)
})

test('content observer follows late transcript layout changes and replaces stale observations', () => {
  const observed = []
  let disconnects = 0
  let notifyResize = null
  class FakeResizeObserver {
    constructor(callback) {
      notifyResize = callback
    }

    observe(element) {
      observed.push(element)
    }

    disconnect() {
      disconnects += 1
    }
  }

  let resizeCount = 0
  const observer = createTranscriptContentObserver({
    ResizeObserverClass: FakeResizeObserver,
    onResize: () => { resizeCount += 1 },
  })
  const firstChildren = [{ id: 'turn-a' }, { id: 'turn-b' }]
  const secondChildren = [{ id: 'turn-c' }]

  observer.observe({ children: firstChildren })
  notifyResize()
  observer.observe({ children: secondChildren })

  assert.deepEqual(observed, [...firstChildren, ...secondChildren])
  assert.equal(resizeCount, 1)
  assert.equal(disconnects, 2)
})
