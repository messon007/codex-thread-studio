import test from 'node:test'
import assert from 'node:assert/strict'
import { createTranscriptDom } from './transcript-dom.mjs'

// Minimal tree implements the DOM operations used by reconciliation. Browser
// integration is checked separately; these tests stay dependency-free in CI.
function fixture() {
  const container = {
    childNodes: [],
    get firstChild() { return this.childNodes[0] || null },
    insertBefore(node, cursor) {
      node.remove()
      const index = cursor ? this.childNodes.indexOf(cursor) : this.childNodes.length
      assert.ok(index >= 0)
      this.childNodes.splice(index, 0, node)
      node.parentNode = this
    },
    ownerDocument: {
      createElement() {
        return {
          set innerHTML(html) {
            this.content = { childNodes: html ? html.split('|').map(value => ({
              value,
              parentNode: null,
              get nextSibling() {
                const siblings = this.parentNode.childNodes
                return siblings[siblings.indexOf(this) + 1] || null
              },
              remove() {
                if (!this.parentNode) return
                const siblings = this.parentNode.childNodes
                siblings.splice(siblings.indexOf(this), 1)
                this.parentNode = null
              },
            })) : [] }
          },
        }
      },
    },
  }
  return container
}
const chunk = (id, html = id) => ({ id, html })

test('retains unchanged turns and replaces only a changed turn', () => {
  const dom = createTranscriptDom()
  const container = fixture()
  dom.render(container, 'codex:a', [chunk('a'), chunk('b'), chunk('c')])
  const [a, b, c] = container.childNodes
  let moved = 0
  const insert = container.insertBefore.bind(container)
  container.insertBefore = (...args) => { moved++; insert(...args) }
  const result = dom.render(container, 'codex:a', [chunk('a'), chunk('b', 'new'), chunk('c')])
  assert.equal(result.reusedTurns, 2)
  assert.deepEqual(container.childNodes.map(node => node.value), ['a', 'new', 'c'])
  assert.equal(container.childNodes[0], a)
  assert.equal(container.childNodes[2], c)
  assert.equal(b.parentNode, null)
  assert.equal(moved, 1)
})

test('handles append, reorder, removal, empty and multi-root chunks', () => {
  const dom = createTranscriptDom()
  const container = fixture()
  dom.render(container, 'a', [chunk('a'), chunk('b', 'b|b2'), chunk('empty', '')])
  const [a, b, b2] = container.childNodes
  dom.render(container, 'a', [chunk('b', 'b|b2'), chunk('a'), chunk('c')])
  assert.deepEqual(container.childNodes.slice(0, 3), [b, b2, a])
  dom.render(container, 'a', [chunk('a')])
  assert.deepEqual(container.childNodes, [a])
  dom.render(container, 'a', [])
  assert.equal(container.childNodes.length, 0)
})

test('stream invalidation and backend/session changes prevent stale reuse', () => {
  const dom = createTranscriptDom()
  const container = fixture()
  dom.render(container, 'codex:a', [chunk('a')])
  const first = container.firstChild
  first.value = 'streamed'
  dom.invalidate('a')
  assert.equal(dom.render(container, 'codex:a', [chunk('a')]).reusedTurns, 0)
  assert.notEqual(container.firstChild, first)
  const second = container.firstChild
  assert.equal(dom.render(container, 'ept-codex:a', [chunk('a')]).reusedTurns, 0)
  assert.notEqual(container.firstChild, second)
})

test('limits retained HTML memory without omitting visible content', () => {
  const dom = createTranscriptDom({ maxBytes: 4 })
  const container = fixture()
  const chunks = [chunk('large', '12345'), chunk('small', '12')]
  dom.render(container, 'a', chunks)
  const [large, small] = container.childNodes
  const result = dom.render(container, 'a', chunks)
  assert.equal(result.cachedDomBytes, 4)
  assert.equal(result.reusedTurns, 1)
  assert.notEqual(container.firstChild, large)
  assert.equal(container.childNodes[1], small)
  assert.deepEqual(container.childNodes.map(node => node.value), ['12345', '12'])
})
