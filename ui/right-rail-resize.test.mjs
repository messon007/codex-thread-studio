import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import vm from 'node:vm'
import test from 'node:test'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
const functions = ['normalizeRightRailWidthRatio', 'applyRightRailWidth', 'beginRightRailResize', 'continueRightRailResize', 'finishRightRailResize', 'clearRightRailResize', 'cancelRightRailResize', 'handleRightRailResizeEscape', 'resetRightRailWidth', 'handleRightRailResizeKey']
const implementation = functions.map(name => {
  const match = app.match(new RegExp(`function ${name}\\([^]*?\\n\\}`, 'u'))
  assert.ok(match, name)
  return match[0]
}).join('\n')

function fixture({ captureFails = false } = {}) {
  const calls = { bounds: 0, geometry: 0, resize: 0, persist: 0, widths: [], releases: [] }
  const overlays = []
  const bounds = { available: 1200, min: 480, max: 780 }
  const state = { rightRailWidthRatio: 0.44 }
  const capture = new Set()
  const resizer = {
    parentElement: { getBoundingClientRect: () => { calls.geometry++; return { left: 672, right: 1200, width: 528 } } },
    setAttribute() {},
    setPointerCapture: pointer => { if (captureFails) throw Error('Capture failed'); capture.add(pointer) },
    hasPointerCapture: pointer => capture.has(pointer),
    releasePointerCapture: pointer => { capture.delete(pointer); calls.releases.push(pointer) },
  }
  const document = {
    documentElement: { style: { setProperty: (name, value) => calls.widths.push([name, value]) } },
    body: { append: node => overlays.push(node) },
    createElement: () => ({ style: {}, dataset: {}, attributes: {}, children: [], setAttribute(name, value) { this.attributes[name] = value }, append(child) { this.children.push(child) }, remove() { overlays.splice(overlays.indexOf(this), 1) } }),
  }
  const context = vm.createContext({
    state, document,
    window: { getSelection: () => ({ removeAllRanges() {} }) },
    appRightRailWidthBounds: () => { calls.bounds++; return bounds },
    workspaceTools: { resize: () => calls.resize++ },
    persistPreferences: () => calls.persist++,
    $$: () => [resizer],
  })
  vm.runInContext(`let rightRailResize = null;\n${implementation}`, context)
  const event = (clientX = 676, options = {}) => ({ button: 0, pointerId: 1, clientX, currentTarget: resizer, preventDefault() {}, stopPropagation() {}, ...options })
  return { context, calls, state, overlays, event, capture }
}

test('pointer movement changes only the guide, with one geometry snapshot', () => {
  const { context, calls, state, overlays, event } = fixture()
  context.beginRightRailResize(event())
  assert.equal(overlays.length, 1)
  assert.equal(overlays[0].attributes['aria-hidden'], 'true')
  assert.equal(overlays[0].children[0].style.transform, 'translate3d(672px, 0, 0)')
  for (let index = 0; index < 100; index++) context.continueRightRailResize(event(600 + index))
  assert.equal(calls.geometry, 1)
  assert.equal(calls.bounds, 1)
  assert.deepEqual(calls.widths, [])
  assert.equal(calls.resize, 0)
  assert.equal(calls.persist, 0)
  assert.equal(state.rightRailWidthRatio, 0.44)
  assert.notEqual(overlays[0].children[0].style.transform, 'translate3d(672px, 0, 0)')
})

test('release uses its final coordinates and commits width, workspace fit and preferences once', () => {
  const { context, calls, state, overlays, event, capture } = fixture()
  context.beginRightRailResize(event())
  context.continueRightRailResize(event(650))
  context.finishRightRailResize(event(576))
  context.finishRightRailResize(event(576))
  assert.equal(overlays.length, 0)
  assert.equal(capture.size, 0)
  assert.deepEqual(calls.releases, [1])
  assert.deepEqual(calls.widths, [['--right-rail-width', '628px']])
  assert.equal(state.rightRailWidthRatio, 628 / 1200)
  assert.equal(calls.resize, 1)
  assert.equal(calls.persist, 1)
  assert.equal(calls.bounds, 2)
})

test('preview and committed widths share min/max bounds without a grab-offset jump', () => {
  for (const [clientX, width] of [[-5000, 780], [5000, 480]]) {
    const { context, calls, overlays, event } = fixture()
    context.beginRightRailResize(event())
    context.continueRightRailResize(event(clientX))
    assert.equal(overlays[0].children[0].style.transform, `translate3d(${1200 - width}px, 0, 0)`)
    context.finishRightRailResize(event(clientX))
    assert.deepEqual(calls.widths, [['--right-rail-width', `${width}px`]])
  }
})

test('clicks and drags returning to the starting point do not trigger layout or persistence', () => {
  for (const move of [false, true]) {
    const { context, calls, overlays, event } = fixture()
    context.beginRightRailResize(event())
    if (move) context.continueRightRailResize(event(550))
    context.finishRightRailResize(event())
    assert.equal(overlays.length, 0)
    assert.deepEqual(calls.widths, [])
    assert.equal(calls.resize, 0)
    assert.equal(calls.persist, 0)
  }
})

test('cancel, capture loss, window blur and Escape discard the preview', () => {
  for (const cause of ['cancel', 'capture', 'blur', 'escape']) {
    const { context, calls, state, overlays, event, capture } = fixture()
    context.beginRightRailResize(event())
    context.continueRightRailResize(event(500))
    if (cause === 'escape') {
      let prevented = false
      let stopped = false
      context.handleRightRailResizeEscape(event(0, { key: 'Escape', preventDefault: () => { prevented = true }, stopPropagation: () => { stopped = true } }))
      assert.equal(prevented, true)
      assert.equal(stopped, true)
    } else context.cancelRightRailResize(cause === 'blur' ? {} : event())
    assert.equal(overlays.length, 0)
    assert.equal(capture.size, 0)
    assert.deepEqual(calls.widths, [])
    assert.equal(calls.persist, 0)
    assert.equal(state.rightRailWidthRatio, 0.44)
  }
})

test('secondary buttons and unrelated pointers cannot start, finish or cancel another drag', () => {
  const { context, calls, overlays, event } = fixture()
  context.beginRightRailResize(event(676, { button: 2 }))
  assert.equal(overlays.length, 0)
  context.beginRightRailResize(event())
  const guide = overlays[0].children[0]
  const initial = guide.style.transform
  context.beginRightRailResize(event(400, { pointerId: 2 }))
  context.continueRightRailResize(event(400, { pointerId: 2 }))
  context.finishRightRailResize(event(400, { pointerId: 2 }))
  context.cancelRightRailResize(event(400, { pointerId: 2 }))
  assert.equal(overlays.length, 1)
  assert.equal(guide.style.transform, initial)
  assert.equal(calls.geometry, 1)
  context.cancelRightRailResize()
})

test('pointer capture failure cleans up the overlay without changing the layout', () => {
  const { context, calls, overlays, event } = fixture({ captureFails: true })
  context.beginRightRailResize(event())
  assert.equal(overlays.length, 0)
  assert.deepEqual(calls.widths, [])
})

test('keyboard resize and reset remain immediate and cancel any pending preview', () => {
  const { context, calls, overlays, event } = fixture()
  context.beginRightRailResize(event())
  context.continueRightRailResize(event(500))
  context.handleRightRailResizeKey(event(0, { key: 'ArrowLeft', shiftKey: false }))
  assert.equal(overlays.length, 0)
  assert.deepEqual(calls.widths, [['--right-rail-width', '552px']])
  assert.equal(calls.persist, 1)
  context.resetRightRailWidth()
  assert.deepEqual(calls.widths.at(-1), ['--right-rail-width', '528px'])
  assert.equal(calls.persist, 2)
})

test('all rail handles share cancellation hooks and a local overlay instead of document-wide drag styles', () => {
  assert.match(app, /resizer\.addEventListener\('pointercancel', cancelRightRailResize\)/u)
  assert.match(app, /resizer\.addEventListener\('lostpointercapture', cancelRightRailResize\)/u)
  assert.match(app, /window\.addEventListener\('blur', cancelRightRailResize\)/u)
  assert.match(app, /window\.addEventListener\('keydown', handleRightRailResizeEscape, true\)/u)
  assert.match(app, /window\.addEventListener\('resize', \(\) => \{\s+cancelRightRailResize\(\)/u)
  assert.match(app, /function activateRightWorkspace\(tool\) \{\s+cancelRightRailResize\(\)/u)
  assert.doesNotMatch(styles, /body\.resizing-right-rail/u)
  assert.match(styles, /\.right-rail-resize-guide \{[^}]*will-change: transform/u)
})
