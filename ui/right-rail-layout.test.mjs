import assert from 'node:assert/strict'
import test from 'node:test'

import { clampRightRailWidth, RIGHT_RAIL_DEFAULT_RATIO, rightRailWidthBounds } from './right-rail-layout.mjs'

test('document and workspace rails share one responsive width model', () => {
  const bounds = rightRailWidthBounds({
    containerWidth: 1400,
    sidebarWidth: 310,
    dividerWidth: 9,
    nominalMinWidth: 310,
  })
  assert.deepEqual(bounds, { available: 1081, min: 310, max: 702.65 })
  assert.equal(clampRightRailWidth(bounds.available * RIGHT_RAIL_DEFAULT_RATIO, bounds), 476)
  assert.equal(clampRightRailWidth(900, bounds), 703)
})

test('right rail bounds remain valid in a narrow window', () => {
  const bounds = rightRailWidthBounds({ containerWidth: 260, nominalMinWidth: 310 })
  assert.deepEqual(bounds, { available: 260, min: 260, max: 260 })
})
