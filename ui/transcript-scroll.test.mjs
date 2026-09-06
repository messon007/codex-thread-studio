import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  createTranscriptContentObserver,
  createTranscriptScrollFollower,
  distanceFromBottom,
  shouldFollowLatestOnReturn,
  shouldPinTranscriptOnTakeover,
  transcriptResizeAction,
  transcriptScrollEventAction,
  transcriptRestorePlan,
} from './transcript-scroll.mjs'

test('calculates transcript distance from the bottom', () => {
  assert.equal(distanceFromBottom({ scrollHeight: 1200, scrollTop: 700, clientHeight: 500 }), 0)
  assert.equal(distanceFromBottom({ scrollHeight: 1500, scrollTop: 700, clientHeight: 500 }), 300)
})

test('ordinary transcript interaction pins only when the reader is away from the latest output', () => {
  const bottom = { scrollHeight: 1200, scrollTop: 700, clientHeight: 500 }
  const readingHistory = { scrollHeight: 1500, scrollTop: 700, clientHeight: 500 }

  assert.equal(shouldPinTranscriptOnTakeover({ metrics: bottom }), false)
  assert.equal(shouldPinTranscriptOnTakeover({ metrics: readingHistory }), true)
  assert.equal(shouldPinTranscriptOnTakeover({ metrics: bottom, hasLaterTurns: true }), true)
})

test('transcript pointer takeover does not unconditionally pin the latest window', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function handleTranscriptUserTakeover(')
  const end = source.indexOf('\nfunction handleTranscriptKeyboardTakeover', start)
  const takeover = source.slice(start, end)

  assert.match(takeover, /if \(shouldPinTranscriptOnTakeover\(\{ metrics: transcript, hasLaterTurns \}\)\) \{[\s\S]*transcriptScrollFollower\.pause\(\)[\s\S]*transcriptPresentationCache\.pinCurrent\(key\)/u)
  assert.match(takeover, /else \{\s*transcriptScrollFollower\.reset\(\)/u)
})

test('content growth alone does not disable an active follower', () => {
  const follower = createTranscriptScrollFollower()
  assert.equal(follower.following, true)
  // A DOM mutation can increase scrollHeight without emitting a user scroll event.
  assert.equal(follower.following, true)
})

test('return policy follows only recent turns that are still near the bottom', () => {
  const orderedTurnIds = ['turn-1', 'turn-2', 'turn-3', 'turn-4']
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-4', bottomDistance: 20 }), true)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-3', bottomDistance: 20 }), true)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-4', bottomDistance: 200 }), false)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, readingTurnId: 'turn-2', bottomDistance: 20 }), false)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, bottomDistance: 20 }), true)
  assert.equal(shouldFollowLatestOnReturn({ orderedTurnIds, bottomDistance: 200 }), false)
})

test('defers restoration until history is authoritative and then rejects a missing anchor', () => {
  const saved = { scrollTop: 640, anchorTurnId: 'turn-3', anchorOffset: -24, followOnReturn: false }

  assert.deepEqual(transcriptRestorePlan({ saved, historyReady: false, orderedTurnIds: [] }), { type: 'defer' })
  assert.deepEqual(transcriptRestorePlan({ saved, historyReady: true, orderedTurnIds: ['turn-3', 'turn-4'] }), {
    type: 'anchor',
    anchorTurnId: 'turn-3',
  })
  assert.deepEqual(transcriptRestorePlan({ saved, historyReady: true, orderedTurnIds: ['turn-4'] }), { type: 'stale' })
  assert.deepEqual(transcriptRestorePlan({
    saved,
    historyReady: true,
    historyComplete: false,
    orderedTurnIds: ['turn-4'],
  }), { type: 'unavailable' })
})

test('uses a numeric scroll position only when no turn anchor was saved', () => {
  assert.deepEqual(transcriptRestorePlan({
    saved: { scrollTop: 320, anchorTurnId: '', followOnReturn: false },
    orderedTurnIds: ['turn-1'],
  }), { type: 'scrollTop', scrollTop: 320 })
  assert.deepEqual(transcriptRestorePlan({ saved: {}, orderedTurnIds: ['turn-1'] }), { type: 'stale' })
  assert.deepEqual(transcriptRestorePlan(), { type: 'none' })
})

test('layout resize restores only an active selection transaction', () => {
  assert.equal(transcriptResizeAction({ hasPendingRestore: true, following: false }), 'restore')
  assert.equal(transcriptResizeAction({ hasPendingRestore: false, following: true }), 'follow')
  assert.equal(transcriptResizeAction({ hasPendingRestore: false, following: false }), 'preserve')
})

test('layout and programmatic scroll events cannot impersonate user scrolling', () => {
  assert.equal(transcriptScrollEventAction({ hasPendingRestore: true, userInitiated: true, following: false }), 'ignore')
  assert.equal(transcriptScrollEventAction({ userInitiated: false, following: true }), 'follow')
  assert.equal(transcriptScrollEventAction({ userInitiated: false, following: false }), 'preserve')
  assert.equal(transcriptScrollEventAction({ userInitiated: true, following: true }), 'capture')
  assert.equal(transcriptScrollEventAction({ userInitiated: true, following: false }), 'capture')
})

test('the transcript restoration transaction consumes authoritative misses and reveals valid hidden anchors', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const prepareStart = source.indexOf('function prepareTranscriptEntryForRestore(')
  const prepareEnd = source.indexOf('\nfunction restoreTranscriptView', prepareStart)
  const prepare = source.slice(prepareStart, prepareEnd)
  const restoreStart = prepareEnd + 1
  const restoreEnd = source.indexOf('\nfunction renderTranscript', restoreStart)
  const restore = source.slice(restoreStart, restoreEnd)
  const contextStart = source.indexOf('function transcriptRestoreContext(')
  const contextEnd = source.indexOf('\nfunction prepareTranscriptEntryForRestore', contextStart)
  const context = source.slice(contextStart, contextEnd)
  const resizeStart = source.indexOf('function handleTranscriptContentResize(')
  const resizeEnd = source.indexOf('\nfunction renderTurnNavigator', resizeStart)
  const resize = source.slice(resizeStart, resizeEnd)
  const captureStart = source.indexOf('function captureTranscriptViewState(')
  const captureEnd = source.indexOf('\nfunction scheduleTranscriptViewCapture', captureStart)
  const capture = source.slice(captureStart, captureEnd)

  assert.match(prepare, /plan\.type === 'anchor'[\s\S]*restoreTurn\(restore\.key, state\.model,/u)
  assert.match(prepare, /plan\.type === 'stale'[\s\S]*pendingTranscriptViewRestore = null[\s\S]*setScrollState\(restore\.key, null\)[\s\S]*transcriptScrollFollower\.reset\(\)/u)
  assert.match(prepare, /plan\.type === 'unavailable'[\s\S]*pendingTranscriptViewRestore = null[\s\S]*transcriptScrollFollower\.reset\(\)/u)
  assert.ok(restore.indexOf("plan.type === 'none' || restore.plan.type === 'defer'") < restore.indexOf('pendingTranscriptViewRestore = null'))
  assert.ok(restore.indexOf('pendingTranscriptViewRestore = null') < restore.indexOf("plan.type === 'anchor'"))
  assert.match(source, /prepareTranscriptEntryForRestore\(\)/u)
  assert.ok(prepare.indexOf('transcriptRestoreContext({') < prepare.lastIndexOf('currentPresentationEntry()'))
  assert.match(source, /catch \(error\) \{\s*(?:if \(backend === 'opencode' && historyEpoch !== openCodeHistoryEpoch\) return\s*)?if \(state\.backend !== backend \|\| state\.selectedId !== id\) return\s*if \(!cachedVisible\) \{\s*abandonTranscriptHistoryRestore\(\)/u)
  assert.match(context, /if \(!pending\) return/u)
  assert.doesNotMatch(context, /scrollState\(/u)
  assert.match(capture, /if \(pendingTranscriptViewRestore\?\.key === key\) return/u)
  assert.doesNotMatch(capture, /pendingTranscriptViewRestore = null/u)
  assert.match(resize, /transcriptResizeAction/u)
  assert.match(resize, /restoreTranscriptLiveLayoutAnchor\(\)/u)
  assert.doesNotMatch(resize, /scrollState\(|currentPresentationEntry/u)
})

test('late transcript layout growth restores the live viewport anchor before recapturing state', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const rememberStart = source.indexOf('function rememberTranscriptLiveLayoutAnchor(')
  const preserveEnd = source.indexOf('\nfunction preserveTranscriptLayout', rememberStart)
  const liveAnchor = source.slice(rememberStart, preserveEnd)
  const restoreStart = source.indexOf('function restoreTranscriptView(')
  const restoreEnd = source.indexOf('\nfunction renderTranscript', restoreStart)
  const restore = source.slice(restoreStart, restoreEnd)
  const scrollStart = source.indexOf('function handleTranscriptScroll(')
  const scrollEnd = source.indexOf('\nfunction handleTranscriptUserTakeover', scrollStart)
  const scroll = source.slice(scrollStart, scrollEnd)

  assert.match(liveAnchor, /transcriptLiveLayoutAnchor = anchor \? \{ key, anchor \} : null/u)
  assert.ok(liveAnchor.indexOf('restoreTranscriptRenderAnchor(') < liveAnchor.indexOf('scheduleTranscriptViewCapture()'))
  assert.match(restore, /anchor[\s\S]*rememberTranscriptLiveLayoutAnchor\(container\)[\s\S]*return true/u)
  assert.match(scroll, /transcriptScrollEventAction/u)
  assert.match(scroll, /action === 'preserve'[\s\S]*scheduleTurnNavigatorSync\(\)[\s\S]*return/u)
  assert.doesNotMatch(scroll.slice(scroll.indexOf("action === 'preserve'"), scroll.indexOf("markTranscriptUserScrollIntent")), /scheduleTranscriptViewCapture/u)
})

test('same-session transcript rebuilds preserve a live DOM anchor without replaying saved session state', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const captureStart = source.indexOf('function captureTranscriptRenderAnchor(')
  const captureEnd = source.indexOf('\nfunction restoreTranscriptRenderAnchor', captureStart)
  const restoreEnd = source.indexOf('\nfunction transcriptReadingTurnId', captureEnd)
  const helpers = source.slice(captureStart, restoreEnd)
  const renderStart = source.indexOf('function renderTranscript(')
  const renderEnd = source.indexOf('\nfunction handleTranscriptScroll', renderStart)
  const render = source.slice(renderStart, renderEnd)

  assert.match(helpers, /\[data-turn-id\]\[data-item-id\].*work-activity\[data-turn-id\]\[data-activity-id\]/u)
  assert.match(helpers, /activityId:/u)
  assert.doesNotMatch(helpers, /scrollState\(|transcriptPresentationCache/u)
  assert.ok(render.indexOf('captureTranscriptRenderAnchor(container)') < render.indexOf('transcriptDom.render(container'))
  assert.ok(render.indexOf('transcriptDom.render(container') < render.indexOf('restoreTranscriptRenderAnchor(renderAnchor, container)'))
  assert.match(render, /preserveScroll \|\| transcriptScrollFollower\.following[\s\S]*\? null[\s\S]*: captureTranscriptRenderAnchor\(container\)/u)
  assert.match(render, /openActivities[\s\S]*openActivityIds: openActivities\.get\(id\)/u)
})

test('submitting a new turn cancels a pinned reading position and follows the latest window', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const helperStart = source.indexOf('function beginTranscriptFollowingLatest(')
  const helperEnd = source.indexOf('\nfunction handleTranscriptScroll', helperStart)
  const helper = source.slice(helperStart, helperEnd)
  const composerStart = source.indexOf('async function sendComposer(')
  const composerEnd = source.indexOf('\nfunction isRouterThread(', composerStart)
  const composer = readFileSync(new URL('../ui-src/submission-controller.mts', import.meta.url), 'utf8').split('async function sendComposer(')[1].split('async function runNextQueuedMessage(')[0]

  assert.match(helper, /cancelScheduledTranscriptViewCapture\(\)/u)
  assert.match(helper, /pendingTranscriptViewRestore = null/u)
  assert.match(helper, /setScrollState\(key, null\)/u)
  assert.ok(helper.indexOf('transcriptScrollFollower.reset()') < helper.indexOf('transcriptPresentationCache.followLatest(key, model)'))
  assert.match(composer, /beginTranscriptFollowingLatest\(targetModel\)/u)
  assert.ok(composer.indexOf('beginTranscriptFollowingLatest(targetModel)') < composer.indexOf('beginOptimisticCodexTurn('))
})

test('starting the selected session queue follows latest while background queues leave the visible transcript alone', () => {
  const source = readFileSync(new URL('../ui-src/submission-controller.mts', import.meta.url), 'utf8')
  const start = source.indexOf('async function runNextQueuedMessage(')
  const end = source.indexOf('\nfunction pauseMessageQueue(', start)
  const queue = source.slice(start, end)

  assert.match(queue, /if \(key === selectedStateKey\(\)\) \{\s*beginTranscriptFollowingLatest\(model\)\s*renderComposerState\(\)/u)
  assert.ok(queue.indexOf('beginTranscriptFollowingLatest(model)') < queue.indexOf('startTurnWithPreparation({'))
})

test('a hidden newer window is presented as a direct jump to latest rather than a turn count', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

  assert.match(source, /data-jump-latest[^>]*>\$\{t\('Jump to latest'\)\}/u)
  assert.doesNotMatch(source, /data-load-later|\{count\} later turns/u)
  assert.match(source, /closest\('\[data-jump-latest\]'\)[\s\S]*beginTranscriptFollowingLatest\(\)[\s\S]*renderTranscript\(\)/u)
})

test('right-rail layout mutations preserve a live anchor independently of session restore state', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('function preserveTranscriptLayout(')
  const end = source.indexOf('\nfunction transcriptReadingTurnId', start)
  const helper = source.slice(start, end)
  assert.ok(helper.indexOf('captureTranscriptRenderAnchor(container)') < helper.indexOf('mutate()'))
  assert.ok(helper.indexOf('mutate()') < helper.indexOf('restoreTranscriptRenderAnchor(anchor, container)'))
  assert.doesNotMatch(helper, /scrollState\(/u)
})

test('opening a file link from the transcript uses the live anchor transaction', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('async function handleTranscriptClick(')
  const end = source.indexOf('\nfunction openActivityLog(', start)
  const click = source.slice(start, end)

  assert.ok(start >= 0 && end > start)
  assert.ok(click.indexOf("event.currentTarget === $('#transcript')") < click.indexOf('preserveTranscriptLayout('))
  assert.ok(click.indexOf('preserveTranscriptLayout(') < click.indexOf('await opening'))
  assert.match(click, /preserveTranscriptLayout\(\(\) => \{ opening = openLinkedArtifact\(\) \}\)/u)
  assert.match(click, /else \{\s*await openLinkedArtifact\(\)\s*\}/u)
})

test('terminal turn updates keep activities the reader explicitly expanded', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const replaceStart = source.indexOf('function replaceRenderedTurn(')
  const replaceEnd = source.indexOf('\nfunction renderTurn(', replaceStart)
  const replace = source.slice(replaceStart, replaceEnd)

  assert.match(replace, /querySelectorAll\('\.work-activity\[open\]'\)/u)
  assert.doesNotMatch(source, /preserveActivity/u)
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
