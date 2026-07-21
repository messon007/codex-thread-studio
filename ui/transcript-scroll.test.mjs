import test from 'node:test'
import assert from 'node:assert/strict'

import { createTranscriptScrollFollower, distanceFromBottom } from './transcript-scroll.mjs'

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

test('user scrolling pauses and returning to the bottom resumes following', () => {
  const follower = createTranscriptScrollFollower({ threshold: 48 })
  assert.equal(follower.handleScroll({ scrollHeight: 1600, scrollTop: 700, clientHeight: 500 }), false)
  assert.equal(follower.handleScroll({ scrollHeight: 1600, scrollTop: 1060, clientHeight: 500 }), true)
  follower.pause()
  assert.equal(follower.following, false)
  follower.reset()
  assert.equal(follower.following, true)
})
