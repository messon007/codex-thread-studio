import test from 'node:test'
import assert from 'node:assert/strict'

import {
  createSessionLibraryState,
  createThreadSearchState,
} from './session-management.mjs'

test('session management state factories return isolated mutable state', () => {
  const firstLibrary = createSessionLibraryState()
  const secondLibrary = createSessionLibraryState()
  firstLibrary.entries.push({ backend: 'codex', thread: { id: 'one' } })
  firstLibrary.errorsByBackend.codex = 'offline'
  firstLibrary.nextCursors.codex = 'next'
  assert.deepEqual(secondLibrary.entries, [])
  assert.deepEqual(secondLibrary.errorsByBackend, {})
  assert.deepEqual(secondLibrary.nextCursors, {})

  const firstSearch = createThreadSearchState()
  const secondSearch = createThreadSearchState()
  firstSearch.entries.push({ turnId: 'turn-one' })
  assert.deepEqual(secondSearch.entries, [])
})
