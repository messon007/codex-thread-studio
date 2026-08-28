import assert from 'node:assert/strict'
import test from 'node:test'

import { createSessionResourcesUI } from './session-resources-ui.mjs'
import { buildSessionResourceIndex } from './session-resources.mjs'

test('keeps resource extraction lazy and caches it by latest-turn revision', async (context) => {
  const originalDocument = globalThis.document
  const hadDocument = Object.hasOwn(globalThis, 'document')
  const elements = fakeResourceElements()
  globalThis.document = { getElementById: (id) => elements[id] || null }
  context.after(() => {
    if (hadDocument) globalThis.document = originalDocument
    else delete globalThis.document
  })

  const thread = { id: 'thread-1', cwd: '/repo' }
  let answerText = 'See docs/guide.md'
  let textReads = 0
  const answer = { id: 'answer', type: 'agentMessage' }
  Object.defineProperty(answer, 'text', {
    configurable: true,
    enumerable: true,
    get() {
      textReads += 1
      return answerText
    },
  })
  const model = {
    turns: [{ id: 'latest', items: [answer] }],
  }
  let modelBuilds = 0
  const idle = controlledIdleScheduler()
  const resources = createSessionResourcesUI({
    getThread: () => thread,
    getModel: () => model,
    getBackend: () => 'codex',
    buildIndex(candidate, options) {
      if (candidate) modelBuilds += 1
      return buildSessionResourceIndex(candidate, options)
    },
    scheduleIdle: idle.schedule,
  })

  resources.syncSelection()
  resources.sync()
  assert.equal(modelBuilds, 0)
  assert.equal(textReads, 0)

  resources.open()
  assert.equal(modelBuilds, 1)
  assert.equal(resources.currentIndex().counts().all, 1)

  resources.close()
  textReads = 0
  resources.sync()
  assert.equal(textReads, 0)
  resources.open()
  assert.equal(modelBuilds, 1)

  resources.close()
  answerText = 'See docs/other.md'
  textReads = 0
  resources.sync({ contentChanged: true })
  assert.equal(textReads, 0)
  assert.equal(modelBuilds, 1)
  resources.open()
  assert.equal(modelBuilds, 2)
  await new Promise((resolve) => setTimeout(resolve, 40))
})

test('shows the last known resource count immediately and refreshes it lazily from the latest turn', (context) => {
  const originalDocument = globalThis.document
  const hadDocument = Object.hasOwn(globalThis, 'document')
  const elements = fakeResourceElements()
  globalThis.document = { getElementById: (id) => elements[id] || null }
  context.after(() => {
    if (hadDocument) globalThis.document = originalDocument
    else delete globalThis.document
  })

  const thread = { id: 'thread-1', cwd: '/repo' }
  let olderReads = 0
  const older = { id: 'older-answer', type: 'agentMessage' }
  Object.defineProperty(older, 'text', {
    configurable: true,
    enumerable: true,
    get() {
      olderReads += 1
      return '[old](docs/old.md)'
    },
  })
  let answerText = '[guide](docs/guide.md) [api](docs/api.md)'
  let latestReads = 0
  const answer = { id: 'latest-answer', type: 'agentMessage' }
  Object.defineProperty(answer, 'text', {
    configurable: true,
    enumerable: true,
    get() {
      latestReads += 1
      return answerText
    },
  })
  const model = {
    turns: [
      { id: 'older', items: [older] },
      { id: 'latest', items: [answer] },
    ],
  }
  let modelBuilds = 0
  const idle = controlledIdleScheduler()
  const resources = createSessionResourcesUI({
    getThread: () => thread,
    getModel: () => model,
    getBackend: () => 'codex',
    buildIndex(candidate, options) {
      if (candidate) modelBuilds += 1
      return buildSessionResourceIndex(candidate, options)
    },
    scheduleIdle: idle.schedule,
  })

  resources.syncSelection()
  assert.equal(modelBuilds, 0)
  assert.equal(latestReads, 0)
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), true)
  assert.equal(idle.activeCount(), 1)

  idle.flush()
  assert.equal(modelBuilds, 1)
  assert.equal(olderReads, 0)
  assert.ok(latestReads > 0)
  assert.equal(elements['thread-resources-count'].textContent, '2')
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), false)

  answerText = '[guide](docs/guide.md)'
  latestReads = 0
  resources.invalidate('codex', 'thread-1')
  assert.equal(modelBuilds, 1)
  assert.equal(latestReads, 0)
  assert.equal(elements['thread-resources-count'].textContent, '2')
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), false)

  resources.sync()
  assert.equal(idle.activeCount(), 1)
  idle.flush()
  assert.equal(modelBuilds, 2)
  assert.equal(olderReads, 0)
  assert.equal(elements['thread-resources-count'].textContent, '1')
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), false)
})

test('reuses a scanned zero count until a completed response invalidates it', (context) => {
  const originalDocument = globalThis.document
  const hadDocument = Object.hasOwn(globalThis, 'document')
  const elements = fakeResourceElements()
  globalThis.document = { getElementById: (id) => elements[id] || null }
  context.after(() => {
    if (hadDocument) globalThis.document = originalDocument
    else delete globalThis.document
  })

  const thread = { id: 'thread-1', cwd: '/repo' }
  let answerText = 'No resource references here.'
  let textReads = 0
  const answer = { id: 'answer', type: 'agentMessage' }
  Object.defineProperty(answer, 'text', {
    configurable: true,
    enumerable: true,
    get() {
      textReads += 1
      return answerText
    },
  })
  const model = { turns: [{ id: 'latest', items: [answer] }] }
  let modelBuilds = 0
  const idle = controlledIdleScheduler()
  const resources = createSessionResourcesUI({
    getThread: () => thread,
    getModel: () => model,
    getBackend: () => 'codex',
    buildIndex(candidate, options) {
      if (candidate) modelBuilds += 1
      return buildSessionResourceIndex(candidate, options)
    },
    scheduleIdle: idle.schedule,
  })

  resources.syncSelection()
  idle.flush()
  assert.equal(modelBuilds, 1)
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), true)

  textReads = 0
  resources.syncSelection()
  resources.sync()
  assert.equal(idle.activeCount(), 0)
  assert.equal(modelBuilds, 1)
  assert.equal(textReads, 0)

  answerText = 'The completed reply links [the guide](docs/guide.md).'
  resources.invalidate('codex', 'thread-1')
  assert.equal(idle.activeCount(), 1)
  idle.flush()
  assert.equal(modelBuilds, 2)
  assert.equal(elements['thread-resources-count'].textContent, '1')
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), false)
})

test('cancels a stale lazy resource scan when the selected thread changes', (context) => {
  const originalDocument = globalThis.document
  const hadDocument = Object.hasOwn(globalThis, 'document')
  const elements = fakeResourceElements()
  globalThis.document = { getElementById: (id) => elements[id] || null }
  context.after(() => {
    if (hadDocument) globalThis.document = originalDocument
    else delete globalThis.document
  })

  let thread = { id: 'thread-a', cwd: '/repo' }
  let model = {
    turns: [{
      id: 'latest-a',
      items: [{ id: 'answer-a', type: 'agentMessage', text: '[one](docs/one.md) [two](docs/two.md)' }],
    }],
  }
  const idle = controlledIdleScheduler()
  const builtThreadIds = []
  const resources = createSessionResourcesUI({
    getThread: () => thread,
    getModel: () => model,
    getBackend: () => 'codex',
    buildIndex(candidate, options) {
      if (candidate) builtThreadIds.push(options.threadId)
      return buildSessionResourceIndex(candidate, options)
    },
    scheduleIdle: idle.schedule,
  })

  resources.syncSelection()
  thread = { id: 'thread-b', cwd: '/repo' }
  model = {
    turns: [{
      id: 'latest-b',
      items: [{ id: 'answer-b', type: 'agentMessage', text: '[only](docs/only.md)' }],
    }],
  }
  resources.syncSelection()

  assert.equal(idle.activeCount(), 1)
  idle.flush()
  assert.deepEqual(builtThreadIds, ['thread-b'])
  assert.equal(elements['thread-resources-count'].textContent, '1')
  assert.equal(elements['thread-resources-count'].classList.contains('hidden'), false)
})

function controlledIdleScheduler() {
  let jobs = []
  return {
    schedule(callback) {
      const job = { callback, cancelled: false }
      jobs.push(job)
      return () => { job.cancelled = true }
    },
    activeCount() {
      return jobs.filter((job) => !job.cancelled).length
    },
    flush() {
      const pending = jobs
      jobs = []
      for (const job of pending) {
        if (!job.cancelled) job.callback()
      }
    },
  }
}

function fakeResourceElements() {
  const ids = [
    'open-thread-resources', 'thread-resources-count', 'resources-rail', 'resources-path',
    'resources-count', 'resources-search', 'resources-filters', 'resources-list',
    'resources-empty', 'resources-summary', 'resources-local-state',
  ]
  return Object.fromEntries(ids.map((id) => [id, fakeElement(id === 'resources-rail')]))
}

function fakeElement(hidden = false) {
  const classes = new Set(hidden ? ['hidden'] : [])
  return {
    classList: {
      add: (...names) => names.forEach((name) => classes.add(name)),
      remove: (...names) => names.forEach((name) => classes.delete(name)),
      contains: (name) => classes.has(name),
      toggle(name, force) {
        const enabled = force === undefined ? !classes.has(name) : force
        if (enabled) classes.add(name)
        else classes.delete(name)
        return enabled
      },
    },
    addEventListener() {},
    focus() {},
    querySelectorAll: () => [],
    setAttribute(name, value) { this.attributes[name] = String(value) },
    attributes: {},
    disabled: false,
    innerHTML: '',
    textContent: '',
    value: '',
  }
}
